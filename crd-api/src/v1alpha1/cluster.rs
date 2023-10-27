// The `JsonSchema` and `CustomResource` macro generates codes that does not pass the clippy lint.
#![allow(clippy::str_to_string)]
#![allow(clippy::missing_docs_in_private_items)]

use garde::Validate;
use k8s_openapi::api::core::v1::{Affinity, Container, PersistentVolumeClaim};
use k8s_openapi::serde::{Deserialize, Serialize};
use kube::CustomResource;
use schemars::JsonSchema;
use std::collections::HashMap;
use std::net::IpAddr;

/// Xline cluster specification
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema, Validate)]
#[kube(
    group = "xlineoperator.xline.cloud",
    version = "v1alpha1",
    kind = "XlineCluster",
    singular = "xlinecluster",
    plural = "xlineclusters",
    struct = "Cluster",
    namespaced,
    status = "ClusterStatus",
    shortname = "xc",
    scale = r#"{"specReplicasPath":".spec.size", "statusReplicasPath":".status.available"}"#,
    printcolumn = r#"{"name":"Size", "type":"string", "description":"The cluster size", "jsonPath":".spec.size"}"#,
    printcolumn = r#"{"name":"Available", "type":"string", "description":"The available amount", "jsonPath":".status.available"}"#,
    printcolumn = r#"{"name":"Backup Cron", "type":"string", "description":"The cron spec defining the interval a backup CronJob is run", "jsonPath":".spec.backup.cron"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "description":"The cluster age", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[schemars(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct ClusterSpec {
    /// Size of the xline cluster, less than 3 is not allowed
    #[garde(range(min = 3))]
    #[schemars(range(min = 3))]
    pub size: usize,
    /// Xline container specification
    pub container: Container,
    /// The affinity of the xline node
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affinity: Option<Affinity>,
    /// Backup specification
    #[garde(dive)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup: Option<BackupSpec>,
    /// The data PVC, if it is not specified, then use emptyDir instead
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<PersistentVolumeClaim>,
    /// Some user defined persistent volume claim templates
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pvcs: Option<Vec<PersistentVolumeClaim>>,
}

/// Xline cluster backup specification
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, Validate)]
pub struct BackupSpec {
    /// Cron Spec
    #[garde(pattern(r"^(?:\*|[0-5]?\d)(?:[-/,]?(?:\*|[0-5]?\d))*(?: +(?:\*|1?[0-9]|2[0-3])(?:[-/,]?(?:\*|1?[0-9]|2[0-3]))*){4}$"))]
    #[schemars(regex(
        pattern = r"^(?:\*|[0-5]?\d)(?:[-/,]?(?:\*|[0-5]?\d))*(?: +(?:\*|1?[0-9]|2[0-3])(?:[-/,]?(?:\*|1?[0-9]|2[0-3]))*){4}$"
    ))]
    pub cron: String,
    /// Backup storage type
    #[garde(dive)]
    #[serde(flatten)]
    pub storage: StorageSpec,
}

/// Xline cluster backup storage specification
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, Validate)]
#[serde(untagged)]
pub enum StorageSpec {
    /// S3 backup type
    S3 {
        /// S3 backup specification
        #[garde(dive)]
        s3: S3Spec,
    },
    /// Persistent volume backup type
    Pvc {
        /// Persistent volume claim
        #[garde(skip)]
        pvc: PersistentVolumeClaim,
    },
}

impl StorageSpec {
    pub fn as_pvc(&self) -> Option<&PersistentVolumeClaim> {
        match *self {
            Self::Pvc { ref pvc } => Some(pvc),
            Self::S3 { .. } => None,
        }
    }
}

/// Xline cluster backup S3 specification
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, Validate)]
pub struct S3Spec {
    /// S3 bucket name to use for backup
    #[garde(pattern(r"^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$"))]
    #[schemars(regex(pattern = r"^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$"))]
    pub bucket: String,
}

/// Xline cluster status
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default, Validate)]
#[garde(context(ClusterSpec as ctx))]
pub struct ClusterStatus {
    /// The available nodes' number in the cluster
    #[garde(range(max = ctx.size))]
    pub available: usize,
    /// The members registry
    #[garde(skip)]
    pub members: HashMap<String, IpAddr>,
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
            affinity: None,
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
            affinity: None,
            pvcs: None,
            data: None,
        };
        assert!(Validate::validate(&bad_size, &())
            .unwrap_err()
            .to_string()
            .contains("size"));
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
            affinity: None,
            pvcs: None,
            data: None,
        };
        assert!(Validate::validate(&bad_cron, &())
            .unwrap_err()
            .to_string()
            .contains("backup.cron"));
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
            affinity: None,
            pvcs: None,
            data: None,
        };
        assert!(Validate::validate(&bad_bucket, &())
            .unwrap_err()
            .to_string()
            .contains("backup.storage.s3.bucket"))
    }
}
