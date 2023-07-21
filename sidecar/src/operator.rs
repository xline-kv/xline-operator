use std::cmp::max;
use std::future::Future;
use std::net::ToSocketAddrs;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use axum::routing::{get, post};
use axum::{Extension, Router};
use futures::TryFutureExt;
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
    ///
    /// Return Err when run failed
    #[inline]
    pub async fn run(&self) -> Result<()> {
        let graceful_shutdown_event = event_listener::Event::new();
        let forceful_shutdown = async {
            info!("press ctrl+c to shut down gracefully");
            let _ctrl_c = tokio::signal::ctrl_c().await;
            graceful_shutdown_event.notify(usize::MAX);
            info!("graceful shutdown already requested, press ctrl+c again to force shut down");
            let _ctrl_c_c = tokio::signal::ctrl_c().await;
        };
        let backup: Option<Box<dyn Provider>> =
            self.config.backup.clone().and_then(|backup| match backup {
                Backup::S3 { .. } => None, // TODO s3
                Backup::PV { path } => {
                    let pv: Box<dyn Provider> = Box::new(Pv { backup_path: path });
                    Some(pv)
                }
            });
        let handle = Arc::new(
            XlineHandle::open(
                &self.config.name,
                &self.config.container_name,
                backup,
                self.config.xline_members(),
                self.config.xline_port,
            )
            .await?,
        );
        let offline_rev = handle.revision_offline().unwrap_or(1);
        let remote_rev = handle.revision_remote().await?;
        let revision = match remote_rev {
            None => offline_rev,
            Some(remote_rev) => max(remote_rev, offline_rev),
        };
        let state = Arc::new(Mutex::new(StatePayload {
            state: State::Start,
            revision,
        }));
        let check_interval = interval(self.config.check_interval);
        let mut controller = Controller::new(
            Arc::clone(&handle),
            check_interval,
            Arc::clone(&state),
            graceful_shutdown_event.listen(),
        );
        let web_server = self.web_server(
            Arc::clone(&handle),
            Arc::clone(&state),
            graceful_shutdown_event.listen(),
        )?;

        tokio::pin!(forceful_shutdown);
        tokio::pin!(web_server);

        let mut controller_shutdown = false;
        let mut web_server_shutdown = false;

        #[allow(clippy::integer_arithmetic)] // this error originates in the macro `tokio::select`
        loop {
            tokio::select! {
                _ = &mut forceful_shutdown => {
                    warn!("forceful shutdown");
                    break
                }
                res = controller.reconcile_once(), if !controller_shutdown => {
                    match res {
                        Ok(instant) => {
                            debug!("successfully reconcile the cluster states within {:?}", instant.elapsed());
                        }
                        Err(err) => {
                            if err == Error::Shutdown {
                                info!("controller graceful shutdown");
                                controller_shutdown = true;
                            } else {
                                error!("reconcile failed, error: {}", err);
                            }
                        }
                    }
                }
                _ = &mut web_server, if !web_server_shutdown => {
                    info!("web server graceful shutdown");
                    web_server_shutdown = true;
                }
            }
            if controller_shutdown && web_server_shutdown {
                break;
            }
        }
        Ok(())
    }

    /// Run a web server to expose current state to other sidecar operators and k8s
    fn web_server(
        &self,
        handle: Arc<XlineHandle>,
        state: Arc<Mutex<StatePayload>>,
        graceful_shutdown: impl Future<Output = ()>,
    ) -> Result<impl Future<Output = Result<()>>> {
        let app = Router::new()
            .route("/health", get(routers::health))
            .route("/backup", get(routers::backup))
            .route("/state", get(routers::state))
            .route("/membership", post(routers::membership))
            .layer(Extension(handle))
            .layer(Extension(state));
        let members = self.config.operator_members();
        let advertise_url = members.get(&self.config.name).ok_or(anyhow!(
            "node name {} not found in members",
            self.config.name
        ))?;
        let addr = advertise_url.to_socket_addrs()?.next().ok_or(anyhow!(
            "the advertise_url in members: {advertise_url} is invalid"
        ))?;
        debug!("web server listen addr: {addr}");
        Ok(axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .with_graceful_shutdown(graceful_shutdown)
            .map_err(anyhow::Error::from))
    }
}
