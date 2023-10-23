use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::routing::any;
use axum::routing::post;
use axum::{Extension, Router};
use futures::FutureExt;
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::{DynamicObject, ListParams, Patch, PatchParams};
use kube::core::crd::merge_crds;
use kube::runtime::wait::{await_condition, conditions};
use kube::{Api, Client, CustomResourceExt, Resource};
use operator_api::HeartbeatStatus;
use prometheus::Registry;
use tokio::sync::watch::{Receiver, Sender};
use tracing::{debug, error, info, warn};

use crate::config::{Config, Namespace};
use crate::consts::FIELD_MANAGER;
use crate::controller::cluster::{ClusterController, ClusterMetrics};
use crate::controller::{Controller, Metrics};
use crate::crd::version::ApiVersion;
use crate::crd::Cluster;
use crate::monitor::SidecarMonitor;
use crate::router::{healthz, metrics, sidecar_state};

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
        let (graceful_shutdown_event, _) = tokio::sync::watch::channel(());
        let forceful_shutdown = self.forceful_shutdown(&graceful_shutdown_event);
        let (status_tx, status_rx) = flume::unbounded();
        let registry = Registry::new();

        self.start_sidecar_monitor(
            status_rx,
            cluster_api.clone(),
            pod_api,
            graceful_shutdown_event.subscribe(),
        );
        self.start_controller(
            kube_client,
            cluster_api,
            &registry,
            graceful_shutdown_event.subscribe(),
        )?;
        self.start_web_server(status_tx, registry, graceful_shutdown_event.subscribe())?;

        tokio::pin!(forceful_shutdown);

        #[allow(clippy::integer_arithmetic)] // this error originates in the macro `tokio::select`
        {
            tokio::select! {
                _ = &mut forceful_shutdown => {
                    warn!("forceful shutdown");
                }
                _ = graceful_shutdown_event.closed() => {
                    info!("graceful shutdown");
                }
            }
        }

        Ok(())
    }

    /// Forceful shutdown
    async fn forceful_shutdown(&self, graceful_shutdown_event: &Sender<()>) {
        info!("press ctrl+c to shut down gracefully");
        let _ctrl_c = tokio::signal::ctrl_c().await;
        let _ig = graceful_shutdown_event.send(());
        info!("graceful shutdown already requested to {} components, press ctrl+c again to force shut down", graceful_shutdown_event.receiver_count());
        let _ctrl_c_c = tokio::signal::ctrl_c().await;
    }

    /// Start controller
    fn start_controller(
        &self,
        kube_client: Client,
        cluster_api: Api<Cluster>,
        registry: &Registry,
        graceful_shutdown: Receiver<()>,
    ) -> Result<()> {
        let metrics = ClusterMetrics::new();
        metrics.register(registry)?;

        let controller = Arc::new(ClusterController {
            kube_client,
            cluster_suffix: self.config.cluster_suffix.clone(),
            metrics,
        });
        #[allow(unsafe_code)] // safe
        let _ig = tokio::spawn(async move {
            let mut shutdown = graceful_shutdown;

            {
                // Safety:
                // Some hacking to make future generated from `graceful_shutdown` to be 'static
                // The 'static marker is required by `kube::runtime::Controller::graceful_shutdown_on`
                // and it is not good for our design.
                let shutdown_static: &'static mut Receiver<()> =
                    unsafe { std::mem::transmute(&mut shutdown) };
                ClusterController::run_with_shutdown(
                    controller,
                    cluster_api,
                    shutdown_static.changed().map(|_| ()),
                )
                .await;
            }

            // yes, we cheated the `ClusterController`, but now it is dead so we can safely dropped here
            drop(shutdown);
            info!("controller shutdown");
        });
        Ok(())
    }

    /// Start sidecar monitor
    fn start_sidecar_monitor(
        &self,
        status_rx: flume::Receiver<HeartbeatStatus>,
        cluster_api: Api<Cluster>,
        pod_api: Api<Pod>,
        graceful_shutdown: Receiver<()>,
    ) {
        let monitor = SidecarMonitor::new(
            status_rx,
            self.config.heartbeat_period,
            cluster_api,
            pod_api,
            self.config.unreachable_thresh,
        );

        let _ig = tokio::spawn(async move {
            let mut shutdown = graceful_shutdown;
            let res = monitor
                .run_with_graceful_shutdown(shutdown.changed().map(|_| ()))
                .await;
            if let Err(err) = res {
                error!("monitor run failed, error: {err}");
            }
            info!("sidecar monitor shutdown");
        });
    }

    /// Start web server
    fn start_web_server(
        &self,
        status_tx: flume::Sender<HeartbeatStatus>,
        registry: Registry,
        graceful_shutdown: Receiver<()>,
    ) -> Result<()> {
        let status = Router::new()
            .route("/status", post(sidecar_state))
            .route("/metrics", any(metrics))
            .route("/healthz", any(healthz))
            .layer(Extension(status_tx))
            .layer(Extension(registry));
        let server = axum::Server::bind(&self.config.listen_addr.parse()?);

        let _ig = tokio::spawn(async move {
            let mut shutdown = graceful_shutdown;
            let res = server
                .serve(status.into_make_service())
                .with_graceful_shutdown(shutdown.changed().map(|_| ()))
                .await;
            if let Err(err) = res {
                error!("web server starts failed, error: {err}");
            }
            info!("web server shut down");
        });

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
}
