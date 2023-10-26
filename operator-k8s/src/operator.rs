use std::sync::Arc;

use anyhow::Result;
use axum::routing::any;
use axum::routing::post;
use axum::{Extension, Router};
use futures::FutureExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, Client};
use operator_api::consts::OPERATOR_MONITOR_ROUTE;
use operator_api::HeartbeatStatus;
use prometheus::Registry;
use tokio::sync::watch::{Receiver, Sender};
use tracing::{error, info, warn};

use crate::config::{Config, Namespace};
use crate::controller::cluster::{ClusterController, ClusterMetrics};
use crate::controller::{Controller, Metrics};
use crate::crd::Cluster;
use crate::monitor::SidecarMonitor;
use crate::router::{healthz, metrics, sidecar_monitor};

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
        crate::crd::setup(
            &kube_client,
            self.config.manage_crd,
            self.config.auto_migration,
        )
        .await?;
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
            self.config.unreachable_thresh,
            cluster_api,
            pod_api,
        );

        let _ig = tokio::spawn(async move {
            let mut shutdown = graceful_shutdown;
            let res = monitor
                .run_with_graceful_shutdown(shutdown.changed().map(|_| ()))
                .await;
            if let Err(err) = res {
                error!("monitor run failed, error: {err}");
            } else {
                info!("sidecar monitor shutdown");
            }
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
            .route(OPERATOR_MONITOR_ROUTE, post(sidecar_monitor))
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
            } else {
                info!("web server shut down");
            }
        });

        Ok(())
    }
}
