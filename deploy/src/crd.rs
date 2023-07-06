#![allow(dead_code)] // remove when it is implemented

#[cfg(test)]
use garde::Validate;
use k8s_openapi::api::core::v1::{Container, PersistentVolumeClaim};
use k8s_openapi::serde::{Deserialize, Serialize};
use kube::CustomResource;
use schemars::JsonSchema;

// garde is for validation locally

/// Current CRD `XineCluster` version
pub(crate) type Cluster = v1::Cluster;

// The `CustomResource` macro generates a struct `Cluster` that does not pass the clippy lint.
#[allow(clippy::str_to_string)]
#[allow(clippy::missing_docs_in_private_items)]
mod v1 {
    #[cfg(test)]
    use super::Validate;
    use super::{
        Container, CustomResource, Deserialize, JsonSchema, PersistentVolumeClaim, Serialize,
    };

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
        printcolumn = r#"{"name":"Backup Cron", "type":"string", "description":"The cron spec defining the interval a backup CronJob is run", "jsonPath":".spec.backup.cron"}"#,
        printcolumn = r#"{"name":"Age", "type":"date", "description":"The cluster age", "jsonPath":".metadata.creationTimestamp"}"#
    )]
    pub(crate) struct ClusterSpec {
        /// Size of the xline cluster, less than 3 is not allowed
        #[cfg_attr(test, garde(range(min = 3)))]
        #[schemars(range(min = 3))]
        pub(crate) size: usize,
        /// Xline container specification
        #[cfg_attr(test, garde(skip))]
        pub(crate) container: Container,
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
        /// Backup storage type
        #[cfg_attr(test, garde(skip))]
        #[serde(flatten)]
        pub(crate) storage: StorageSpec,
    }

    /// Xline cluster backup storage specification
    #[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
    #[serde(untagged)]
    pub(crate) enum StorageSpec {
        S3 { s3: S3Spec },
        Pvc { pvc: PersistentVolumeClaim },
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

    /// Xline cluster status
    #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
    pub(crate) struct ClusterStatus {
        /// Whether the operator is working on creating or updating
        pub(crate) pending: bool,
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
}

#[cfg(test)]
mod test {
    use garde::Validate;
    use k8s_openapi::api::core::v1::{Container, PersistentVolumeClaim};

    use crate::crd::v1::{BackupSpec, ClusterSpec, StorageSpec};

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
        };
        assert_eq!(
            Validate::validate(&bad_cron, &()).unwrap_err().flatten()[0].0,
            "value.backup"
        );
    }
}
