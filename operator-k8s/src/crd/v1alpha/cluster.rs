// The `JsonSchema` and `CustomResource` macro generates codes that does not pass the clippy lint.
#![allow(clippy::str_to_string)]
#![allow(clippy::missing_docs_in_private_items)]

#[cfg(test)]
use garde::Validate;
use k8s_openapi::api::core::v1::{Container, PersistentVolumeClaim};
use k8s_openapi::serde::{Deserialize, Serialize};
use kube::CustomResource;
use schemars::JsonSchema;

/// Xline cluster specification
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[cfg_attr(test, derive(Validate))]
#[kube(
    group = "xlineoperator.xline.cloud",
    version = "v1alpha",
    kind = "XlineCluster",
    singular = "xlinecluster",
    plural = "xlineclusters",
    struct = "Cluster",
    namespaced,
    status = "ClusterStatus",
    shortname = "xc",
    scale = r#"{"specReplicasPath":".spec.size", "statusReplicasPath":".status.available"}"#,
    printcolumn = r#"{"name":"Size", "type":"string", "description":"The cluster size", "jsonPath":".spec.size"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "description":"The cluster age", "jsonPath":".metadata.creationTimestamp"}"#
)]
pub(crate) struct ClusterSpec {
    /// Size of the xline cluster, less than 3 is not allowed
    #[cfg_attr(test, garde(range(min = 3)))]
    #[schemars(range(min = 3))]
    pub(crate) size: i32,
    /// Xline container specification
    #[cfg_attr(test, garde(skip))]
    pub(crate) container: Container,
    /// The data PVC, if it is not specified, then use emptyDir instead
    #[cfg_attr(test, garde(skip))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) data: Option<PersistentVolumeClaim>,
    /// Some user defined persistent volume claim templates
    #[cfg_attr(test, garde(skip))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pvcs: Option<Vec<PersistentVolumeClaim>>,
}

/// Xline cluster status
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub(crate) struct ClusterStatus {
    /// The available nodes' number in the cluster
    pub(crate) available: i32,
}

#[cfg(test)]
mod test {
    use garde::Validate;
    use k8s_openapi::api::core::v1::Container;

    use super::ClusterSpec;

    #[test]
    fn validation_ok() {
        let ok = ClusterSpec {
            size: 3,
            container: Container::default(),
            pvcs: None,
            data: None,
        };
        assert!(Validate::validate(&ok, &()).is_ok());
    }

    #[test]
    fn validation_bad_size() {
        let bad_size = ClusterSpec {
            size: 1,
            container: Container::default(),
            pvcs: None,
            data: None,
        };
        assert_eq!(
            Validate::validate(&bad_size, &()).unwrap_err().flatten()[0].0,
            "value.size"
        );
    }
}
