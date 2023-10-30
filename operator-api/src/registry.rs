#![allow(unused)] // TODO remove

use anyhow::anyhow;
use async_trait::async_trait;
use crd_api::Cluster;
use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::serde_json::json;
use kube::api::{Patch, PatchParams};
use kube::core::object::HasStatus;
use kube::{Api, Client};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::ToSocketAddrs;
use std::sync::OnceLock;
use std::time::Duration;

use crate::consts::OPERATOR_REGISTRY_ROUTE;

pub use dummy::*;
pub use http::*;
pub use k8s::*;

#[derive(Deserialize, Debug, Default, Clone, Serialize)]
pub struct Config {
    /// Members [node_name] => [node_host]
    pub members: HashMap<String, String>,
    /// Cluster size
    pub cluster_size: usize,
}

const WAIT_DELAY: Duration = Duration::from_secs(5);
const WAIT_THRESHOLD: usize = 60;

#[async_trait]
pub trait Registry {
    async fn send_fetch(&self, self_name: String, self_host: String) -> anyhow::Result<Config>;

    async fn wait_full_fetch(
        &self,
        self_name: String,
        self_host: String,
    ) -> anyhow::Result<Config> {
        let mut retry = 0;
        loop {
            let config = self
                .send_fetch(self_name.clone(), self_host.clone())
                .await?;
            if config.members.len() == config.cluster_size {
                break Ok(config);
            }
            retry += 1;
            if retry > WAIT_THRESHOLD {
                break Err(anyhow!("wait for full config timeout"));
            }
            tokio::time::sleep(WAIT_DELAY).await;
        }
    }
}

mod k8s {
    use super::*;

    const FIELD_MANAGER: &str = "xlineoperator.datenlord.io/registry";

    /// K8s statefulset registry
    pub struct K8sStsRegistry {
        sts_name: String,
        sts_api: Api<StatefulSet>,
        namespace: String,
        dns_suffix: String,
    }

    impl K8sStsRegistry {
        /// New a k8s statefulset registry with default kube client
        pub async fn new_with_default(
            sts_name: String,
            namespace: String,
            dns_suffix: String,
        ) -> Self {
            let kube_client = Client::try_default()
                .await
                .unwrap_or_else(|_ig| unreachable!("it must be setup in k8s environment"));
            Self {
                sts_name,
                sts_api: Api::namespaced(kube_client, &namespace),
                namespace,
                dns_suffix,
            }
        }

        pub fn new(
            sts_name: String,
            namespace: String,
            kube_client: Client,
            dns_suffix: String,
        ) -> Self {
            Self {
                sts_name,
                sts_api: Api::namespaced(kube_client, &namespace),
                namespace,
                dns_suffix,
            }
        }
    }

    #[async_trait]
    impl Registry for K8sStsRegistry {
        async fn send_fetch(&self, _: String, _: String) -> anyhow::Result<Config> {
            let sts = self.sts_api.get(&self.sts_name).await?;
            let spec = sts
                .spec
                .unwrap_or_else(|| unreachable!(".spec should be set in statefulset"));
            let replicas = spec
                .replicas
                .unwrap_or_else(|| unreachable!(".spec.replicas should be set in statefulset"));
            let start_at = spec
                .ordinals
                .into_iter()
                .flat_map(|ordinal| ordinal.start)
                .next()
                .unwrap_or(0);
            let members = (start_at..)
                .take(replicas as usize)
                .map(|idx| {
                    let name = format!("{}-{idx}", self.sts_name);
                    let host = format!(
                        "{name}.{}.{}.svc.{}",
                        spec.service_name, self.namespace, self.dns_suffix
                    );
                    (name, host)
                })
                .collect();
            Ok(Config {
                members,
                cluster_size: replicas as usize,
            })
        }
    }
}

mod http {
    use super::*;

    static DEFAULT_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct RegisterQuery {
        pub cluster: String,
        pub name: String,
        pub host: String,
    }

    /// HTTP server registry
    pub struct HttpRegistry {
        server_addr: String,
        cluster_name: String,
    }

    impl HttpRegistry {
        pub fn new(server_addr: String, cluster_name: String) -> Self {
            Self {
                server_addr,
                cluster_name,
            }
        }
    }

    #[async_trait]
    impl Registry for HttpRegistry {
        async fn send_fetch(&self, self_name: String, self_host: String) -> anyhow::Result<Config> {
            let client = DEFAULT_HTTP_CLIENT.get_or_init(|| {
                reqwest::Client::builder().build().unwrap_or_else(|err| {
                    unreachable!("cannot build http client to register config, err: {err}")
                })
            });
            let config: Config = client
                .get(format!(
                    "http://{}{}",
                    self.server_addr, OPERATOR_REGISTRY_ROUTE
                ))
                .query(&RegisterQuery {
                    cluster: self.cluster_name.clone(),
                    name: self_name,
                    host: self_host,
                })
                .send()
                .await?
                .json()
                .await?;
            Ok(config)
        }
    }
}

mod dummy {
    use super::*;

    /// Dummy registry does not register anything, it keeps the original config
    pub struct DummyRegistry {
        init_members: HashMap<String, String>,
    }

    impl DummyRegistry {
        pub fn new(init_members: HashMap<String, String>) -> Self {
            Self { init_members }
        }
    }

    #[async_trait]
    impl Registry for DummyRegistry {
        async fn send_fetch(&self, _: String, _: String) -> anyhow::Result<Config> {
            Ok(Config {
                members: self.init_members.clone(),
                cluster_size: self.init_members.len(),
            })
        }
    }
}
