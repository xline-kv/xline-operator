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
    printcolumn = r#"{"name":"Size", "type":"string", "description":"The cluster size", "jsonPath":".spec.size"}"#,
    printcolumn = r#"{"name":"Available", "type":"string", "description":"The available amount", "jsonPath":".status.available"}"#,
    printcolumn = r#"{"name":"Backup Cron", "type":"string", "description":"The cron spec defining the interval a backup CronJob is run", "jsonPath":".spec.backup.cron"}"#,
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
    /// Backup specification
    #[cfg_attr(test, garde(custom(option_backup_dive)))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) backup: Option<BackupSpec>,
    /// The data PVC, if it is not specified, then use emptyDir instead
    #[cfg_attr(test, garde(skip))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) data: Option<PersistentVolumeClaim>,
    /// Some user defined persistent volume claim templates
    #[cfg_attr(test, garde(skip))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pvcs: Option<Vec<PersistentVolumeClaim>>,
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
    /// Backup storage type
    #[cfg_attr(test, garde(dive))]
    #[serde(flatten)]
    pub(crate) storage: StorageSpec,
}

/// Xline cluster backup storage specification
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[cfg_attr(test, derive(Validate))]
#[serde(untagged)]
pub(crate) enum StorageSpec {
    /// S3 backup type
    S3 {
        /// S3 backup specification
        #[cfg_attr(test, garde(dive))]
        s3: S3Spec,
    },
    /// Persistent volume backup type
    Pvc {
        /// Persistent volume claim
        #[cfg_attr(test, garde(skip))]
        pvc: PersistentVolumeClaim,
    },
}

/// Xline cluster backup S3 specification
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[cfg_attr(test, derive(Validate))]
pub(crate) struct S3Spec {
    /// S3 bucket name to use for backup
    #[cfg_attr(test, garde(pattern(r"^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$")))]
    #[schemars(regex(pattern = r"^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$"))]
    pub(crate) bucket: String,
}

/// Xline cluster status
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub(crate) struct ClusterStatus {
    /// The available nodes' number in the cluster
    pub(crate) available: i32,
}

#[cfg(test)]
#[allow(clippy::trivially_copy_pass_by_ref)] // required bt garde
fn option_backup_dive(value: &Option<BackupSpec>, _cx: &()) -> garde::Result {
    if let Some(spec) = value.as_ref() {
        spec.validate(&())
            .map_err(|e| garde::Error::new(e.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use garde::Validate;
    use k8s_openapi::api::core::v1::{Container, PersistentVolumeClaim};

    use super::{BackupSpec, ClusterSpec, S3Spec, StorageSpec};

    #[test]
    fn validation_ok() {
        let ok = ClusterSpec {
            size: 3,
            backup: Some(BackupSpec {
                cron: "*/15 * * * *".to_owned(),
                storage: StorageSpec::Pvc {
                    pvc: PersistentVolumeClaim::default(),
                },
            }),
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
            backup: Some(BackupSpec {
                cron: "*/15 * * * *".to_owned(),
                storage: StorageSpec::Pvc {
                    pvc: PersistentVolumeClaim::default(),
                },
            }),
            container: Container::default(),
            pvcs: None,
            data: None,
        };
        assert_eq!(
            Validate::validate(&bad_size, &()).unwrap_err().flatten()[0].0,
            "value.size"
        );
    }

    #[test]
    fn validation_bad_cron() {
        let bad_cron = ClusterSpec {
            size: 5,
            backup: Some(BackupSpec {
                cron: "1 day".to_owned(),
                storage: StorageSpec::Pvc {
                    pvc: PersistentVolumeClaim::default(),
                },
            }),
            container: Container::default(),
            pvcs: None,
            data: None,
        };
        assert_eq!(
            Validate::validate(&bad_cron, &()).unwrap_err().flatten()[0].0,
            "value.backup"
        );
    }

    #[test]
    fn validation_bad_s3_bucket() {
        let bad_bucket = ClusterSpec {
            size: 5,
            backup: Some(BackupSpec {
                cron: "*/15 * * * *".to_owned(),
                storage: StorageSpec::S3 {
                    s3: S3Spec {
                        bucket: "&%$# /".to_owned(),
                    },
                },
            }),
            container: Container::default(),
            pvcs: None,
            data: None,
        };
        assert_eq!(
            Validate::validate(&bad_bucket, &()).unwrap_err().flatten()[0].0,
            "value.backup"
        );
    }
}
