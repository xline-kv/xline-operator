use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use crate::controller::{on_error, reconcile, Context};
use crate::crd::Cluster;
use crate::utils::compare_versions;

use anyhow::Result;
use futures::StreamExt;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::{ListParams, Patch, PatchParams, PostParams};
use kube::runtime::watcher::Config as WatcherConfig;
use kube::runtime::Controller;
use kube::{Api, Client, CustomResourceExt, Resource};
use tracing::debug;

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
        self.prepare_crd(&kube_client).await?;
        let cluster_api: Api<Cluster> = if self.config.cluster_wide {
            Api::all(kube_client.clone())
        } else {
            Api::namespaced(kube_client.clone(), self.config.namespace.as_str())
        };
        let cx: Arc<Context> = Arc::new(Context { kube_client });

        Controller::new(cluster_api.clone(), WatcherConfig::default())
            .shutdown_on_signal()
            .run(reconcile, on_error, cx)
            .filter_map(|x| async move { x.ok() })
            .for_each(|_| futures::future::ready(()))
            .await;
        Ok(())
    }

    /// Prepare CRD
    /// This method attempts to initialize the CRD if it does not already exist.
    /// Additionally, it could migrate CRD with the version of `CURRENT_VERSION`.
    async fn prepare_crd(&self, kube_client: &Client) -> Result<()> {
        let crd_api: Api<CustomResourceDefinition> = Api::all(kube_client.clone());
        let crds: HashMap<_, _> = crd_api
            .list(&ListParams::default())
            .await?
            .items
            .into_iter()
            .filter_map(|crd| crd.metadata.name.map(|name| (name, crd.spec.versions)))
            .collect();

        let definition = Cluster::crd();

        match crds.get(Cluster::crd_name()) {
            None => {
                // cannot find crd name, initial CRD
                debug!("cannot found XlineCluster CRD, try to init it");
                let _crd = crd_api.create(&PostParams::default(), &definition).await?;
            }
            Some(versions) => {
                let current_version = Cluster::version(&());
                debug!("found XlineCluster CRD, current version {current_version}",);

                if versions.iter().all(|ver| {
                    matches!(
                        compare_versions(current_version.as_ref(), ver.name.as_str()),
                        Ok(Ordering::Greater),
                    )
                }) {
                    debug!("{current_version} is larger than all version on k8s, patch to latest");
                    let _crd = crd_api
                        .patch(
                            Cluster::crd_name(),
                            &PatchParams::default(),
                            &Patch::Merge(definition),
                        )
                        .await?;
                    return Ok(());
                }

                if !self.config.create_crd
                    && versions.iter().any(|ver| {
                        matches!(
                            compare_versions(ver.name.as_str(), current_version.as_ref()),
                            Ok(Ordering::Greater),
                        )
                    })
                {
                    panic!(
                        "The current XlineCluster CRD version {current_version} is not compatible with higher version on k8s. Please use the latest deployment operator or set --create_crd to true."
                    );
                }

                if self.config.create_crd {
                    debug!("create_crd set to true, force patch this CRD");
                    let _crd = crd_api
                        .patch(
                            Cluster::crd_name(),
                            &PatchParams::default(),
                            &Patch::Merge(definition),
                        )
                        .await?;
                }
            }
        }

        Ok(())
    }
}
