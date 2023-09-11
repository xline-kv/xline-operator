use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use k8s_openapi::NamespaceResourceScope;
use kube::api::{Patch, PatchParams};
use kube::{Api, Client, Resource};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tracing::{debug, error};

use crate::consts::FIELD_MANAGER;
use crate::controller::cluster::ClusterMetrics;
use crate::controller::{Controller, MetricsLabeled};
use crate::crd::v1alpha1::Cluster;
use crate::manager::cluster::Factory;

/// CRD `XlineCluster` controller
pub(crate) struct ClusterController {
    /// Kubernetes client
    pub(crate) kube_client: Client,
    /// The kubernetes cluster dns suffix
    pub(crate) cluster_suffix: String,
    /// Cluster metrics
    pub(crate) metrics: ClusterMetrics,
}

impl MetricsLabeled for kube::Error {
    fn labels(&self) -> Vec<&str> {
        #[allow(clippy::wildcard_enum_match_arm)] // the reason is enough
        match *self {
            Self::Api(_) => vec!["api error"],
            Self::Service(_) => vec!["service error"],
            Self::FromUtf8(_) | Self::SerdeError(_) => vec!["encode/decode error"],
            Self::Auth(_) => vec!["authorization error"],
            Self::OpensslTls(_) => vec!["tls error"],
            Self::HyperError(_) | Self::HttpError(_) => vec!["http error"],
            _ => vec!["unknown"],
        }
    }
}

/// Controller result
type Result<T> = std::result::Result<T, kube::Error>;

impl ClusterController {
    /// Apply resource
    #[allow(clippy::expect_used)] // use expect rather than unwrap_or_else(|| unreachable())
    async fn apply_resource<R: Resource<Scope = NamespaceResourceScope>>(
        &self,
        res: R,
    ) -> Result<()>
    where
        R: Clone + DeserializeOwned + Debug + Serialize,
        R::DynamicType: Default,
    {
        let namespace = res.meta().namespace.as_deref().expect("require namespace");
        let name = res.meta().name.clone().expect("require name");
        let api: Api<R> = Api::namespaced(self.kube_client.clone(), namespace);
        _ = api
            .patch(
                &name,
                &PatchParams::apply(FIELD_MANAGER),
                &Patch::Apply(res),
            )
            .await?;
        Ok(())
    }
}

#[async_trait]
impl Controller<Cluster> for ClusterController {
    type Error = kube::Error;
    type Metrics = ClusterMetrics;

    fn metrics(&self) -> &Self::Metrics {
        &self.metrics
    }

    async fn reconcile_once(&self, cluster: &Arc<Cluster>) -> Result<()> {
        debug!(
            "Reconciling cluster: \n{}",
            serde_json::to_string_pretty(cluster.as_ref()).unwrap_or_default()
        );
        let factory = Factory::new(Arc::clone(cluster), &self.cluster_suffix);

        self.apply_resource(factory.node_service()).await?;
        // TODO wait service ready
        self.apply_resource(factory.sts()).await?;

        Ok(())
    }

    fn handle_error(&self, resource: &Arc<Cluster>, err: &Self::Error) {
        error!("{:?} reconciliation error: {}", resource.metadata.name, err);
    }
}
