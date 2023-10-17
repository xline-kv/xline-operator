use async_trait::async_trait;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{AttachParams, AttachedProcess};
use kube::Api;
use std::fmt::{Debug, Formatter};

use std::process::{Child, Command};

/// xline handle abstraction
#[async_trait]
pub trait XlineHandle: Debug + Send + Sync + 'static {
    /// start a xline node
    async fn start(&mut self) -> anyhow::Result<()>; // we dont care about what failure happened when start, it just failed

    /// kill a xline node
    async fn kill(&mut self) -> anyhow::Result<()>;
}

/// Local xline handle, it will execute the xline in the local
/// machine with the start_cmd
#[derive(Debug)]
pub struct LocalXlineHandle {
    start_cmd: String,
    child_proc: Option<Child>,
}

impl LocalXlineHandle {
    /// New a local xline handle
    pub fn new(start_cmd: String) -> Self {
        Self {
            start_cmd,
            child_proc: None,
        }
    }
}

#[async_trait]
impl XlineHandle for LocalXlineHandle {
    async fn start(&mut self) -> anyhow::Result<()> {
        self.kill().await?;
        let mut cmds = self.start_cmd.split_whitespace();
        let Some((exe, args)) = cmds
            .next()
            .map(|exe| (exe, cmds.collect::<Vec<_>>())) else {
            unreachable!("the start_cmd must be valid");
        };
        let proc = Command::new(exe).args(args).spawn()?;
        self.child_proc = Some(proc);
        Ok(())
    }

    async fn kill(&mut self) -> anyhow::Result<()> {
        if let Some(mut proc) = self.child_proc.take() {
            return Ok(proc.kill()?);
        }
        Ok(())
    }
}

/// K8s xline handle, it will execute the xline start_cmd
/// in pod
pub struct K8sXlineHandle {
    /// the pod name
    pod_name: String,
    /// the container name of xline
    container_name: String,
    /// k8s pods api
    pods_api: Api<Pod>,
    /// the attached process of xline
    process: Option<AttachedProcess>,
    /// the xline start cmd, parameters are split by ' '
    start_cmd: String,
}

impl Debug for K8sXlineHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("K8sXlineHandle")
            .field("pod_name", &self.pod_name)
            .field("container_name", &self.container_name)
            .field("pods_api", &self.pods_api)
            .field("start_cmd", &self.start_cmd)
            .finish()
    }
}

impl K8sXlineHandle {
    /// New with default k8s client
    pub async fn new_with_default(
        pod_name: String,
        container_name: String,
        namespace: &str,
        start_cmd: String,
    ) -> Self {
        let client = kube::Client::try_default()
            .await
            .unwrap_or_else(|_ig| unreachable!("it must be setup in k8s environment"));
        Self {
            pod_name,
            container_name,
            pods_api: Api::namespaced(client, namespace),
            process: None,
            start_cmd,
        }
    }

    /// New with the provided k8s client
    pub fn new_with_client(
        pod_name: String,
        container_name: String,
        client: kube::Client,
        namespace: &str,
        start_cmd: String,
    ) -> Self {
        Self {
            pod_name,
            container_name,
            pods_api: Api::namespaced(client, namespace),
            process: None,
            start_cmd,
        }
    }
}

#[async_trait]
impl XlineHandle for K8sXlineHandle {
    async fn start(&mut self) -> anyhow::Result<()> {
        self.kill().await?;
        let start_cmd: Vec<&str> = self.start_cmd.split_whitespace().collect();
        let process = self
            .pods_api
            .exec(
                &self.pod_name,
                start_cmd,
                &AttachParams::default().container(&self.container_name),
            )
            .await?;
        self.process = Some(process);
        Ok(())
    }

    async fn kill(&mut self) -> anyhow::Result<()> {
        if let Some(process) = self.process.take() {
            process.abort();
        }
        Ok(())
    }
}
