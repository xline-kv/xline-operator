#![allow(dead_code)] // TODO remove when it is implemented

use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use operator_api::registry::Registry;
use tokio::select;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, MissedTickBehavior};
use tracing::{debug, error, info};

use crate::types::{MemberConfig, State, StatePayload, StateStatus};
use crate::xline::XlineHandle;

/// Sidecar operator controller
pub(crate) struct Controller {
    /// The name of this sidecar
    name: String,
    /// The state of this operator
    state: Arc<Mutex<StatePayload>>,
    /// Xline handle
    handle: Arc<RwLock<XlineHandle>>,
    /// Check interval
    reconcile_interval: Duration,
    /// Configuration Registry
    registry: Arc<dyn Registry + Sync + Send>,
}

impl Debug for Controller {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Controller")
            .field("name", &self.name)
            .field("state", &self.state)
            .field("reconcile_interval", &self.reconcile_interval)
            .finish()
    }
}

impl Controller {
    /// Constructor
    pub(crate) fn new(
        name: String,
        state: Arc<Mutex<StatePayload>>,
        handle: Arc<RwLock<XlineHandle>>,
        reconcile_interval: Duration,
        registry: Arc<dyn Registry + Sync + Send>,
    ) -> Self {
        Self {
            name,
            state,
            handle,
            reconcile_interval,
            registry,
        }
    }

    /// Run reconcile loop with shutdown
    #[allow(clippy::integer_arithmetic)] // this error originates in the macro `tokio::select`
    pub(crate) async fn run_reconcile_with_shutdown(
        self,
        init_member_config: MemberConfig,
        graceful_shutdown: impl Future<Output = ()>,
    ) -> Result<()> {
        select! {
            _ = graceful_shutdown => {
                Ok(())
            }
            res = self.run_reconcile(init_member_config) => {
                res
            }
        }
    }

    /// Run reconcile loop
    pub(crate) async fn run_reconcile(self, init_member_config: MemberConfig) -> Result<()> {
        let mut tick = interval(self.reconcile_interval);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let self_host = init_member_config
            .get_host(&self.name)
            .ok_or_else(|| anyhow!("node name {} not found in initial members", &self.name))?
            .clone();
        let init_config = self
            .registry
            .wait_full_fetch(self.name.clone(), self_host.clone()) // wait for all nodes to register config
            .await?;
        let mut member_config = MemberConfig {
            members: init_config.members,
            ..init_member_config
        };

        self.handle
            .write()
            .await
            .start(&member_config.xline_members())
            .await?;

        loop {
            let instant = tick.tick().await;

            let config = match self
                .registry
                .wait_full_fetch(self.name.clone(), self_host.clone())
                .await
            {
                Ok(c) => c,
                Err(err) => {
                    error!("fetch config failed, error {err}");
                    continue;
                }
            };
            member_config.members = config.members;

            if let Err(err) = self.reconcile_once(&member_config).await {
                error!("reconcile failed, error: {err}");
                continue;
            }
            debug!(
                "successfully reconcile the cluster states within {:?}",
                instant.elapsed()
            );
        }
    }

    /// Reconcile inner
    async fn reconcile_once(&self, member_config: &MemberConfig) -> Result<()> {
        let mut handle = self.handle.write().await;

        let sidecar_members = member_config.sidecar_members();
        let xline_members = member_config.xline_members();
        let cluster_size = member_config.members.len();
        let majority = member_config.majority_cnt();

        handle.apply_members(&xline_members).await?;

        let cluster_health = handle.is_healthy().await;
        let xline_running = handle.is_running().await;
        let states = StateStatus::gather(&sidecar_members).await?;

        match (cluster_health, xline_running) {
            (true, true) => {
                self.set_state(State::OK).await;

                info!("status: cluster healthy + xline running");
            }
            (true, false) => {
                self.set_state(State::Pending).await;

                info!("status: cluster healthy + xline not running, joining the cluster");
                handle.start(&xline_members).await?;
            }
            (false, true) => {
                self.set_state(State::Pending).await;

                if states
                    .states
                    .get(&State::OK)
                    .is_some_and(|c| *c >= majority)
                {
                    info!("status: cluster unhealthy + xline running + quorum ok, waiting...");
                } else {
                    info!(
                        "status: cluster unhealthy + xline running + quorum loss, backup and start failure recovery"
                    );
                    handle.backup().await?;
                    handle.stop().await?;
                }
            }
            (false, false) => {
                let is_seeder = states.seeder == self.name;
                if !is_seeder {
                    info!("status: cluster unhealthy + xline not running + not seeder, try to install backup");
                    handle.install_backup().await?;
                }

                self.set_state(State::Start).await;

                if states
                    .states
                    .get(&State::Start)
                    .is_some_and(|c| *c != cluster_size)
                {
                    info!("status: cluster unhealthy + xline not running + wait all start");
                    return Ok(());
                }

                if is_seeder {
                    info!(
                        "status: cluster unhealthy + xline not running + all start + seeder, seed cluster"
                    );
                    handle.start(&xline_members).await?;
                }
            }
        }

        Ok(())
    }

    /// Set current state
    async fn set_state(&self, state: State) {
        self.state.lock().await.state = state;
    }
}
