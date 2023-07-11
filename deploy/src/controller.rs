#![allow(dead_code)] // remove when it is implemented

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use k8s_openapi::api::apps::v1::{
    RollingUpdateStatefulSetStrategy, StatefulSet, StatefulSetSpec, StatefulSetUpdateStrategy,
};
use k8s_openapi::api::batch::v1::{CronJob, CronJobSpec, JobSpec, JobTemplateSpec};
use k8s_openapi::api::core::v1::{
    Container, PersistentVolumeClaim, PodSpec, PodTemplateSpec, Service, ServicePort, ServiceSpec,
    VolumeMount,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta, OwnerReference};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, Client, Resource};
use tracing::error;
use tracing::log::debug;

use crate::crd::{Cluster, StorageSpec};

/// Default recover requeue duration
const DEFAULT_REQUEUE_DURATION: Duration = Duration::from_secs(600);
/// The field manager identifier of deploy operator
const FIELD_MANAGER: &str = "xlineoperator.datenlord.io/deployoperator";
/// Default backup PV mount path in container, this path cannot be mounted by user
const DEFAULT_BACKUP_DIR: &str = "/xline_backup";
/// Default xline data dir, this path cannot be mounted by user
const DEFAULT_DATA_DIR: &str = "/usr/local/xline/data-dir";

/// Context data
pub(crate) struct Context {
    /// Kubernetes client
    pub(crate) kube_client: Client,
}

/// All possible errors
#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {
    /// Missing an object in cluster
    #[error("Missing object key {0} in cluster")]
    MissingObject(&'static str),
    /// Kube error
    #[error("Kubernetes api error")]
    Kube(#[from] kube::Error),
    /// Backup PV mount path is already mounted
    #[error("The path {0} is internally used in the xline operator and cannot be mounted.")]
    CannotMount(&'static str),
}

/// Controller result
type Result<T> = std::result::Result<T, Error>;

/// The reconciliation logic
#[allow(clippy::unused_async)] // remove when it is implemented
pub(crate) async fn reconcile(cluster: Arc<Cluster>, cx: Arc<Context>) -> Result<Action> {
    let (namespace, name, owner_ref, pvcs) = extract_essential(&cluster)?;
    let metadata = build_metadata(namespace, name, owner_ref);
    let pp = &PatchParams::apply(FIELD_MANAGER);
    // expose all the container pods
    let service_ports = cluster
        .spec
        .container
        .ports
        .as_ref()
        .map(|v| {
            v.iter()
                .map(|port| ServicePort {
                    name: port.name.clone(),
                    port: port.container_port,
                    ..ServicePort::default()
                })
                .collect()
        })
        .ok_or(Error::MissingObject(".spec.container.ports"))?;

    apply_headless_service(
        Api::namespaced(cx.kube_client.clone(), namespace),
        name,
        pp,
        &metadata,
        service_ports,
    )
    .await?;
    apply_statefulset(
        Api::namespaced(cx.kube_client.clone(), namespace),
        name,
        &cluster,
        pvcs,
        pp,
        &metadata,
    )
    .await?;
    if let Some(spec) = cluster.spec.backup.as_ref() {
        Box::pin(apply_backup_cron_job(
            Api::namespaced(cx.kube_client.clone(), namespace),
            name,
            namespace,
            cluster.spec.size,
            spec.cron.as_str(),
            pp,
            &metadata,
        ))
        .await?;
    }

    Ok(Action::requeue(DEFAULT_REQUEUE_DURATION))
}

/// The reconciliation error handle logic
#[allow(clippy::needless_pass_by_value)] // The function definition is required in Controller::run
pub(crate) fn on_error(_cluster: Arc<Cluster>, err: &Error, _cx: Arc<Context>) -> Action {
    error!("reconciliation error: {}", err);
    Action::requeue(DEFAULT_REQUEUE_DURATION)
}

/// Extract essential values
fn extract_essential(
    cluster: &Arc<Cluster>,
) -> Result<(&str, &str, OwnerReference, Vec<PersistentVolumeClaim>)> {
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
    let mut pvcs = Vec::new();
    // check if the backup type is PV, add the pvc to pvcs
    if let Some(spec) = cluster.spec.backup.as_ref() {
        if let StorageSpec::Pvc { pvc } = spec.storage.clone() {
            pvcs.push(pvc);
        }
    }
    // check if the data pvc if specified, add the pvc to pvcs
    if let Some(pvc) = cluster.spec.data.as_ref() {
        pvcs.push(pvc.clone());
    }
    // extend the user defined pvcs
    if let Some(spec_pvcs) = cluster.spec.pvcs.clone() {
        pvcs.extend(spec_pvcs);
    }

    #[allow(clippy::unwrap_used)] // controller_owner_ref can be safely unwrap
    Ok((
        namespace,
        name,
        cluster.controller_owner_ref(&()).unwrap(),
        pvcs,
    ))
}

/// Build the metadata which shares between all subresources
fn build_metadata(namespace: &str, name: &str, owner_ref: OwnerReference) -> ObjectMeta {
    let mut labels: BTreeMap<String, String> = BTreeMap::new();
    let _: Option<_> = labels.insert("app".to_owned(), name.to_owned());
    ObjectMeta {
        labels: Some(labels.clone()),            // it is used in selector
        name: Some(name.to_owned()),             // all subresources share the same name
        namespace: Some(namespace.to_owned()),   // all subresources share the same namespace
        owner_references: Some(vec![owner_ref]), // allow k8s GC to automatically clean up itself
        ..ObjectMeta::default()
    }
}

/// Apply the headless service to provide a stable network id between kvservers
async fn apply_headless_service(
    api: Api<Service>,
    name: &str,
    pp: &PatchParams,
    metadata: &ObjectMeta,
    service_ports: Vec<ServicePort>,
) -> Result<()> {
    let _: Service = api
        .patch(
            name,
            pp,
            &Patch::Apply(Service {
                metadata: metadata.clone(),
                spec: Some(ServiceSpec {
                    cluster_ip: None,
                    ports: Some(service_ports),
                    selector: metadata.labels.clone(),
                    ..ServiceSpec::default()
                }),
                ..Service::default()
            }),
        )
        .await?;
    Ok(())
}

/// Apply the statefulset in k8s to reconcile cluster
async fn apply_statefulset(
    api: Api<StatefulSet>,
    name: &str,
    cluster: &Arc<Cluster>,
    pvcs: Vec<PersistentVolumeClaim>,
    pp: &PatchParams,
    metadata: &ObjectMeta,
) -> Result<()> {
    let mut container = cluster.spec.container.clone();
    let backup = cluster.spec.backup.clone();
    let data = cluster.spec.data.clone();

    // mount backup volume to `DEFAULT_BACKUP_PV_MOUNT_PATH` in container
    let backup_mount = if let Some(spec) = backup {
        let backup_pvc_name = match spec.storage {
            StorageSpec::S3 { .. } => None,
            StorageSpec::Pvc { pvc } => Some(
                pvc.metadata
                    .name
                    .ok_or(Error::MissingObject(".spec.backup.pvc.metadata.name"))?,
            ),
        };
        backup_pvc_name.map(|pvc_name| VolumeMount {
            mount_path: DEFAULT_BACKUP_DIR.to_owned(),
            name: pvc_name,
            ..VolumeMount::default()
        })
    } else {
        None
    };
    // mount data volume to `DEFAULT_DATA_DIR` in container
    let data_mount = if let Some(pvc) = data {
        Some(VolumeMount {
            mount_path: DEFAULT_DATA_DIR.to_owned(),
            name: pvc
                .metadata
                .name
                .ok_or(Error::MissingObject(".spec.data.metadata.name"))?,
            ..VolumeMount::default()
        })
    } else {
        None
    };

    let mut mounts = Vec::new();
    // check if the container has specified volume_mounts before
    if let Some(spec_mounts) = container.volume_mounts {
        // if the container mount the dir used in operator, return error
        if spec_mounts
            .iter()
            .any(|mount| mount.mount_path.starts_with(DEFAULT_BACKUP_DIR))
        {
            return Err(Error::CannotMount(DEFAULT_BACKUP_DIR));
        }
        if spec_mounts
            .iter()
            .any(|mount| mount.mount_path.starts_with(DEFAULT_DATA_DIR))
        {
            return Err(Error::CannotMount(DEFAULT_DATA_DIR));
        }
        // extend the mounts
        mounts.extend(spec_mounts);
    }
    if let Some(mount) = backup_mount {
        mounts.push(mount);
    }
    if let Some(mount) = data_mount {
        mounts.push(mount);
    }
    // override the container volume_mounts
    container.volume_mounts = Some(mounts);

    let _: StatefulSet = api
        .patch(
            name,
            pp,
            &Patch::Apply(StatefulSet {
                metadata: metadata.clone(),
                spec: Some(StatefulSetSpec {
                    replicas: Some(cluster.spec.size),
                    selector: LabelSelector {
                        match_expressions: None,
                        match_labels: metadata.labels.clone(),
                    },
                    service_name: name.to_owned(),
                    volume_claim_templates: Some(pvcs),
                    update_strategy: Some(StatefulSetUpdateStrategy {
                        rolling_update: Some(RollingUpdateStatefulSetStrategy {
                            max_unavailable: Some(IntOrString::String("50%".to_owned())), // allow a maximum of half the cluster quorum shutdown when performing a rolling update
                            partition: None,
                        }),
                        ..StatefulSetUpdateStrategy::default()
                    }),
                    template: PodTemplateSpec {
                        metadata: Some(ObjectMeta {
                            labels: metadata.labels.clone(),
                            ..ObjectMeta::default()
                        }),
                        spec: Some(PodSpec {
                            init_containers: Some(vec![]), // TODO publish sidecar operator to registry
                            containers: vec![container], // TODO inject the sidecar operator container here
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
    Ok(())
}

/// Apply the cron job to trigger backup
async fn apply_backup_cron_job(
    api: Api<CronJob>,
    name: &str,
    namespace: &str,
    size: i32,
    cron: &str,
    pp: &PatchParams,
    metadata: &ObjectMeta,
) -> Result<()> {
    let trigger_cmd = vec![
        "/bin/sh".to_owned(),
        "-ecx".to_owned(),
        format!("curl {name}-$((RANDOM % {size})).{name}.{namespace}.svc.cluster.local/backup"),
    ];
    let _: CronJob = api
        .patch(
            name,
            pp,
            &Patch::Apply(CronJob {
                metadata: metadata.clone(),
                spec: Some(CronJobSpec {
                    concurrency_policy: Some("Forbid".to_owned()), // A cron job should wait for the previous one to complete (or timeout)
                    schedule: cron.to_owned(),
                    job_template: JobTemplateSpec {
                        spec: Some(JobSpec {
                            template: PodTemplateSpec {
                                spec: Some(PodSpec {
                                    containers: vec![Container {
                                        name: format!("{name}-backup-cronjob"),
                                        image_pull_policy: Some("IfNotPresent".to_owned()),
                                        image: Some("curlimages/curl".to_owned()),
                                        command: Some(trigger_cmd),
                                        ..Container::default()
                                    }],
                                    restart_policy: Some("OnFailure".to_owned()),
                                    ..PodSpec::default()
                                }),
                                ..PodTemplateSpec::default()
                            },
                            ..JobSpec::default()
                        }),
                        ..JobTemplateSpec::default()
                    },
                    ..CronJobSpec::default()
                }),
                ..CronJob::default()
            }),
        )
        .await?;
    Ok(())
}
