use std::net::ToSocketAddrs;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use axum::routing::{get, post};
use axum::{Extension, Router};
use futures::{FutureExt, TryFutureExt};
use operator_api::consts::{SIDECAR_BACKUP_ROUTE, SIDECAR_HEALTH_ROUTE, SIDECAR_STATE_ROUTE};
use operator_api::registry::{DummyRegistry, HttpRegistry, K8sStsRegistry, Registry};
use operator_api::{HeartbeatStatus, K8sXlineHandle, LocalXlineHandle};
use tokio::sync::watch::{Receiver, Sender};
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, MissedTickBehavior};
use tracing::{debug, error, info, warn};

use crate::backup::pv::Pv;
use crate::backup::Provider;
use crate::controller::Controller;
use crate::routers;
use crate::types::{BackendConfig, BackupConfig, Config, RegistryConfig, State, StatePayload};
use crate::xline::XlineHandle;

/// Sidecar
#[derive(Debug)]
pub struct Sidecar {
    /// Operator config
    config: Config,
}

impl Sidecar {
    /// Constructor
    #[must_use]
    #[inline]
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Run operator
    ///
    /// # Errors
    /// Return Err when config is invalid
    #[inline]
    pub async fn run(&self) -> Result<()> {
        let (graceful_shutdown_event, _) = tokio::sync::watch::channel(());
        let forceful_shutdown = self.forceful_shutdown(&graceful_shutdown_event);
        let handle = Arc::new(RwLock::new(self.init_xline_handle().await?));
        let revision = handle.read().await.revision_offline().unwrap_or(1);
        let state = Arc::new(Mutex::new(StatePayload {
            state: State::Start,
            revision,
        }));
        let registry: Arc<dyn Registry + Send + Sync> = match self.config.registry.clone() {
            None => Arc::new(DummyRegistry::new(self.config.init_member.members.clone())),
            Some(RegistryConfig::Sts { name, namespace }) => Arc::new(
                K8sStsRegistry::new_with_default(name, namespace, "cluster.local".to_owned()).await,
            ),
            Some(RegistryConfig::Http { server_addr }) => Arc::new(HttpRegistry::new(
                server_addr,
                self.config.cluster_name.clone(),
            )),
        };

        self.start_controller(
            Arc::clone(&handle),
            Arc::clone(&registry),
            Arc::clone(&state),
            graceful_shutdown_event.subscribe(),
        );
        self.start_web_server(
            Arc::clone(&handle),
            Arc::clone(&state),
            graceful_shutdown_event.subscribe(),
        )?;
        self.start_heartbeat(registry, graceful_shutdown_event.subscribe())?;

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

    /// Initialize xline handle
    async fn init_xline_handle(&self) -> Result<XlineHandle> {
        let backup = self
            .config
            .backup
            .as_ref()
            .and_then(|backup| match *backup {
                BackupConfig::S3 { .. } => None, // TODO S3 backup
                BackupConfig::PV { ref path } => {
                    let pv: Box<dyn Provider> = Box::new(Pv {
                        backup_path: path.clone(),
                    });
                    Some(pv)
                }
            });
        let inner: Box<dyn operator_api::XlineHandle> = match self.config.backend.clone() {
            BackendConfig::K8s {
                pod_name,
                container_name,
                namespace,
            } => {
                let handle = K8sXlineHandle::new_with_default(
                    pod_name,
                    container_name,
                    &namespace,
                    self.config.xline.clone(),
                )
                .await;
                Box::new(handle)
            }
            BackendConfig::Local => {
                let handle = LocalXlineHandle::new(self.config.xline.clone());
                Box::new(handle)
            }
        };
        XlineHandle::open(
            &self.config.name,
            &self.config.xline.data_dir,
            backup,
            inner,
            self.config.init_member.xline_port,
        )
    }

    /// Forceful shutdown
    async fn forceful_shutdown(&self, graceful_shutdown_event: &Sender<()>) {
        info!("press ctrl+c to shut down gracefully");
        let _ctrl_c = tokio::signal::ctrl_c().await;
        let _ig = graceful_shutdown_event.send(());
        info!("graceful shutdown already requested to {} components, press ctrl+c again to force shut down", graceful_shutdown_event.receiver_count());
        let _ctrl_c_c = tokio::signal::ctrl_c().await;
    }

    /// Start heartbeat
    fn start_heartbeat(
        &self,
        registry: Arc<dyn Registry + Send + Sync>,
        graceful_shutdown: Receiver<()>,
    ) -> Result<()> {
        let Some(monitor) = self.config.monitor.clone() else {
            info!("monitor did not set, disable heartbeat");
            return Ok(());
        };
        let cluster_name = self.config.cluster_name.clone();
        let name = self.config.name.clone();
        let self_host = self
            .config
            .init_member
            .get_host(&name)
            .ok_or_else(|| anyhow!("node name {} not found in initial members", &name))?
            .clone();
        let mut member_config = self.config.init_member.clone();

        #[allow(clippy::integer_arithmetic)] // this error originates in the macro `tokio::select`
        let _ig = tokio::spawn(async move {
            let mut shutdown = graceful_shutdown;

            let heartbeat_task = async move {
                let mut tick = interval(monitor.heartbeat_interval);
                // ensure a fixed heartbeat interval
                tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
                loop {
                    let instant = tick.tick().await;

                    let config = match registry
                        .wait_full_fetch(name.clone(), self_host.clone())
                        .await
                    {
                        Ok(c) => c,
                        Err(err) => {
                            error!("fetch config failed, error {err}");
                            continue;
                        }
                    };
                    member_config.members = config.members;

                    debug!("send heartbeat at {instant:?}");
                    let status = HeartbeatStatus::gather(
                        cluster_name.clone(),
                        name.clone(),
                        &member_config.sidecar_members(),
                    )
                    .await;

                    debug!("sidecar gathered status: {status:?}");
                    if let Err(e) = status.report(&monitor.monitor_addr).await {
                        error!("heartbeat report failed, error {e}");
                    }
                }
            };

            tokio::select! {
                _ = shutdown.changed() => {
                    info!("heartbeat task graceful shutdown");
                },
                _ = heartbeat_task => {}
            }
        });

        Ok(())
    }

    /// Start controller
    fn start_controller(
        &self,
        handle: Arc<RwLock<XlineHandle>>,
        registry: Arc<dyn Registry + Sync + Send>,
        state: Arc<Mutex<StatePayload>>,
        graceful_shutdown: Receiver<()>,
    ) {
        let controller = Controller::new(
            self.config.name.clone(),
            state,
            handle,
            self.config.reconcile_interval,
            registry,
        );
        let init_member_config = self.config.init_member.clone();
        let _ig = tokio::spawn(async move {
            let mut shutdown = graceful_shutdown;
            let res = controller
                .run_reconcile_with_shutdown(init_member_config, shutdown.changed().map(|_| ()))
                .await;
            if let Err(err) = res {
                error!("controller run failed, error: {err}");
            } else {
                info!("controller shutdown");
            }
        });
    }

    /// Run a web server to expose current state to other sidecar operators and k8s
    fn start_web_server(
        &self,
        handle: Arc<RwLock<XlineHandle>>,
        state: Arc<Mutex<StatePayload>>,
        graceful_shutdown: Receiver<()>,
    ) -> Result<()> {
        let members = self.config.init_member.sidecar_members();
        let advertise_url = members.get(&self.config.name).ok_or(anyhow!(
            "node name {} not found in members",
            self.config.name
        ))?;
        let addr = advertise_url.to_socket_addrs()?.next().ok_or(anyhow!(
            "the advertise_url in members: {advertise_url} is invalid"
        ))?;

        let app = Router::new()
            .route(SIDECAR_HEALTH_ROUTE, get(routers::health))
            .route(SIDECAR_BACKUP_ROUTE, get(routers::backup))
            .route(SIDECAR_STATE_ROUTE, get(routers::state))
            .route("/membership", post(routers::membership))
            .layer(Extension(handle))
            .layer(Extension(state));

        debug!("web server listen addr: {addr}");

        let _ig = tokio::spawn(async move {
            let mut graceful_shutdown = graceful_shutdown;
            let res = axum::Server::bind(&addr)
                .serve(app.into_make_service())
                .with_graceful_shutdown(graceful_shutdown.changed().map(|_| ()))
                .map_err(anyhow::Error::from)
                .await;
            if let Err(e) = res {
                error!("web server error: {e}");
            } else {
                info!("web server graceful shutdown");
            }
        });

        Ok(())
    }
}
