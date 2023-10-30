use anyhow::Result;
use crd_api::Cluster;
use kube::Api;
use operator_api::registry::Config;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::SidecarClusterOwned;

/// Member status
#[derive(Debug, Clone)]
struct MemberStatus {
    /// Host
    host: String,
    /// Last active
    last_active: Instant,
}

/// Http server registry
#[derive(Debug)]
pub(crate) struct Registry {
    /// Member status, [cluster_name] => [sidecar cluster members]
    members: HashMap<String, SidecarClusterOwned<MemberStatus>>,
    /// Time to live
    ttl: Duration,
    /// Api for Cluster
    cluster_api: Api<Cluster>,
}

impl Registry {
    /// Create a new registry
    pub(crate) fn new(ttl: Duration, cluster_api: Api<Cluster>) -> Self {
        Self {
            members: HashMap::new(),
            ttl,
            cluster_api,
        }
    }

    /// State update inner
    pub(crate) async fn receive(
        &mut self,
        cluster: String,
        name: String,
        host: String,
    ) -> Result<Config> {
        let cluster_size = self.get_spec_size(&cluster).await?;
        let now = Instant::now();

        let sidecars = self.members.entry(cluster).or_default();

        let sidecar = sidecars.entry(name).or_insert_with(|| MemberStatus {
            host: host.clone(),
            last_active: now,
        });
        sidecar.last_active = now;
        sidecar.host = host;

        sidecars.retain(|_, status| now.duration_since(status.last_active) < self.ttl);

        Ok(Config {
            members: sidecars
                .iter()
                .map(|(k, v)| (k.clone(), v.host.clone()))
                .collect(),
            cluster_size,
        })
    }

    /// Get the certain cluster size in the specification
    async fn get_spec_size(&self, cluster: &str) -> Result<usize> {
        let cluster = self.cluster_api.get(cluster).await?;
        Ok(cluster.spec.size)
    }
}
