use std::collections::HashMap;

use anyhow::Result;
use clippy_utilities::OverflowArithmetic;
use flume::Receiver;
use futures::Future;
use k8s_openapi::api::core::v1::Pod;
use kube::api::DeleteParams;
use kube::{Api, CustomResourceExt};
use operator_api::HeartbeatStatus;
use tracing::{debug, error};

use crate::crd::Cluster;

/// State of sidecar operators
pub(crate) struct SidecarState {
    /// Map for each sidecar operator and its status
    statuses: HashMap<String, HeartbeatStatus>,
    /// Receiver for heartbeat status
    status_rx: Receiver<HeartbeatStatus>,
    /// Maximum interval between accepted `HeartbeatStatus`
    heartbeat_period: u64,
    /// Api for Cluster
    cluster_api: Api<Cluster>,
    /// Api for Pods
    pod_api: Api<Pod>,
    /// Unreachable cache
    unreachable: HashMap<String, usize>,
    /// Unreachable counter threshold
    unreachable_thresh: usize,
}

impl SidecarState {
    /// Creates a new `SidecarState`
    pub(crate) fn new(
        status_rx: Receiver<HeartbeatStatus>,
        heartbeat_period: u64,
        cluster_api: Api<Cluster>,
        pod_api: Api<Pod>,
        unreachable_thresh: usize,
    ) -> Self {
        Self {
            statuses: HashMap::new(),
            status_rx,
            heartbeat_period,
            cluster_api,
            pod_api,
            unreachable: HashMap::new(),
            unreachable_thresh,
        }
    }

    /// Run the state update task with graceful shutdown.
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
            res = self.state_update_inner() => {
                res
            }
        }
    }

    /// Inner task for state update
    async fn state_update_inner(mut self) -> Result<()> {
        loop {
            let status = self.status_rx.recv_async().await?;
            debug!("received status: {status:?}");
            let _prev = self.statuses.insert(status.name.clone(), status);

            let spec_size = self.get_spec_size().await?;
            let majority = (spec_size / 2).overflow_add(1);
            debug!("spec.size: {spec_size}, majority: {majority}");

            let Some(reachable_counts) =
                    Self::get_reachable_counts(&self.statuses, self.heartbeat_period, majority)
                else {
                    continue;
                };
            debug!("reachable_counts: {reachable_counts:?}");

            for name in self.statuses.keys() {
                let count = reachable_counts.get(name).copied().unwrap_or(0);
                // The sidecar operator is considered offline
                if count < majority {
                    // If already in unreachable cache, increment the counter.
                    // We would consider the recovery is failed if the counter reach
                    // the threshold.
                    if let Some(cnt) = self.unreachable.get_mut(name) {
                        *cnt = cnt.overflow_add(1);
                        if *cnt == self.unreachable_thresh {
                            error!("failed to recover the operator: {name}");
                            let _ignore = self.unreachable.remove(name);
                            // TODO: notify the administrator
                        }
                    }
                    // Otherwise delete the pod, which will trigger k8s to recreate it
                    else {
                        debug!("{name} is unreachable, count: {count}, deleting the pod");

                        if let Err(e) = self.pod_api.delete(name, &DeleteParams::default()).await {
                            error!("failed to delete pod {name}, {e}");
                        }
                        let _ignore = self.unreachable.insert(name.clone(), 0);
                    }
                // If recovered, remove it from the cache
                } else if self.unreachable.remove(name).is_some() {
                    debug!("operator {name} recovered");
                } else {
                    debug!("operator {name} online");
                }
            }
        }
    }

    /// Get the count for each reachable sidecar name
    fn get_reachable_counts(
        statuses: &HashMap<String, HeartbeatStatus>,
        heartbeat_period: u64,
        majority: usize,
    ) -> Option<HashMap<String, usize>> {
        let latest_ts = statuses
            .values()
            .max()
            .unwrap_or_else(|| unreachable!("there should be at least one status"))
            .timestamp;

        // Take timestamps that within the period from the latest
        let accepted = statuses
            .values()
            .filter(|s| s.timestamp.overflow_add(heartbeat_period) >= latest_ts);

        // The current accepted status is less than half
        if accepted.clone().count() < majority {
            return None;
        }

        Some(
            accepted
                .flat_map(|s| s.reachable.iter())
                .fold(HashMap::new(), |mut map, name| {
                    let v = map.entry(name.clone()).or_insert(0);
                    *v = v.overflow_add(1);
                    map
                }),
        )
    }

    /// Get the cluster size in the specification
    async fn get_spec_size(&self) -> Result<usize> {
        let cluster = self.cluster_api.get(Cluster::crd_name()).await?;
        Ok(cluster
            .spec
            .size
            .try_into()
            .unwrap_or_else(|_| unreachable!("the spec size should not be negative")))
    }
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

        let statuses1 = [
            (id0.clone(), HeartbeatStatus::new(id0.clone(), 0, vec![])),
            (id1.clone(), HeartbeatStatus::new(id1.clone(), 1, vec![])),
            (id2.clone(), HeartbeatStatus::new(id1.clone(), 10, vec![])),
        ];

        assert!(
            SidecarState::get_reachable_counts(&statuses1.into(), heartbeat_period, majority)
                .is_none(),
            "the reachable status should not be accepted"
        );

        let statuses0 = [
            (
                id0.clone(),
                HeartbeatStatus::new(id0.clone(), 0, vec![id0.clone(), id1.clone(), id2.clone()]),
            ),
            (
                id1.clone(),
                HeartbeatStatus::new(id1.clone(), 10, vec![id1.clone(), id2.clone()]),
            ),
            (
                id2.clone(),
                HeartbeatStatus::new(id1.clone(), 11, vec![id2.clone(), id0.clone(), id1.clone()]),
            ),
        ];

        let counts =
            SidecarState::get_reachable_counts(&statuses0.into(), heartbeat_period, majority)
                .expect("the status not accepted");

        assert_eq!(counts[&id0], 1);
        assert_eq!(counts[&id1], 2);
        assert_eq!(counts[&id2], 2);
    }
}
