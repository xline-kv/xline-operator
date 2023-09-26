use std::net::ToSocketAddrs;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use axum::routing::{get, post};
use axum::{Extension, Router};
use futures::{FutureExt, TryFutureExt};
use tokio::sync::watch::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::backup::pv::Pv;
use crate::backup::Provider;
use crate::controller::Controller;
use crate::controller::Error;
use crate::routers;
use crate::types::{Backup, Config, State, StatePayload};
use crate::xline::XlineHandle;

/// Sidecar operator
#[derive(Debug)]
pub struct Operator {
    /// Operator config
    config: Config,
}

impl Operator {
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
        let backup = self.init_backup();
        let handle = Arc::new(XlineHandle::open(
            &self.config.name,
            &self.config.container_name,
            backup,
            self.config.xline_port,
            self.config.xline_members(),
        )?);
        let revision = handle.revision_offline().unwrap_or(1);
        let state = Arc::new(Mutex::new(StatePayload {
            state: State::Start,
            revision,
        }));

        self.start_controller(
            Arc::clone(&handle),
            Arc::clone(&state),
            graceful_shutdown_event.subscribe(),
        );
        self.start_web_server(
            Arc::clone(&handle),
            Arc::clone(&state),
            graceful_shutdown_event.subscribe(),
        )?;

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

    /// Initialize backup
    fn init_backup(&self) -> Option<Box<dyn Provider>> {
        self.config
            .backup
            .as_ref()
            .and_then(|backup| match *backup {
                Backup::S3 { .. } => None, // TODO S3 backup
                Backup::PV { ref path } => {
                    let pv: Box<dyn Provider> = Box::new(Pv {
                        backup_path: path.clone(),
                    });
                    Some(pv)
                }
            })
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
        handle: Arc<XlineHandle>,
        state: Arc<Mutex<StatePayload>>,
        graceful_shutdown: Receiver<()>,
    ) {
        let mut controller = Controller::new(
            handle,
            interval(self.config.check_interval),
            state,
            graceful_shutdown,
        );
        let _ig = tokio::spawn(async move {
            loop {
                match controller.reconcile_once().await {
                    Ok(instant) => {
                        debug!(
                            "successfully reconcile the cluster states within {:?}",
                            instant.elapsed()
                        );
                    }
                    Err(err) => {
                        if err == Error::Shutdown {
                            info!("controller graceful shutdown");
                            break;
                        }
                        error!("reconcile failed, error: {}", err);
                    }
                }
            }
        });
    }

    /// Run a web server to expose current state to other sidecar operators and k8s
    fn start_web_server(
        &self,
        handle: Arc<XlineHandle>,
        state: Arc<Mutex<StatePayload>>,
        graceful_shutdown: Receiver<()>,
    ) -> Result<()> {
        let members = self.config.operator_members();
        let advertise_url = members.get(&self.config.name).ok_or(anyhow!(
            "node name {} not found in members",
            self.config.name
        ))?;
        let addr = advertise_url.to_socket_addrs()?.next().ok_or(anyhow!(
            "the advertise_url in members: {advertise_url} is invalid"
        ))?;

        let app = Router::new()
            .route("/health", get(routers::health))
            .route("/backup", get(routers::backup))
            .route("/state", get(routers::state))
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
