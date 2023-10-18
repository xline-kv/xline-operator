use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::routing::any;
use axum::routing::post;
use axum::{Extension, Router};
use flume::Sender;
use futures::FutureExt;
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::{DynamicObject, ListParams, Patch, PatchParams};
use kube::core::crd::merge_crds;
use kube::runtime::wait::{await_condition, conditions};
use kube::{Api, Client, CustomResourceExt, Resource};
use operator_api::HeartbeatStatus;
use prometheus::Registry;
use tokio::signal;
use tracing::{debug, info, warn};

use crate::config::{Config, Namespace};
use crate::consts::FIELD_MANAGER;
use crate::controller::cluster::{ClusterController, ClusterMetrics};
use crate::controller::{Controller, Metrics};
use crate::crd::version::ApiVersion;
use crate::crd::Cluster;
use crate::router::{healthz, metrics, sidecar_state};
use crate::sidecar_state::SidecarState;

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
        let (cluster_api, pod_api): (Api<Cluster>, Api<Pod>) = match self.config.namespace {
            Namespace::Single(ref namespace) => (
                Api::namespaced(kube_client.clone(), namespace.as_str()),
                Api::namespaced(kube_client.clone(), namespace.as_str()),
            ),
            Namespace::ClusterWide => {
                (Api::all(kube_client.clone()), Api::all(kube_client.clone()))
            }
        };
        let (status_tx, status_rx) = flume::unbounded();
        let graceful_shutdown_event = event_listener::Event::new();
        let forceful_shutdown = async {
            info!("press ctrl+c to shut down gracefully");
            let _ctrl_c = tokio::signal::ctrl_c().await;
            graceful_shutdown_event.notify(usize::MAX);
            info!("graceful shutdown already requested, press ctrl+c again to force shut down");
            let _ctrl_c_c = tokio::signal::ctrl_c().await;
        };

        let state_update_task = SidecarState::new(
            status_rx,
            self.config.heartbeat_period,
            cluster_api.clone(),
            pod_api,
            self.config.unreachable_thresh,
        )
        .run_with_graceful_shutdown(graceful_shutdown_event.listen());

        let metrics = ClusterMetrics::new();
        let registry = Registry::new();
        metrics.register(&registry)?;
        let controller = Arc::new(ClusterController {
            kube_client,
            cluster_suffix: self.config.cluster_suffix.clone(),
            metrics,
        });
        let mut controller = ClusterController::run(controller, cluster_api);

        let web_server = self.web_server(status_tx, registry);

        tokio::pin!(forceful_shutdown);
        tokio::pin!(web_server);
        tokio::pin!(state_update_task);

        let mut web_server_shutdown = false;
        let mut controller_shutdown = false;
        let mut state_update_shutdown = false;

        #[allow(clippy::integer_arithmetic)] // required by tokio::select
        loop {
            tokio::select! {
                _ = &mut forceful_shutdown => {
                    warn!("forceful shutdown");
                    break
                }
                res = &mut state_update_task, if !state_update_shutdown => {
                    res?;
                    state_update_shutdown = true;
                    info!("state update task graceful shutdown");
                }
                res = &mut web_server, if !web_server_shutdown => {
                    res?;
                    web_server_shutdown = true;
                    info!("web server graceful shutdown");
                }
                _ = &mut controller, if !controller_shutdown => {
                    controller_shutdown = true;
                    info!("controller graceful shutdown");
                }
            }

            if web_server_shutdown && controller_shutdown && state_update_shutdown {
                break;
            }
        }

        Ok(())
    }

    /// Wait for CRD to be established
    async fn wait_crd_established(
        crd_api: Api<CustomResourceDefinition>,
        crd_name: &str,
    ) -> Result<()> {
        let establish = await_condition(crd_api, crd_name, conditions::is_crd_established());
        debug!("wait for crd established");
        _ = tokio::time::timeout(CRD_ESTABLISH_TIMEOUT, establish).await??;
        Ok(())
    }

    /// Prepare CRD
    /// This method attempts to initialize the CRD if it does not already exist.
    /// Additionally, it could migrate CRD with the version of `CURRENT_VERSION`.
    async fn prepare_crd(&self, kube_client: &Client) -> Result<()> {
        let crd_api: Api<CustomResourceDefinition> = Api::all(kube_client.clone());
        let definition = Cluster::crd();
        let current_version: ApiVersion<Cluster> = Cluster::version(&()).as_ref().parse()?;

        let ret = crd_api.get(Cluster::crd_name()).await;
        if let Err(kube::Error::Api(kube::error::ErrorResponse { code: 404, .. })) = ret {
            if !self.config.create_crd {
                return Err(anyhow::anyhow!(
                    "cannot found XlineCluster CRD, please set --create-crd to true or apply the CRD manually"
                ));
            }
            // the following code needs `customresourcedefinitions` write permission
            debug!("cannot found XlineCluster CRD, try to init it");
            _ = crd_api
                .patch(
                    Cluster::crd_name(),
                    &PatchParams::apply(FIELD_MANAGER),
                    &Patch::Apply(definition.clone()),
                )
                .await?;
            Self::wait_crd_established(crd_api.clone(), Cluster::crd_name()).await?;
            return Ok(());
        }

        debug!("found XlineCluster CRD, current version: {current_version}");

        let mut add = true;
        let mut storage = String::new();

        let mut crds = ret?
            .spec
            .versions
            .iter()
            .cloned()
            .map(|ver| {
                let mut crd = definition.clone();
                if ver.name == current_version.to_string() {
                    add = false;
                }
                if ver.storage {
                    storage = ver.name.clone();
                }
                crd.spec.versions = vec![ver];
                crd
            })
            .collect::<Vec<_>>();

        if add {
            crds.push(definition.clone());
        } else {
            debug!("current version already exists, try to migrate");
            self.try_migration(kube_client, crds, &current_version, &storage)
                .await?;
            return Ok(());
        }

        if !self.config.create_crd {
            return Err(anyhow::anyhow!(
                "cannot found XlineCluster CRD with version {current_version}, please set --create-crd to true or apply the CRD manually"
            ));
        }

        let merged_crd = merge_crds(crds.clone(), &storage)?;
        debug!("try to update crd");
        _ = crd_api
            .patch(
                Cluster::crd_name(),
                &PatchParams::apply(FIELD_MANAGER),
                &Patch::Apply(merged_crd),
            )
            .await?;
        Self::wait_crd_established(crd_api.clone(), Cluster::crd_name()).await?;

        debug!("crd updated, try to migrate");
        self.try_migration(kube_client, crds, &current_version, &storage)
            .await?;

        Ok(())
    }

    /// Try to migrate CRD
    #[allow(clippy::indexing_slicing)] // there is at least one element in `versions`
    #[allow(clippy::expect_used)]
    async fn try_migration(
        &self,
        kube_client: &Client,
        crds: Vec<CustomResourceDefinition>,
        current_version: &ApiVersion<Cluster>,
        storage: &str,
    ) -> Result<()> {
        if !self.config.auto_migration {
            debug!("auto migration is disabled, skip migration");
            return Ok(());
        }
        if current_version.to_string() == storage {
            // stop migration if current version is already in storage
            debug!("current version is already in storage, skip migration");
            return Ok(());
        }
        let versions: Vec<ApiVersion<Cluster>> = crds
            .iter()
            .map(|crd| crd.spec.versions[0].name.parse())
            .collect::<Result<_>>()?;
        if versions.iter().any(|ver| current_version < ver) {
            // stop migration if current version is less than any version in `versions`
            debug!("current version is less than some version in crd, skip migration");
            return Ok(());
        }
        let group = kube::discovery::group(kube_client, Cluster::group(&()).as_ref()).await?;
        let Some((ar, _)) = group
            .versioned_resources(storage)
            .into_iter()
            .find(|res| res.0.kind == Cluster::kind(&())) else { return Ok(()) };
        let api: Api<DynamicObject> = Api::all_with(kube_client.clone(), &ar);
        let clusters = api.list(&ListParams::default()).await?.items;
        if !clusters.is_empty() && !current_version.compat_with(&storage.parse()?) {
            // there is some clusters with storage version and is not compat with current version, stop migration
            // TODO add a flag to these clusters to indicate that they need to be migrated
            return Ok(());
        }
        // start migration as there is no cluster with storage version
        let merged_crd = merge_crds(crds, &current_version.to_string())?;
        let crd_api: Api<CustomResourceDefinition> = Api::all(kube_client.clone());
        debug!("try to migrate crd from {storage} to {current_version}");
        _ = crd_api
            .patch(
                Cluster::crd_name(),
                &PatchParams::apply(FIELD_MANAGER),
                &Patch::Apply(merged_crd),
            )
            .await?;
        Self::wait_crd_established(crd_api.clone(), Cluster::crd_name()).await?;
        Ok(())
    }

    /// Run a server that receive sidecar operators' status
    async fn web_server(
        &self,
        status_tx: Sender<HeartbeatStatus>,
        registry: Registry,
    ) -> Result<()> {
        let status = Router::new()
            .route("/status", post(sidecar_state))
            .route("/metrics", any(metrics))
            .route("/healthz", any(healthz))
            .layer(Extension(status_tx))
            .layer(Extension(registry));

        axum::Server::bind(&self.config.listen_addr.parse()?)
            .serve(status.into_make_service())
            .with_graceful_shutdown(signal::ctrl_c().map(|_| ()))
            .await?;

        Ok(())
    }
}
