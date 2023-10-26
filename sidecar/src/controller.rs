#![allow(dead_code)] // TODO remove when it is implemented

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::select;
use tokio::sync::Mutex;
use tokio::time::{interval, MissedTickBehavior};
use tracing::debug;

use crate::types::StatePayload;
use crate::xline::XlineHandle;

/// Sidecar operator controller
#[derive(Debug)]
pub(crate) struct Controller {
    /// The state of this operator
    state: Arc<Mutex<StatePayload>>,
    /// Xline handle
    handle: Arc<XlineHandle>,
    /// Check interval
    reconcile_interval: Duration,
}

impl Controller {
    /// Constructor
    pub(crate) fn new(
        state: Arc<Mutex<StatePayload>>,
        handle: Arc<XlineHandle>,
        reconcile_interval: Duration,
    ) -> Self {
        Self {
            state,
            handle,
            reconcile_interval,
        }
    }

    /// Run reconcile loop with shutdown
    #[allow(clippy::integer_arithmetic)] // this error originates in the macro `tokio::select`
    pub(crate) async fn run_reconcile_with_shutdown(
        self,
        graceful_shutdown: impl Future<Output = ()>,
    ) -> Result<()> {
        select! {
            _ = graceful_shutdown => {
                Ok(())
            }
            res = self.run_reconcile() => {
                res
            }
        }
    }

    /// Run reconcile loop
    pub(crate) async fn run_reconcile(self) -> Result<()> {
        let mut tick = interval(self.reconcile_interval);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            let instant = tick.tick().await;
            let _result = self.evaluate().await;
            let _result1 = self.execute().await;
            debug!(
                "successfully reconcile the cluster states within {:?}",
                instant.elapsed()
            );
        }
    }

    /// Evaluate cluster states
    #[allow(clippy::unused_async)] // TODO remove when it is implemented
    async fn evaluate(&self) -> Result<()> {
        // TODO evaluate states
        Ok(())
    }

    /// Execute reconciliation based on evaluation
    #[allow(clippy::unused_async)] // TODO remove when it is implemented
    async fn execute(&self) -> Result<()> {
        // TODO execute reconciliation
        Ok(())
    }
}
