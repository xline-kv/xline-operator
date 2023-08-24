#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Sidecar operator config
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Config {
    /// Name of this node
    pub name: String,
    /// Xline container name
    pub container_name: String,
    /// The xline server port
    pub xline_port: u16,
    /// The operator web server port
    pub operator_port: u16,
    /// Check cluster health interval
    pub check_interval: Duration,
    /// Backup storage config
    pub backup: Option<Backup>,
    /// Operators hosts, [pod_name]->[pod_host]
    members: HashMap<String, String>,
}

/// Backup storage config
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Backup {
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
    /// Constructor
    #[must_use]
    #[inline]
    pub fn new(
        name: String,
        container_name: String,
        members: HashMap<String, String>,
        xline_port: u16,
        operator_port: u16,
        check_interval: Duration,
        backup: Option<Backup>,
    ) -> Self {
        Self {
            name,
            container_name,
            xline_port,
            operator_port,
            check_interval,
            backup,
            members,
        }
    }

    /// Get the operator members
    pub(crate) fn operator_members(&self) -> HashMap<String, String> {
        self.members
            .clone()
            .into_iter()
            .map(|(name, host)| (name, format!("{host}:{}", self.operator_port)))
            .collect()
    }

    /// Get the xline members
    pub(crate) fn xline_members(&self) -> HashMap<String, String> {
        self.members
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
