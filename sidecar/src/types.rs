#![allow(dead_code)]

use operator_api::XlineConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Sidecar operator config
#[derive(Debug, Clone)]
#[allow(clippy::exhaustive_structs)] // It is only constructed once
pub struct Config {
    /// Name of this node
    pub name: String,
    /// The cluster name
    pub cluster_name: String,
    /// Sidecar initial hosts, [pod_name]->[pod_host]
    pub init_members: HashMap<String, String>,
    /// The xline server port
    pub xline_port: u16,
    /// The sidecar web server port
    pub sidecar_port: u16,
    /// Reconcile cluster interval
    pub reconcile_interval: Duration,
    /// The xline config
    pub xline: XlineConfig,
    /// The backend to run xline
    pub backend: BackendConfig,
    /// The sidecar monitor (operator) config, set to enable
    /// heartbeat and configuration discovery
    pub monitor: Option<MonitorConfig>,
    /// Backup storage config
    pub backup: Option<BackupConfig>,
}

/// Monitor(Operator) config
#[derive(Debug, Clone)]
#[allow(clippy::exhaustive_structs)] // It is only constructed once
pub struct MonitorConfig {
    /// Monitor address
    pub monitor_addr: String,
    /// heartbeat interval
    pub heartbeat_interval: Duration,
}

/// Sidecar backend, it determinate how xline could be setup
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BackendConfig {
    /// K8s backend
    K8s {
        /// The pod name of this node
        pod_name: String,
        /// The xline container name, used to attach on it
        container_name: String,
        /// The namespace of this node
        namespace: String,
    },
    /// Local backend
    Local,
}

/// Backup storage config
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BackupConfig {
    /// S3 storage
    S3 {
        /// S3 bucket name
        bucket: String,
    },
    /// PV storage
    PV {
        /// Mounted path of pv
        path: PathBuf,
    },
}

impl Config {
    /// Get the initial sidecar members
    #[must_use]
    #[inline]
    pub fn init_sidecar_members(&self) -> HashMap<String, String> {
        self.init_members
            .clone()
            .into_iter()
            .map(|(name, host)| (name, format!("{host}:{}", self.sidecar_port)))
            .collect()
    }

    /// Get the initial xline members
    #[must_use]
    #[inline]
    pub fn init_xline_members(&self) -> HashMap<String, String> {
        self.init_members
            .clone()
            .into_iter()
            .map(|(name, host)| (name, format!("{host}:{}", self.xline_port)))
            .collect()
    }
}

/// Sidecar operator state
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub(crate) enum State {
    /// When this operator is trying to start it's kvserver
    Start,
    /// When this operator is pending on some confuse cluster status
    Pending,
    /// When this operator is working normally
    OK,
}

/// The state payload to expose states to other operators
#[derive(Debug, Deserialize, Serialize, Clone)]
pub(crate) struct StatePayload {
    /// Current state
    pub(crate) state: State,
    /// Current revision
    pub(crate) revision: i64,
}

/// The membership change request sent by other sidecar operators when they are shutting down
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct MembershipChange {
    /// The name of the sidecar operator
    pub(crate) name: String,
    /// The operation of this membership change request
    pub(crate) op: ChangeOP,
}

/// The change operation
#[derive(Debug, Deserialize, Serialize)]
pub(crate) enum ChangeOP {
    /// Remove this member
    Remove,
    /// Add this member with an address
    Add(String),
}
