#![allow(dead_code)] // TODO remove when it is implemented

use std::sync::Arc;
use thiserror::Error;
use tokio::select;
use tokio::sync::watch::Receiver;
use tokio::sync::Mutex;
use tokio::time::{Instant, Interval};

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
    check_interval: Interval,
    /// graceful shutdown signal
    graceful_shutdown: Receiver<()>,
}

/// All possible errors
#[derive(Error, Debug, PartialEq)]
pub(crate) enum Error {
    /// Graceful shutdown error
    #[error("operator has been shutdown")]
    Shutdown,
}

/// Controller result
type Result<T> = std::result::Result<T, Error>;

impl Controller {
    /// Constructor
    pub(crate) fn new(
        handle: Arc<XlineHandle>,
        check_interval: Interval,
        state: Arc<Mutex<StatePayload>>,
        graceful_shutdown: Receiver<()>,
    ) -> Self {
        Self {
            state,
            handle,
            check_interval,
            graceful_shutdown,
        }
    }

    /// Perform a reconciliation
    #[allow(clippy::integer_arithmetic)] // this error originates in the macro `tokio::select`
    pub(crate) async fn reconcile_once(&mut self) -> Result<Instant> {
        select! {
            _ = self.graceful_shutdown.changed() => {
                // TODO notify the cluster of this node's shutdown
                Err(Error::Shutdown)
            }
            instant = self.check_interval.tick() => {
                self.reconcile_inner().await.map(|_| instant)
            }
        }
    }

    /// Reconciliation inner
    async fn reconcile_inner(&mut self) -> Result<()> {
        self.evaluate().await?;
        self.execute().await
    }

    /// Evaluate cluster states
    #[allow(clippy::unused_async)] // TODO remove when it is implemented
    async fn evaluate(&mut self) -> Result<()> {
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
