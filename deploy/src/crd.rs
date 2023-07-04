#![allow(dead_code)] // remove when it is implemented

#[cfg(test)]
use garde::Validate; // garde is for validation locally

use k8s_openapi::api::core::v1::{PodResourceClaim, PodSecurityContext, Toleration};
use k8s_openapi::serde::{Deserialize, Serialize};
use kube::CustomResource;
use schemars::JsonSchema;
use std::collections::BTreeMap;

/// The group name
pub(crate) const GROUP_NAME: &str = "xlineoperator.datenlord.io";
/// Current api group version
pub(crate) const CURRENT_VERSION: &str = "v1";

// The `CustomResource` macro generates a struct `Cluster` that does not pass the clippy lint.
#[allow(clippy::str_to_string)]
#[allow(clippy::missing_docs_in_private_items)]
pub(crate) mod v1 {
    #[cfg(test)]
    use super::Validate;

    use super::{
        BTreeMap, CustomResource, Deserialize, JsonSchema, PodResourceClaim, PodSecurityContext,
        Serialize, Toleration,
    };

    /// Xline cluster specification
    #[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
    #[cfg_attr(test, derive(Validate))]
    #[cfg_attr(test, garde(context(ClusterSpec)))]
    #[kube(
        group = "xlineoperator.datenlord.io",
        version = "v1",
        kind = "XlineCluster",
        singular = "xlinecluster",
        plural = "xlineclusters",
        struct = "Cluster",
        namespaced,
        status = "ClusterStatus",
        shortname = "xc",
        scale = r#"{"specReplicasPath":".spec.size", "statusReplicasPath":".status.size"}"#,
        printcolumn = r#"{"name":"CronSpec", "type":"string", "description":"The cron spec defining the interval a backup CronJob is run", "jsonPath":".spec.backup.cron"}"#,
        printcolumn = r#"{"name":"Size", "type":"string", "description":"The cluster size", "jsonPath":".spec.size"}"#,
        printcolumn = r#"{"name":"StorageType", "type":"string", "description":"The backup storage type", "jsonPath":".spec.backup.storage_type"}"#
    )]
    pub(crate) struct ClusterSpec {
        /// Size of the xline cluster, less than 3 is not allowed
        #[cfg_attr(test, garde(range(min = 3)))]
        #[schemars(range(min = 3))]
        pub(crate) size: usize,
        /// Pod specification
        #[cfg_attr(test, garde(dive))]
        pub(crate) pod: PodSpec,
        /// Backup specification
        #[cfg_attr(test, garde(custom(option_backup_dive)))]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub(crate) backup: Option<BackupSpec>,
    }

    /// Xline cluster backup specification
    #[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
    #[cfg_attr(test, derive(Validate))]
    pub(crate) struct BackupSpec {
        /// Cron Spec
        #[cfg_attr(test, garde(pattern(r"^(?:\*|[0-5]?\d)(?:[-/,]?(?:\*|[0-5]?\d))*(?: +(?:\*|1?[0-9]|2[0-3])(?:[-/,]?(?:\*|1?[0-9]|2[0-3]))*){4}$")))]
        #[schemars(regex(
            pattern = r"^(?:\*|[0-5]?\d)(?:[-/,]?(?:\*|[0-5]?\d))*(?: +(?:\*|1?[0-9]|2[0-3])(?:[-/,]?(?:\*|1?[0-9]|2[0-3]))*){4}$"
        ))]
        pub(crate) cron: String,
        /// Backup storage type, one of ["s3", "pv"]
        #[cfg_attr(test, garde(skip))]
        #[serde(flatten)]
        pub(crate) storage: StorageSpec,
    }

    /// Xline cluster backup storage specification
    #[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
    #[serde(untagged)]
    pub(crate) enum StorageSpec {
        #[serde(rename = "s3")]
        S3 { s3: S3Spec },
        #[serde(rename = "pv")]
        PV { pv: String },
    }

    /// Xline cluster backup S3 specification
    #[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
    pub(crate) struct S3Spec {
        /// S3 bucket path to use for backup
        /// TODO validate
        pub(crate) path: String,
        /// Name of k8s secret containing AWS creds
        pub(crate) secret: String,
    }

    /// Xline pod specification
    #[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
    pub(crate) struct PodSpec {
        /// Container image of xline to use
        pub(crate) container_image: String,
        /// NodeSelector is a selector which must be true for the pod to fit on a node.
        /// Selector which must match a node's labels for the pod to be scheduled on that node.
        /// More info: https://kubernetes.io/docs/concepts/configuration/assign-pod-node/
        /// The length must be `.spec.size`
        #[serde(skip_serializing_if = "Option::is_none")]
        pub(crate) node_selectors: Option<Vec<BTreeMap<String, String>>>,
        /// PodResourceClaim references exactly one ResourceClaim through a ClaimSource.
        /// It adds a name to it that uniquely identifies the ResourceClaim inside the Pod.
        /// Containers that need access to the ResourceClaim reference it with this name.
        /// The length must be `.spec.size`
        #[serde(skip_serializing_if = "Option::is_none")]
        pub(crate) resource_claims: Option<Vec<Vec<PodResourceClaim>>>,
        /// The pod this Toleration is attached to tolerates any taint that matches the triple
        /// <key,value,effect> using the matching operator <operator>.
        /// The length must be `.spec.size`
        #[serde(skip_serializing_if = "Option::is_none")]
        pub(crate) tolerations: Option<Vec<Vec<Toleration>>>,
        /// SecurityContext holds pod-level security attributes and common container settings.
        /// Optional: Defaults to empty. See type description for default values of each field.
        /// The length must be `.spec.size`
        #[serde(skip_serializing_if = "Option::is_none")]
        pub(crate) security_context: Option<Vec<PodSecurityContext>>,
    }

    /// Xline cluster status
    #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
    pub(crate) struct ClusterStatus {
        /// Whether the operator is working on creating or updating
        pub(crate) pending: bool,
    }

    #[cfg(test)]
    impl Validate for PodSpec {
        type Context = ClusterSpec;

        fn validate(&self, cx: &Self::Context) -> Result<(), garde::Errors> {
            garde::Errors::simple(|builder| {
                if let Some(node_selectors) = self.node_selectors.as_ref() {
                    if node_selectors.len() != cx.size {
                        builder.push(garde::Error::new(format!(
                            "The length of node_selectors must be {}",
                            cx.size
                        )));
                    }
                }
                if let Some(resource_claims) = self.resource_claims.as_ref() {
                    if resource_claims.len() != cx.size {
                        builder.push(garde::Error::new(format!(
                            "The length of resource_claims must be {}",
                            cx.size
                        )));
                    }
                }
                if let Some(tolerations) = self.tolerations.as_ref() {
                    if tolerations.len() != cx.size {
                        builder.push(garde::Error::new(format!(
                            "The length of tolerations must be {}",
                            cx.size
                        )));
                    }
                }
                if let Some(security_context) = self.security_context.as_ref() {
                    if security_context.len() != cx.size {
                        builder.push(garde::Error::new(format!(
                            "The length of security_context must be {}",
                            cx.size
                        )));
                    }
                }
            })
            .finish()
        }
    }

    #[cfg(test)]
    fn option_backup_dive(value: &Option<BackupSpec>, _cx: &ClusterSpec) -> garde::Result {
        if let Some(spec) = value.as_ref() {
            spec.validate(&())
                .map_err(|e| garde::Error::new(e.to_string()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::crd::v1::{BackupSpec, Cluster, ClusterSpec, PodSpec, S3Spec, StorageSpec};
    use crate::crd::{CURRENT_VERSION, GROUP_NAME};
    use garde::Validate;
    use kube::Resource;

    #[test]
    fn test_api_info() {
        assert_eq!(
            format!("{GROUP_NAME}/{CURRENT_VERSION}"),
            Cluster::api_version(&())
        );
    }

    #[test]
    fn validation_ok() {
        let ok = ClusterSpec {
            size: 3,
            backup: Some(BackupSpec {
                cron: "*/15 * * * *".to_owned(),
                storage: StorageSpec::PV {
                    pv: "xline_data".to_owned(),
                },
            }),
            pod: PodSpec {
                container_image: "datenlord/xline:latest".to_owned(),
                node_selectors: None,
                resource_claims: None,
                tolerations: None,
                security_context: None,
            },
        };
        assert!(Validate::validate(&ok, &ok).is_ok());
    }

    #[test]
    fn validation_bad_size() {
        let bad_size = ClusterSpec {
            size: 1,
            backup: Some(BackupSpec {
                cron: "*/15 * * * *".to_owned(),
                storage: StorageSpec::PV {
                    pv: "xline_data".to_owned(),
                },
            }),
            pod: PodSpec {
                container_image: "datenlord/xline:latest".to_owned(),
                node_selectors: None,
                resource_claims: None,
                tolerations: None,
                security_context: None,
            },
        };
        assert_eq!(
            Validate::validate(&bad_size, &bad_size)
                .unwrap_err()
                .flatten()[0]
                .0,
            "value.size"
        );
    }

    #[test]
    fn validation_bad_cron() {
        let bad_cron = ClusterSpec {
            size: 5,
            backup: Some(BackupSpec {
                cron: "1 day".to_owned(),
                storage: StorageSpec::PV {
                    pv: "xline_data".to_owned(),
                },
            }),
            pod: PodSpec {
                container_image: "datenlord/xline:latest".to_owned(),
                node_selectors: None,
                resource_claims: None,
                tolerations: None,
                security_context: None,
            },
        };
        assert_eq!(
            Validate::validate(&bad_cron, &bad_cron)
                .unwrap_err()
                .flatten()[0]
                .0,
            "value.backup"
        );
    }

    #[test]
    fn validation_bad_pod_spec_size() {
        let bad_spec_size = ClusterSpec {
            size: 3,
            backup: Some(BackupSpec {
                cron: "*/15 * * * *".to_owned(),
                storage: StorageSpec::S3 {
                    s3: S3Spec {
                        path: "/path/to/bucket".to_owned(),
                        secret: "secret".to_owned(),
                    },
                },
            }),
            pod: PodSpec {
                container_image: "datenlord/xline:latest".to_owned(),
                node_selectors: Some(vec![]),
                resource_claims: Some(vec![]),
                tolerations: Some(vec![]),
                security_context: Some(vec![]),
            },
        };
        assert!(Validate::validate(&bad_spec_size, &bad_spec_size)
            .unwrap_err()
            .to_string()
            .contains("must be 3"));
    }
}
