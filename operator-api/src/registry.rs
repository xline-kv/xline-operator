#![allow(unused)] // TODO remove

use anyhow::anyhow;
use async_trait::async_trait;
use crd_api::Cluster;
use k8s_openapi::serde_json::json;
use kube::api::{Patch, PatchParams};
use kube::core::object::HasStatus;
use kube::{Api, Client};

use std::collections::HashMap;
use std::net::ToSocketAddrs;

pub struct Config {
    /// Members [node_name] => [node_ip]
    pub members: HashMap<String, String>,
    /// Cluster size
    pub cluster_size: usize,
}

#[async_trait]
pub trait Registry {
    const FIELD_MANAGER: &'static str = "xlineoperator.datenlord.io/registry";

    async fn send_fetch(&self, self_name: String, self_ip: String) -> anyhow::Result<Config>;
}

/// K8s custom resource `Cluster` status registry
pub struct K8sClusterStatusRegistry {
    cluster_name: String,
    cluster_api: Api<Cluster>,
}

impl K8sClusterStatusRegistry {
    /// New a registry with default kube client
    pub async fn new_with_default(cluster_name: String, namespace: &str) -> Self {
        let kube_client = Client::try_default()
            .await
            .unwrap_or_else(|_ig| unreachable!("it must be setup in k8s environment"));
        Self {
            cluster_name,
            cluster_api: Api::namespaced(kube_client, namespace),
        }
    }

    pub async fn new(cluster_name: String, namespace: &str, kube_client: Client) -> Self {
        Self {
            cluster_name,
            cluster_api: Api::namespaced(kube_client, namespace),
        }
    }
}

#[async_trait]
impl Registry for K8sClusterStatusRegistry {
    async fn send_fetch(&self, self_name: String, self_ip: String) -> anyhow::Result<Config> {
        /// TODO: hold a distributed lock here
        let cluster = self.cluster_api.get_status(&self.cluster_name).await?;
        let mut status = cluster
            .status
            .ok_or_else(|| anyhow!("no status found in cluster {}", self.cluster_name))?;

        // dns may not change, but ip can
        let ip = format!("{self_ip}:0")
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow!("cannot resolve dns {self_ip}"))?
            .ip();

        if status.members.get(&self_name) != Some(&ip) {
            status.members.insert(self_name, ip);
            let patch = json!({
                "status": status,
            });
            let _ig = self
                .cluster_api
                .patch_status(
                    &self.cluster_name,
                    &PatchParams::apply(Self::FIELD_MANAGER),
                    &Patch::Apply(patch),
                )
                .await?;
        }

        Ok(Config {
            members: status
                .members
                .into_iter()
                .map(|(k, v)| (k, v.to_string()))
                .collect(),
            cluster_size: cluster.spec.size,
        })
    }
}
