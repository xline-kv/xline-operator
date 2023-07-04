#![allow(dead_code)] // remove when it is implemented

use crate::config::Config;
use crate::controller::{on_error, reconcile, Context};
use crate::crd::v1::Cluster;
use anyhow::Result;
use futures::StreamExt;
use kube::runtime::watcher::Config as WatcherConfig;
use kube::runtime::Controller;
use kube::{Api, Client};
use std::sync::Arc;

/// Deployment Operator for k8s
#[derive(Debug)]
pub struct Operator {
    /// Config of this operator
    config: Config,
}

impl Operator {
    /// Constructor
    #[inline]
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Run operator
    ///
    /// # Errors
    ///
    /// Return `Err` when run failed
    #[inline]
    pub async fn run(&self) -> Result<()> {
        let kube_client: Client = Client::try_default().await?;

        let crd_api: Api<Cluster> = Api::all(kube_client.clone());
        let cx: Arc<Context> = Arc::new(Context { kube_client });

        Controller::new(crd_api.clone(), WatcherConfig::default())
            .shutdown_on_signal()
            .run(reconcile, on_error, cx)
            .filter_map(|x| async move { x.ok() })
            .for_each(|_| futures::future::ready(()))
            .await;
        Ok(())
    }
}
