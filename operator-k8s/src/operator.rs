use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::{ListParams, Patch, PatchParams, PostParams};
use kube::runtime::wait::{await_condition, conditions};
use kube::{Api, Client, CustomResourceExt, Resource};
use tracing::debug;
use utils::migration::ApiVersion;

use crate::config::Config;
use crate::controller::cluster::ClusterController;
use crate::controller::{Context, Controller};
use crate::crd::Cluster;

/// wait crd to establish timeout
const CRD_ESTABLISH_TIMEOUT: Duration = Duration::from_secs(20);

/// Xline Operator for k8s
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
        let ctx = Arc::new(Context::new(ClusterController {
            kube_client,
            cluster_api,
            cluster_suffix: self.config.cluster_suffix.clone(),
        }));
        ClusterController::run(ctx).await;
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
                debug!("found XlineCluster CRD, current version {current_version}");
                let current_version: ApiVersion<Cluster> = current_version.as_ref().parse()?;
                let versions: Vec<ApiVersion<Cluster>> = versions
                    .iter()
                    .map(|v| v.name.parse())
                    .collect::<Result<_>>()?;
                if versions.iter().all(|ver| &current_version > ver) {
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
                assert!(self.config.create_crd || !versions.iter().any(|ver| ver > &current_version), "The current XlineCluster CRD version {current_version} is not compatible with higher version on k8s. Please use the latest xline-operator or set --create_crd to true.");
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
        let establish = await_condition(
            crd_api,
            Cluster::crd_name(),
            conditions::is_crd_established(),
        );
        let _crd = tokio::time::timeout(CRD_ESTABLISH_TIMEOUT, establish).await??;
        debug!("crd established");
        Ok(())
    }
}
