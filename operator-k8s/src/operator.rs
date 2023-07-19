use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::routing::post;
use axum::{Json, Router};
use flume::Sender;
use futures::FutureExt;
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::{ListParams, Patch, PatchParams, PostParams};
use kube::runtime::wait::{await_condition, conditions};
use kube::{Api, Client, CustomResourceExt, Resource};
use operator_api::HeartbeatStatus;
use tokio::signal;
use tracing::{debug, error, info, warn};
use utils::migration::ApiVersion;

use crate::config::{Config, Namespace};
use crate::controller::cluster::Controller as ClusterController;
use crate::controller::{Context, Controller};
use crate::crd::Cluster;
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

        let web_server = self.web_server(status_tx);

        let state_update_task = SidecarState::new(
            status_rx,
            self.config.heartbeat_period,
            cluster_api.clone(),
            pod_api,
            self.config.unreachable_thresh,
        )
        .run_with_graceful_shutdown(graceful_shutdown_event.listen());

        let ctx = Arc::new(Context::new(ClusterController {
            kube_client,
            cluster_suffix: self.config.cluster_suffix.clone(),
        }));
        let mut controller = ClusterController::run(ctx, cluster_api);

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

    /// Run a server that receive sidecar operators' status
    async fn web_server(&self, status_tx: Sender<HeartbeatStatus>) -> Result<()> {
        let status = Router::new().route(
            "/status",
            post(|body: Json<HeartbeatStatus>| async move {
                if let Err(e) = status_tx.send(body.0) {
                    error!("channel send error: {e}");
                }
            }),
        );

        axum::Server::bind(&self.config.listen_addr.parse()?)
            .serve(status.into_make_service())
            .with_graceful_shutdown(signal::ctrl_c().map(|_| ()))
            .await?;

        Ok(())
    }
}
