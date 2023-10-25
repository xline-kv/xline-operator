/// v1alpha1
/// Features:
///     1. Xline sidecar
///     2. PV backup
pub(crate) mod v1alpha1;

/// CRD version
pub(crate) mod version;

/// Current CRD `XineCluster`
pub(crate) use v1alpha1::Cluster;

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::runtime::conditions;
use kube::runtime::wait::await_condition;
use kube::{Api, Client};
use tracing::debug;

use std::time::Duration;

/// wait crd to establish timeout
const CRD_ESTABLISH_TIMEOUT: Duration = Duration::from_secs(20);

/// Setup CRD
pub(crate) async fn setup(
    kube_client: &Client,
    manage_crd: bool,
    auto_migration: bool,
) -> anyhow::Result<()> {
    v1alpha1::set_up(kube_client, manage_crd, auto_migration).await
}

/// Wait for CRD to be established
async fn wait_crd_established(
    crd_api: Api<CustomResourceDefinition>,
    crd_name: &str,
) -> anyhow::Result<()> {
    let establish = await_condition(crd_api, crd_name, conditions::is_crd_established());
    debug!("wait for crd established");
    _ = tokio::time::timeout(CRD_ESTABLISH_TIMEOUT, establish).await??;
    Ok(())
}
