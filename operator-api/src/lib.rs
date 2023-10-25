/// constants shared by the operator and the sidecar
pub mod consts;

/// Xline handle
mod xline;

use std::time::{SystemTime, UNIX_EPOCH};
pub use xline::{K8sXlineHandle, LocalXlineHandle, XlineHandle};

use serde::{Deserialize, Serialize};

/// Heartbeat status, sort by timestamp.
/// The clock of each machine may be different, which may cause heartbeat to be unable to assist
/// the operator in detecting the dropped sidecar.
///
/// FIXME: May cause misjudgment under extreme conditions:
/// Assume a 3-node cluster. One of the sidecars has a slow clock. When network fluctuations occur,
/// the two sidecars with faster clocks fail to send heartbeats. At this time, the sidecar with
/// slower clocks successfully sends heartbeats and communicates with some outdated data stored on
/// the operator. The heartbeats satisfaction interval is not greater than the heartbeat period, and
/// the results obtained by the operator at this time may be out of date.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeartbeatStatus {
    /// the cluster name of this sidecar
    pub cluster_name: String,
    /// the name of the sidecar
    pub name: String,
    /// the timestamp of this status in seconds
    pub timestamp: u64,
    /// reachable sidecar names
    pub reachable: Vec<String>,
}

impl PartialOrd for HeartbeatStatus {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.timestamp.cmp(&other.timestamp))
    }
}

impl Ord for HeartbeatStatus {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

impl HeartbeatStatus {
    /// Create a new `HeartbeatStatus` with current timestamp
    pub fn current(cluster_name: String, name: String, reachable: Vec<String>) -> Self {
        let dur = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| unreachable!("time turns back!"));
        Self {
            cluster_name,
            name,
            timestamp: dur.as_secs(),
            reachable,
        }
    }

    /// Create a new `HeartbeatStatus`
    pub fn new(cluster_name: String, name: String, timestamp: u64, reachable: Vec<String>) -> Self {
        Self {
            cluster_name,
            name,
            timestamp,
            reachable,
        }
    }
}
