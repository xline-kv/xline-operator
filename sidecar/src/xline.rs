#![allow(dead_code)] // TODO remove as it is implemented
#![allow(clippy::unnecessary_wraps)] // TODO remove as it is implemented
#![allow(clippy::unused_self)] // TODO remove as it is implemented

use std::collections::HashMap;
use std::fmt::Debug;
use std::time::Duration;

use anyhow::{anyhow, Result};
use bytes::Buf;
use engine::{Engine, EngineType, StorageEngine};
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use operator_api::consts::DEFAULT_DATA_DIR;
use tonic::transport::{Channel, Endpoint};
use tonic_health::pb::health_check_response::ServingStatus;
use tonic_health::pb::health_client::HealthClient;
use tonic_health::pb::HealthCheckRequest;
use tracing::debug;
use xline_client::types::kv::RangeRequest;
use xline_client::{Client, ClientOptions};

use crate::backup::Metadata;
use crate::backup::Provider;

/// Meta table name
pub(crate) const META_TABLE: &str = "meta";
/// KV table name
pub(crate) const KV_TABLE: &str = "kv";
/// Lease table name
pub(crate) const LEASE_TABLE: &str = "lease";
/// User table
pub(crate) const USER_TABLE: &str = "user";
/// Role table
pub(crate) const ROLE_TABLE: &str = "role";
/// Auth table
pub(crate) const AUTH_TABLE: &str = "auth";

/// They are copied from xline because the sidecar operator want to handle the storage engine directly
pub(crate) const XLINE_TABLES: [&str; 6] = [
    META_TABLE,
    KV_TABLE,
    LEASE_TABLE,
    AUTH_TABLE,
    USER_TABLE,
    ROLE_TABLE,
];

/// The xline server handle
#[derive(Debug)]
pub(crate) struct XlineHandle {
    /// The name of the operator
    name: String,
    /// The xline backup provider
    backup: Option<Box<dyn Provider>>,
    /// The xline client
    client: Option<Client>,
    /// The xline health client
    health_client: HealthClient<Channel>,
    /// The rocks db engine
    engine: Engine,
    /// The xline members
    xline_members: HashMap<String, String>,
    /// Health retires of xline client
    is_healthy_retries: usize,
    /// The detailed xline process handle
    inner: Box<dyn operator_api::XlineHandle>,
}

impl XlineHandle {
    /// Create the xline handle but not start the xline node
    pub(crate) fn open(
        name: &str,
        backup: Option<Box<dyn Provider>>,
        inner: Box<dyn operator_api::XlineHandle>,
        xline_port: u16,
        xline_members: HashMap<String, String>,
    ) -> Result<Self> {
        debug!("name: {name}, backup: {backup:?}, xline_port: {xline_port}");
        let endpoint: Endpoint = format!("http://127.0.0.1:{xline_port}").parse()?;
        let channel = Channel::balance_list(std::iter::once(endpoint));
        let health_client = HealthClient::new(channel);
        let engine = Engine::new(EngineType::Rocks(DEFAULT_DATA_DIR.parse()?), &XLINE_TABLES)?;
        Ok(Self {
            name: name.to_owned(),
            backup,
            health_client,
            engine,
            client: None, // TODO maybe we could initialize the client here when xline#423 is merged
            xline_members,
            is_healthy_retries: 5,
            inner,
        })
    }

    /// Return the xline client
    fn client(&self) -> Client {
        self.client
            .clone()
            .unwrap_or_else(|| panic!("xline client not initialized"))
    }

    /// Start the xline server
    pub(crate) async fn start(&mut self) -> Result<()> {
        // Step 1: Check if there is any node running
        // Step 2: If there is no node running, start single node cluster
        // Step 3: If there are some nodes running, start the node as a member to join the cluster
        let endpoints = self
            .xline_members
            .values()
            .map(|addr| {
                Ok::<_, tonic::transport::Error>(
                    Endpoint::from_shared(addr.clone())?.connect_timeout(Duration::from_secs(3)),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let futs: FuturesUnordered<_> = endpoints.iter().map(Endpoint::connect).collect();
        // the cluster is started if any of the connection is successful
        let cluster_started = futs.any(|res| async move { res.is_ok() }).await;

        self.inner.start().await?;

        let client = Client::connect(self.xline_members.values(), ClientOptions::default()).await?;
        if cluster_started {
            let _cluster_client = client.cluster_client();
            // send membership change here
        }
        let _ig = self.client.replace(client);
        Ok(())
    }

    /// Stop the xline server
    pub(crate) async fn stop(&mut self) -> Result<()> {
        // Step 1: Kill the xline node
        // Step 2: Remove the xline node from the cluster if the cluster exist
        self.inner.kill().await?;

        if self.is_healthy().await {
            let _cluster_client = self.client().cluster_client();
            // send membership change here
        }
        Ok(())
    }

    /// Return the xline cluster health by sending kv requests
    pub(crate) async fn is_healthy(&self) -> bool {
        let client = self.client().kv_client();
        for _ in 0..self.is_healthy_retries {
            // send linearized request to check if the xline server is healthy
            if client
                .range(RangeRequest::new("health").with_serializable(false))
                .await
                .is_ok()
            {
                return true;
            }
        }
        false
    }

    /// Return the xline server running state by sending a `gRPC` health request
    pub(crate) async fn is_running(&self) -> bool {
        let mut client = self.health_client.clone();
        let resp = client
            .check(HealthCheckRequest {
                service: String::new(), // do not match specific service
            })
            .await;
        match resp {
            Ok(resp) => resp.into_inner().status == i32::from(ServingStatus::Serving),
            Err(_) => false,
        }
    }

    /// Return the xline server kv revision if the server is online
    pub(crate) async fn revision_online(&self) -> Result<i64> {
        let client = self.client().kv_client();
        let response = client.range(RangeRequest::new(vec![])).await?;
        let header = response
            .header
            .ok_or(anyhow!("no header found in response"))?;
        Ok(header.revision)
    }

    /// Return the remote revision if the backup is specified and at least one backup is found
    pub(crate) async fn revision_remote(&self) -> Result<Option<i64>> {
        let backup = match self.backup.as_ref() {
            None => return Ok(None),
            Some(backup) => backup,
        };
        Ok(backup.latest().await?.map(|metadata| metadata.revision))
    }

    /// Return the xline server kv revision if the server is offline
    /// This is very useful for restoring a stopped xline server by comparing it's revision
    /// and the remote revision to prevent overriding the latest data
    /// NOTICE: This can only be used when the xline server is stopped, otherwise this may result in
    /// a race condition if we get the revision while xline is running.
    pub(crate) fn revision_offline(&self) -> Result<i64> {
        // Let caller to promise it
        // if self.is_running().await {
        //     return Err(anyhow!(
        //         "the xline server is running, cannot parse revision from data directory"
        //     ));
        // }
        let kvs = self.engine.get_all(KV_TABLE)?;
        let current_rev = kvs.last().map_or(1, |pair| pair.0.as_slice().get_i64());
        Ok(current_rev)
    }

    /// Backup snapshot
    pub(crate) async fn backup(&self) -> Result<()> {
        // Step 1. Get the remote backup snapshot revision
        // Step 2. Compare with the local revision (online)
        //         If the local revision is less than remote, abort backup
        // Step 3. Start backup
        let backup = match self.backup.as_ref() {
            None => return Err(anyhow!("no backup specified")),
            Some(backup) => backup,
        };
        let remote = backup.latest().await?.map(|metadata| metadata.revision);
        let local = self.revision_online().await?;
        if let Some(remote) = remote {
            if local < remote {
                // If the current local revision is less than remote, abort backup
                // return Ok here to prevent CronJob from retrying backup
                return Ok(());
            }
        }
        let mut client = self.client().maintenance_client();
        // The reason for using xline-client to take a snapshot instead of directly
        // reading the data-dir with rocksdb is to prevent race condition.
        let stream = client.snapshot().await?;
        backup
            .save(
                stream,
                &Metadata {
                    name: self.name.clone(),
                    revision: local,
                },
            )
            .await?;
        Ok(())
    }
}
