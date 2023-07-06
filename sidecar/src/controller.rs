#![allow(dead_code)] // TODO remove when it is implemented

use thiserror::Error;
use tokio::select;
use tokio::sync::oneshot::Receiver;
use tokio::time::{Instant, Interval};

use crate::config::Config;

/// Sidecar operator controller
#[derive(Debug)]
pub(crate) struct Controller {
    /// Sidecar operator config
    config: Config,
    /// Check interval
    check_interval: Interval,
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
    pub(crate) fn new(config: Config) -> Self {
        let check_interval = tokio::time::interval(config.check_interval);
        Self {
            config,
            check_interval,
        }
    }

    /// Perform a reconciliation
    #[allow(clippy::integer_arithmetic)] // this error originates in the macro `tokio::select`
    pub(crate) async fn reconcile_once(
        &mut self,
        shutdown_rx: &mut Receiver<()>,
    ) -> Result<Instant> {
        select! {
            _ = shutdown_rx => {
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
