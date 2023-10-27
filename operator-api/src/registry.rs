#![allow(unused)] // TODO remove

use anyhow::anyhow;
use async_trait::async_trait;
use crd_api::Cluster;
use k8s_openapi::serde_json::json;
use kube::api::{Patch, PatchParams};
use kube::core::object::HasStatus;
use kube::{Api, Client};
use std::collections::HashMap;

pub struct Config {
    /// Members [node_name] => [node_ip]
    pub members: HashMap<String, String>,
    /// Cluster size
    pub cluster_size: usize,
}

#[async_trait]
pub trait Registry {
    const FIELD_MANAGER: &'static str = "xlineoperator.datenlord.io/registry";

    async fn send_fetch(&self, self_name: String, self_ip: String) -> anyhow::Result<Config>;
}

pub struct CustomResourceRegistry {
    cluster_name: String,
    cluster_api: Api<Cluster>,
}

impl CustomResourceRegistry {
    /// New a registry with default kube client
    pub async fn new_with_default(cluster_name: String, namespace: &str) -> Self {
        let kube_client = Client::try_default()
            .await
            .unwrap_or_else(|_ig| unreachable!("it must be setup in k8s environment"));
        Self {
            cluster_name,
            cluster_api: Api::namespaced(kube_client, namespace),
        }
    }

    pub async fn new(cluster_name: String, namespace: &str, kube_client: Client) -> Self {
        Self {
            cluster_name,
            cluster_api: Api::namespaced(kube_client, namespace),
        }
    }
}

#[async_trait]
impl Registry for CustomResourceRegistry {
    async fn send_fetch(&self, self_name: String, self_ip: String) -> anyhow::Result<Config> {
        /// TODO: hold a distributed lock here
        let cluster = self.cluster_api.get_status(&self.cluster_name).await?;
        let mut status = cluster
            .status
            .ok_or_else(|| anyhow!("no status found in cluster {}", self.cluster_name))?;
        if status.members.get(&self_name) != Some(&self_ip) {
            status.members.insert(self_name, self_ip);
            let patch = json!({
                "status": status,
            });
            let _ig = self
                .cluster_api
                .patch_status(
                    &self.cluster_name,
                    &PatchParams::apply(Self::FIELD_MANAGER),
                    &Patch::Apply(patch),
                )
                .await?;
        }
        Ok(Config {
            members: status.members,
            cluster_size: cluster.spec.size as usize,
        })
    }
}
