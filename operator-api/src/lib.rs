/// constants shared by the operator and the sidecar
pub mod consts;

/// Config registry
pub mod registry;

/// Xline handle
mod xline;

use futures::stream::FuturesUnordered;
use futures::StreamExt;
use reqwest::StatusCode;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub use xline::*;

use serde::{Deserialize, Serialize};

/// Heartbeat http client
static HEARTBEAT_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

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
    const DEFAULT_HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(10);

    /// Create a new `HeartbeatStatus`
    pub fn new(cluster_name: String, name: String, timestamp: u64, reachable: Vec<String>) -> Self {
        Self {
            cluster_name,
            name,
            timestamp,
            reachable,
        }
    }

    /// Create a new `HeartbeatStatus` from gathered information
    pub async fn gather(
        cluster_name: String,
        name: String,
        sidecars: &HashMap<String, String>,
    ) -> Self {
        use consts::SIDECAR_HEALTH_ROUTE;

        let client = HEARTBEAT_CLIENT.get_or_init(|| {
            reqwest::Client::builder()
                .timeout(Self::DEFAULT_HEALTH_CHECK_TIMEOUT)
                .build()
                .unwrap_or_else(|err| unreachable!("http client build error {err}"))
        });

        let mut reachable: Vec<_> = sidecars
            .iter()
            .map(|(name, addr)| async move {
                (
                    name,
                    client
                        .get(format!("http://{addr}{SIDECAR_HEALTH_ROUTE}"))
                        .send()
                        .await,
                )
            })
            .collect::<FuturesUnordered<_>>()
            .filter_map(|(name, resp)| async {
                resp.is_ok_and(|r| r.status() == StatusCode::OK)
                    .then(|| name.clone())
            })
            .collect()
            .await;

        // make sure self name should be inside
        if !reachable.contains(&name) {
            reachable.push(name.clone());
        }

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| unreachable!("time turns back!"));
        Self {
            cluster_name,
            name,
            timestamp: ts.as_secs(),
            reachable,
        }
    }

    /// Report status to monitor
    pub async fn report(&self, monitor_addr: &str) -> anyhow::Result<()> {
        use consts::OPERATOR_MONITOR_ROUTE;

        let url = format!("http://{monitor_addr}{OPERATOR_MONITOR_ROUTE}");

        let client = HEARTBEAT_CLIENT.get_or_init(|| {
            reqwest::Client::builder()
                .timeout(Self::DEFAULT_HEALTH_CHECK_TIMEOUT)
                .build()
                .unwrap_or_else(|err| unreachable!("http client build error {err}"))
        });

        let _ig = client.post(url).json(self).send().await?;

        Ok(())
    }
}
