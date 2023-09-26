use async_trait::async_trait;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{AttachParams, AttachedProcess};
use kube::Api;

/// xline handle abstraction
#[async_trait]
pub trait XlineHandle {
    /// the err during start and kill
    type Err;

    /// start a xline node
    async fn start(&mut self) -> Result<(), Self::Err>;

    /// kill a xline node
    async fn kill(&mut self) -> Result<(), Self::Err>;
}

/// K8s xline handle
pub struct K8sXlineHandle {
    /// the pod name
    pod_name: String,
    /// the container name of xline
    container_name: String,
    /// k8s pods api
    pods_api: Api<Pod>,
    /// the attached process of xline
    process: Option<AttachedProcess>,
}

#[async_trait]
impl XlineHandle for K8sXlineHandle {
    type Err = kube::Error;

    async fn start(&mut self) -> Result<(), Self::Err> {
        let process = self
            .pods_api
            .exec(
                &self.pod_name,
                vec!["sh"],
                &AttachParams::default().container(&self.container_name),
            )
            .await?;
        self.process = Some(process);
        todo!()
    }

    async fn kill(&mut self) -> Result<(), Self::Err> {
        todo!()
    }
}
