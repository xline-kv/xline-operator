#![allow(dead_code)] // remove when it is implemented

use crate::crd::Cluster;
use kube::runtime::controller::Action;
use kube::Client;
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

/// Context data
pub(crate) struct Context {
    /// Kubernetes client
    pub(crate) kube_client: Client,
}

/// All possible errors
#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {}

/// The reconciliation logic
#[allow(clippy::unused_async)] // remove when it is implemented
pub(crate) async fn reconcile(_crd: Arc<Cluster>, _cx: Arc<Context>) -> Result<Action, Error> {
    debug!("reconciling");
    Ok(Action::requeue(Duration::from_secs(10)))
}

/// The reconciliation error handle logic
#[allow(clippy::needless_pass_by_value)] // The function definition is required in Controller::run
pub(crate) fn on_error(_crd: Arc<Cluster>, _err: &Error, _cx: Arc<Context>) -> Action {
    Action::requeue(Duration::from_secs(10))
}
