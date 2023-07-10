#![allow(dead_code)] // remove when it is implemented

use k8s_openapi::api::apps::v1::{
    RollingUpdateStatefulSetStrategy, StatefulSet, StatefulSetSpec, StatefulSetUpdateStrategy,
};
use k8s_openapi::api::core::v1::{
    PersistentVolumeClaim, PodSpec, PodTemplateSpec, Service, ServicePort, ServiceSpec,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta, OwnerReference};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, Client, Resource};
use tracing::error;
use tracing::log::debug;

use crate::crd::{Cluster, StorageSpec};

/// Default recover requeue duration
const DEFAULT_RECOVER_REQUEUE_DURATION: Duration = Duration::from_secs(10);
/// The field manager identifier of deploy operator
const FIELD_MANAGER: &str = "xlineoperator.datenlord.io/deployoperator";

/// Context data
pub(crate) struct Context {
    /// Kubernetes client
    pub(crate) kube_client: Client,

    /// Reconcile interval
    pub(crate) reconcile_interval: Duration,
}

/// Action to be taken upon an `XlineCluster` resource during reconciliation
enum ClusterAction {
    /// Creation or Updating
    Mutation,
    /// Deletion
    Deletion,
    /// NoOp
    NoOp,
}

/// All possible errors
#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {
    /// Missing an object in cluster
    #[error("missing object {0} in cluster")]
    MissingObject(&'static str),
    /// Kube error
    #[error("api error")]
    Kube(#[from] kube::Error),
}

/// Controller result
type Result<T> = std::result::Result<T, Error>;

/// The reconciliation logic
#[allow(clippy::unused_async)] // remove when it is implemented
pub(crate) async fn reconcile(cluster: Arc<Cluster>, cx: Arc<Context>) -> Result<Action> {
    let (namespace, name, owner_ref, pvc) = extract_essential(&cluster)?;

    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let _: Option<_> = labels.insert("app".to_owned(), name.to_owned());
    let pp = &PatchParams::apply(FIELD_MANAGER);
    let metadata = ObjectMeta {
        labels: Some(labels.clone()),
        name: Some(name.to_owned()),
        namespace: Some(namespace.to_owned()),
        owner_references: Some(vec![owner_ref]),
        ..ObjectMeta::default()
    };

    let service_api: Api<Service> = Api::namespaced(cx.kube_client.clone(), namespace);
    let _: Service = service_api
        .patch(
            name,
            pp,
            &Patch::Apply(Service {
                metadata: metadata.clone(),
                spec: Some(ServiceSpec {
                    cluster_ip: None,
                    ports: cluster.spec.container.ports.as_ref().map(|v| {
                        v.iter()
                            .map(|port| ServicePort {
                                name: port.name.clone(),
                                port: port.container_port,
                                ..ServicePort::default()
                            })
                            .collect()
                    }),
                    selector: Some(labels.clone()),
                    ..ServiceSpec::default()
                }),
                ..Service::default()
            }),
        )
        .await?;

    let stateful_api: Api<StatefulSet> = Api::namespaced(cx.kube_client.clone(), namespace);
    let _: StatefulSet = stateful_api
        .patch(
            name,
            pp,
            &Patch::Apply(StatefulSet {
                metadata: metadata.clone(),
                spec: Some(StatefulSetSpec {
                    replicas: Some(cluster.spec.size),
                    selector: LabelSelector {
                        match_expressions: None,
                        match_labels: Some(labels.clone()),
                    },
                    service_name: name.to_owned(),
                    volume_claim_templates: pvc.map(|p| vec![p]),
                    update_strategy: Some(StatefulSetUpdateStrategy {
                        rolling_update: Some(RollingUpdateStatefulSetStrategy {
                            max_unavailable: Some(IntOrString::String("50%".to_owned())),
                            partition: None,
                        }),
                        type_: None,
                    }),
                    template: PodTemplateSpec {
                        metadata: Some(ObjectMeta {
                            labels: Some(labels),
                            ..ObjectMeta::default()
                        }),
                        spec: Some(PodSpec {
                            containers: vec![],
                            init_containers: None,
                            volumes: None,
                            ..PodSpec::default()
                        }),
                    },
                    ..StatefulSetSpec::default()
                }),
                ..StatefulSet::default()
            }),
        )
        .await?;

    Ok(Action::requeue(cx.reconcile_interval))
}

/// The reconciliation error handle logic
#[allow(clippy::needless_pass_by_value)] // The function definition is required in Controller::run
pub(crate) fn on_error(_cluster: Arc<Cluster>, err: &Error, _cx: Arc<Context>) -> Action {
    error!("reconciliation error: {}", err);
    Action::requeue(DEFAULT_RECOVER_REQUEUE_DURATION)
}

/// Extract essential values
fn extract_essential(
    cluster: &Arc<Cluster>,
) -> Result<(&str, &str, OwnerReference, Option<PersistentVolumeClaim>)> {
    debug!("status: {:?}", cluster.status);
    debug!("finalizers: {:?}", cluster.metadata.finalizers);
    let namespace = cluster
        .metadata
        .namespace
        .as_deref()
        .ok_or(Error::MissingObject(".metadata.namespace"))?;
    let name = cluster
        .metadata
        .name
        .as_deref()
        .ok_or(Error::MissingObject(".metadata.name"))?;

    let pvc = cluster
        .spec
        .backup
        .clone()
        .and_then(|spec| match spec.storage {
            StorageSpec::S3 { .. } => None,
            StorageSpec::Pvc { pvc } => Some(pvc),
        });

    #[allow(clippy::unwrap_used)] // controller_owner_ref can be safely unwrap
    Ok((
        namespace,
        name,
        cluster.controller_owner_ref(&()).unwrap(),
        pvc,
    ))
}
