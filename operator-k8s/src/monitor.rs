use std::collections::HashMap;
use std::time::SystemTime;

use anyhow::Result;
use clippy_utilities::OverflowArithmetic;
use flume::Receiver;
use futures::Future;
use k8s_openapi::api::core::v1::Pod;
use kube::api::DeleteParams;
use kube::Api;
use operator_api::HeartbeatStatus;
use tracing::{debug, error};

use crate::crd::Cluster;

/// Sidecar monitor context
struct Context {
    /// Receiver for heartbeat status
    status_rx: Receiver<HeartbeatStatus>,
    /// Maximum interval between accepted `HeartbeatStatus`
    heartbeat_period: u64,
    /// Unreachable counter threshold
    unreachable_thresh: usize,
    /// Api for Cluster
    cluster_api: Api<Cluster>,
    /// Api for Pods
    pod_api: Api<Pod>,
}

/// A sidecar cluster states map
type SidecarClusterOwned<T> = HashMap<String, T>;

/// Sidecar monitor.
/// It monitors the communication of all sidecars, finds and tries to recover the dropped sidecar.
pub(crate) struct SidecarMonitor {
    /// Map for each sidecar clusters and their status
    statuses: HashMap<String, SidecarClusterOwned<HeartbeatStatus>>,
    /// Unreachable cache
    unreachable: HashMap<String, SidecarClusterOwned<usize>>,
    /// Context for sidecar monitor
    ctx: Context,
}

impl SidecarMonitor {
    /// Creates a new `SidecarState`
    pub(crate) fn new(
        status_rx: Receiver<HeartbeatStatus>,
        heartbeat_period: u64,
        unreachable_thresh: usize,
        cluster_api: Api<Cluster>,
        pod_api: Api<Pod>,
    ) -> Self {
        Self {
            statuses: HashMap::new(),
            unreachable: HashMap::new(),
            ctx: Context {
                status_rx,
                heartbeat_period,
                unreachable_thresh,
                cluster_api,
                pod_api,
            },
        }
    }

    /// Run the state update task with graceful shutdown.
    /// Return fatal error if run failed.
    /// The task that update the state received from sidecar operators
    #[allow(clippy::integer_arithmetic)] // required by tokio::select
    pub(crate) async fn run_with_graceful_shutdown(
        self,
        graceful_shutdown: impl Future<Output = ()>,
    ) -> Result<()> {
        tokio::select! {
            _ = graceful_shutdown => {
                Ok(())
            }
            res = self.state_update() => {
                res
            }
        }
    }

    /// Inner task for state update, return the unrecoverable error
    async fn state_update(mut self) -> Result<()> {
        loop {
            let status = self.ctx.status_rx.recv_async().await?;
            debug!("received status: {status:?}");
            self.state_update_inner(status).await;
        }
    }

    /// State update inner
    async fn state_update_inner(&mut self, status: HeartbeatStatus) {
        let spec_size = match self.get_spec_size(&status.cluster_name).await {
            Ok(spec_size) => spec_size,
            Err(err) => {
                error!("get cluster size failed, error: {err}");
                return;
            }
        };
        let majority = (spec_size / 2).overflow_add(1);

        debug!(
            "cluster: {}, spec.size: {spec_size}, majority: {majority}",
            status.cluster_name
        );

        let cluster_unreachable = self
            .unreachable
            .entry(status.cluster_name.clone())
            .or_default();
        let cluster_status = self
            .statuses
            .entry(status.cluster_name.clone())
            .or_default();
        let _prev = cluster_status.insert(status.name.clone(), status);

        let reachable: HashMap<_, _> =
            match get_reachable_counts(cluster_status, self.ctx.heartbeat_period, majority) {
                None => return,
                Some(counts) => cluster_status
                    .keys()
                    .map(|name| (name.clone(), counts.get(name).copied().unwrap_or(0)))
                    .collect(),
            };

        for (name, count) in reachable {
            // The sidecar operator is considered offline
            if count < majority {
                // If already in unreachable cache, increment the counter.
                // We would consider the recovery is failed if the counter reach
                // the threshold.
                if let Some(cnt) = cluster_unreachable.get_mut(&name) {
                    *cnt = cnt.overflow_add(1);
                    if *cnt == self.ctx.unreachable_thresh {
                        error!("failed to recover the operator: {name}");
                        let _ignore = cluster_unreachable.remove(&name);
                        let _ig = cluster_status.remove(&name);
                        // TODO: notify the administrator
                    }
                    continue;
                }
                // Otherwise delete the pod, which will trigger k8s to recreate it
                debug!("{name} is unreachable, count: {count}, deleting the pod");
                if let Err(e) = self
                    .ctx
                    .pod_api
                    .delete(&name, &DeleteParams::default())
                    .await
                {
                    error!("failed to delete pod {name}, {e}");
                }
                let _ignore = cluster_unreachable.insert(name, 0);
                continue;
            }
            // If recovered, remove it from the cache
            if cluster_unreachable.remove(&name).is_some() {
                debug!("sidecar {name} recovered");
            } else {
                debug!("sidecar {name} online");
            }
        }
    }

    /// Get the certain cluster size in the specification
    async fn get_spec_size(&self, cluster_name: &str) -> Result<usize> {
        let cluster = self.ctx.cluster_api.get(cluster_name).await?;
        Ok(cluster
            .spec
            .size
            .try_into()
            .unwrap_or_else(|_| unreachable!("the spec size should not be negative")))
    }
}

/// Get reachable counts for a sidecar cluster
fn get_reachable_counts(
    statuses: &SidecarClusterOwned<HeartbeatStatus>,
    heartbeat_period: u64,
    majority: usize,
) -> Option<HashMap<String, usize>> {
    let latest_ts = statuses.values().max()?.timestamp;

    let my_ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_else(|_| unreachable!("time turns back!"))
        .as_secs();

    if my_ts.abs_diff(latest_ts) > heartbeat_period {
        debug!("serious clock skew, sidecar latest timestamp: {latest_ts}, operator latest timestamp: {my_ts}");
        return None;
    }

    // Take timestamps that within the period from the latest
    let accepted = statuses
        .values()
        .filter(|s| s.timestamp.overflow_add(heartbeat_period) >= latest_ts);

    // The current accepted status is less than half
    if accepted.clone().count() < majority {
        return None;
    }

    let counts = accepted
        .flat_map(|s| s.reachable.iter())
        .fold(HashMap::new(), |mut map, name| {
            let v = map.entry(name.clone()).or_insert(0);
            *v = v.overflow_add(1);
            map
        });

    Some(counts)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn reachable_counts_should_be_correct() {
        let majority = 2;
        let heartbeat_period = 1;
        let id0 = "o0".to_owned();
        let id1 = "o1".to_owned();
        let id2 = "o2".to_owned();
        let cur_ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_else(|_| unreachable!("time turns back!"))
            .as_secs();

        let statuses1 = [
            (
                id0.clone(),
                HeartbeatStatus::new("c1".to_owned(), id0.clone(), cur_ts, vec![]),
            ),
            (
                id1.clone(),
                HeartbeatStatus::new("c1".to_owned(), id1.clone(), cur_ts + 1, vec![]),
            ),
            (
                id2.clone(),
                HeartbeatStatus::new("c1".to_owned(), id1.clone(), cur_ts + 10, vec![]),
            ),
        ];

        assert!(
            get_reachable_counts(&statuses1.into(), heartbeat_period, majority).is_none(),
            "the reachable status should not be accepted"
        );

        let statuses0 = [
            (
                id0.clone(),
                HeartbeatStatus::new(
                    "c1".to_owned(),
                    id0.clone(),
                    cur_ts - 10,
                    vec![id0.clone(), id1.clone(), id2.clone()],
                ),
            ),
            (
                id1.clone(),
                HeartbeatStatus::new(
                    "c1".to_owned(),
                    id1.clone(),
                    cur_ts,
                    vec![id1.clone(), id2.clone()],
                ),
            ),
            (
                id2.clone(),
                HeartbeatStatus::new(
                    "c1".to_owned(),
                    id1.clone(),
                    cur_ts + 1,
                    vec![id2.clone(), id0.clone(), id1.clone()],
                ),
            ),
        ];

        let counts = get_reachable_counts(&statuses0.into(), heartbeat_period, majority)
            .expect("the status not accepted");

        assert_eq!(counts[&id0], 1);
        assert_eq!(counts[&id1], 2);
        assert_eq!(counts[&id2], 2);
    }

    #[test]
    fn serious_clock_skew_should_be_detected() {
        let id0 = "o0".to_owned();
        let id1 = "o1".to_owned();
        let id2 = "o2".to_owned();

        let statuses = [
            (
                id0.clone(),
                HeartbeatStatus::new(
                    "c1".to_owned(),
                    id0.clone(),
                    10,
                    vec![id0.clone(), id1.clone()],
                ),
            ),
            (
                id1.clone(),
                HeartbeatStatus::new(
                    "c1".to_owned(),
                    id1.clone(),
                    10,
                    vec![id0.clone(), id1.clone()],
                ),
            ),
            (
                id2.clone(),
                HeartbeatStatus::new("c1".to_owned(), id1.clone(), 10, vec![id0, id2, id1]),
            ),
        ];

        assert!(
            get_reachable_counts(&statuses.into(), 20, 2).is_none(),
            "Did you test it on the original computer?"
        );
    }
}
