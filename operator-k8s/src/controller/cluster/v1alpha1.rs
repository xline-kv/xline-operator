use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use k8s_openapi::api::apps::v1::{
    RollingUpdateStatefulSetStrategy, StatefulSet, StatefulSetSpec, StatefulSetUpdateStrategy,
};
use k8s_openapi::api::batch::v1::{CronJob, CronJobSpec, JobSpec, JobTemplateSpec};
use k8s_openapi::api::core::v1::{
    Container, ContainerPort, EmptyDirVolumeSource, EnvVar, EnvVarSource, ObjectFieldSelector,
    PersistentVolumeClaim, PodSpec, PodTemplateSpec, Service, ServicePort, ServiceSpec, Volume,
    VolumeMount,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta, OwnerReference};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::api::{Patch, PatchParams};
use kube::{Api, Client, Resource, ResourceExt};
use tracing::{debug, error};
use utils::consts::{DEFAULT_BACKUP_DIR, DEFAULT_DATA_DIR};

use crate::controller::consts::{
    CRONJOB_IMAGE, DATA_EMPTY_DIR_NAME, DEFAULT_SIDECAR_PORT, DEFAULT_XLINE_PORT, FIELD_MANAGER,
    SIDECAR_PORT_NAME, XLINE_POD_NAME_ENV, XLINE_PORT_NAME,
};
use crate::controller::Controller;
use crate::crd::v1alpha1::{Cluster, StorageSpec};

/// CRD `XlineCluster` controller
pub(crate) struct ClusterController {
    /// Kubernetes client
    pub(crate) kube_client: Client,
    /// The kubernetes cluster dns suffix
    pub(crate) cluster_suffix: String,
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
    /// Volume(PVC) name conflict with `DATA_EMPTY_DIR_NAME`
    #[error("The {0} is conflict with the name internally used in the xline operator")]
    InvalidVolumeName(&'static str),
}

/// Controller result
type Result<T> = std::result::Result<T, Error>;

impl ClusterController {
    /// Extract ports
    fn extract_ports(cluster: &Arc<Cluster>) -> (ContainerPort, ContainerPort, Vec<ServicePort>) {
        // expose all the container's ports
        let mut xline_port = None;
        let mut sidecar_port = None;
        let container_ports = cluster.spec.container.ports.clone().unwrap_or_default();
        let mut service_ports: Vec<_> = container_ports
            .into_iter()
            .map(|port| {
                // the port with name `xline` is considered to be the port of xline
                if matches!(port.name.as_deref(), Some(XLINE_PORT_NAME)) {
                    xline_port = Some(port.clone());
                }
                // the port with name `sidecar` is considered to be the port of xline
                if matches!(port.name.as_deref(), Some(SIDECAR_PORT_NAME)) {
                    sidecar_port = Some(port.clone());
                }
                ServicePort {
                    name: port.name.clone(),
                    port: port.container_port,
                    ..ServicePort::default()
                }
            })
            .collect();
        if xline_port.is_none() {
            // add default xline port 2379 to service port if xline port is not specified
            service_ports.push(ServicePort {
                name: Some(XLINE_PORT_NAME.to_owned()),
                port: DEFAULT_XLINE_PORT,
                ..ServicePort::default()
            });
        }
        if sidecar_port.is_none() {
            // add default sidecar port 2380 to service port if sidecar port is not specified
            service_ports.push(ServicePort {
                name: Some(SIDECAR_PORT_NAME.to_owned()),
                port: DEFAULT_SIDECAR_PORT,
                ..ServicePort::default()
            });
        }
        // if it is not specified, 2379 is used as xline port
        let xline_port = xline_port.unwrap_or(ContainerPort {
            name: Some(XLINE_PORT_NAME.to_owned()),
            container_port: DEFAULT_XLINE_PORT,
            ..ContainerPort::default()
        });
        // if it is not specified, 2380 is used as sidecar port
        let sidecar_port = sidecar_port.unwrap_or(ContainerPort {
            name: Some(SIDECAR_PORT_NAME.to_owned()),
            container_port: DEFAULT_SIDECAR_PORT,
            ..ContainerPort::default()
        });
        (xline_port, sidecar_port, service_ports)
    }

    /// Extract persistent volume claims
    fn extract_pvcs(cluster: &Arc<Cluster>) -> Result<Vec<PersistentVolumeClaim>> {
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
            if spec_pvcs
                .iter()
                .any(|pvc| pvc.name_any() == DATA_EMPTY_DIR_NAME)
            {
                return Err(Error::InvalidVolumeName(".spec.pvcs[].metadata.name"));
            }
            pvcs.extend(spec_pvcs);
        }
        Ok(pvcs)
    }

    /// Extract owner reference
    fn extract_owner_ref(cluster: &Arc<Cluster>) -> OwnerReference {
        // unwrap controller_owner_ref is always safe
        let Some(owner_ref) = cluster.controller_owner_ref(&()) else { unreachable!("kube-runtime has undergone some changes.") };
        owner_ref
    }

    /// Extract name, namespace
    fn extract_id(cluster: &Arc<Cluster>) -> Result<(&str, &str)> {
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
        Ok((namespace, name))
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

    /// Apply headless service
    async fn apply_headless_service(
        &self,
        namespace: &str,
        name: &str,
        metadata: &ObjectMeta,
        service_ports: Vec<ServicePort>,
    ) -> Result<()> {
        let api: Api<Service> = Api::namespaced(self.kube_client.clone(), namespace);
        let _: Service = api
            .patch(
                name,
                &PatchParams::apply(FIELD_MANAGER),
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

    /// Prepare container volume
    fn prepare_container_volume(
        cluster: &Arc<Cluster>,
        mut container: Container,
    ) -> Result<(Container, Option<Vec<Volume>>)> {
        let backup = cluster.spec.backup.clone();
        let data = cluster.spec.data.clone();
        let mut volumes = None;
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
            if let Some(pvc_name) = backup_pvc_name {
                if pvc_name == DATA_EMPTY_DIR_NAME {
                    return Err(Error::InvalidVolumeName(".spec.backup.metadata.name"));
                }
                Some(VolumeMount {
                    mount_path: DEFAULT_BACKUP_DIR.to_owned(),
                    name: pvc_name,
                    ..VolumeMount::default()
                })
            } else {
                None
            }
        } else {
            None
        };
        // mount data volume to `DEFAULT_DATA_DIR` in container
        let data_mount = if let Some(pvc) = data {
            let name = pvc
                .metadata
                .name
                .ok_or(Error::MissingObject(".spec.data.metadata.name"))?;
            if name == DATA_EMPTY_DIR_NAME {
                return Err(Error::InvalidVolumeName(".spec.data.metadata.name"));
            }
            Some(VolumeMount {
                mount_path: DEFAULT_DATA_DIR.to_owned(),
                name,
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
            if spec_mounts
                .iter()
                .any(|mount| mount.name == DATA_EMPTY_DIR_NAME)
            {
                return Err(Error::InvalidVolumeName(
                    ".spec.container.volume_mounts[].name",
                ));
            }
            // extend the mounts
            mounts.extend(spec_mounts);
        }
        if let Some(mount) = backup_mount {
            mounts.push(mount);
        }
        if let Some(mount) = data_mount {
            mounts.push(mount);
        } else {
            // if data pv is not provided, then use emptyDir as volume
            volumes = Some(vec![Volume {
                name: DATA_EMPTY_DIR_NAME.to_owned(),
                empty_dir: Some(EmptyDirVolumeSource::default()),
                ..Volume::default()
            }]);
            mounts.push(VolumeMount {
                mount_path: DEFAULT_DATA_DIR.to_owned(),
                name: DATA_EMPTY_DIR_NAME.to_owned(),
                ..VolumeMount::default()
            });
        }
        // override the container volume_mounts
        container.volume_mounts = Some(mounts);
        Ok((container, volumes))
    }

    /// Prepare container environment
    fn prepare_container_env(mut container: Container) -> Container {
        // to get pod unique name
        let mut env = container.env.unwrap_or_default();
        env.push(EnvVar {
            name: XLINE_POD_NAME_ENV.to_owned(),
            value_from: Some(EnvVarSource {
                field_ref: Some(ObjectFieldSelector {
                    field_path: "metadata.name".to_owned(),
                    ..ObjectFieldSelector::default()
                }),
                ..EnvVarSource::default()
            }),
            ..EnvVar::default()
        });
        // override the pod environments
        container.env = Some(env);
        container
    }

    /// Prepare container command
    fn prepare_container_command(mut container: Container) -> Container {
        // the main command should wait forever so that the sidecar could always contact the xline container
        // so we use `tail -F /dev/null` here
        container.command = Some(
            "tail -F /dev/null"
                .split_whitespace()
                .map(ToOwned::to_owned)
                .collect(),
        );
        container
    }

    /// Prepare the xline container provided by user
    fn prepare_container(cluster: &Arc<Cluster>) -> Result<(Container, Option<Vec<Volume>>)> {
        let container = cluster.spec.container.clone();
        let (container, volumes) = Self::prepare_container_volume(cluster, container)?;
        let container = Self::prepare_container_env(container);
        let container = Self::prepare_container_command(container);
        Ok((container, volumes))
    }

    /// Apply the statefulset in k8s to reconcile cluster
    async fn apply_statefulset(
        &self,
        namespace: &str,
        name: &str,
        cluster: &Arc<Cluster>,
        pvcs: Vec<PersistentVolumeClaim>,
        metadata: &ObjectMeta,
    ) -> Result<()> {
        let api: Api<StatefulSet> = Api::namespaced(self.kube_client.clone(), namespace);
        let (container, volumes) = Self::prepare_container(cluster)?;
        let _: StatefulSet = api
            .patch(
                name,
                &PatchParams::apply(FIELD_MANAGER),
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
                                volumes,
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
        &self,
        namespace: &str,
        name: &str,
        size: i32,
        cron: &str,
        sidecar_port: &ContainerPort,
        metadata: &ObjectMeta,
    ) -> Result<()> {
        let api: Api<CronJob> = Api::namespaced(self.kube_client.clone(), namespace);
        let trigger_cmd = vec![
            "/bin/sh".to_owned(),
            "-ecx".to_owned(),
            format!(
                "curl {name}-$((RANDOM % {size})).{name}.{namespace}.svc.{}:{}/backup",
                self.cluster_suffix, sidecar_port.container_port
            ), // choose a node randomly
        ];
        let _: CronJob = api
            .patch(
                name,
                &PatchParams::apply(FIELD_MANAGER),
                &Patch::Apply(CronJob {
                    metadata: metadata.clone(),
                    spec: Some(CronJobSpec {
                        concurrency_policy: Some("Forbid".to_owned()), // A backup cron job cannot run concurrently
                        schedule: cron.to_owned(),
                        job_template: JobTemplateSpec {
                            spec: Some(JobSpec {
                                template: PodTemplateSpec {
                                    spec: Some(PodSpec {
                                        containers: vec![Container {
                                            name: format!("{name}-backup-cronjob"),
                                            image_pull_policy: Some("IfNotPresent".to_owned()),
                                            image: Some(CRONJOB_IMAGE.to_owned()),
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
}

#[async_trait]
impl Controller<Cluster> for ClusterController {
    type Error = Error;

    async fn reconcile_once(&self, cluster: &Arc<Cluster>) -> Result<()> {
        debug!(
            "Reconciling cluster: \n{}",
            serde_json::to_string_pretty(cluster.as_ref()).unwrap_or_default()
        );
        let (namespace, name) = Self::extract_id(cluster)?;
        let owner_ref = Self::extract_owner_ref(cluster);
        let pvcs = Self::extract_pvcs(cluster)?;
        let (_xline_port, sidecar_port, service_ports) = Self::extract_ports(cluster);
        let metadata = Self::build_metadata(namespace, name, owner_ref);

        self.apply_headless_service(namespace, name, &metadata, service_ports)
            .await?;
        self.apply_statefulset(namespace, name, cluster, pvcs, &metadata)
            .await?;

        if let Some(spec) = cluster.spec.backup.as_ref() {
            Box::pin(self.apply_backup_cron_job(
                namespace,
                name,
                cluster.spec.size,
                spec.cron.as_str(),
                &sidecar_port,
                &metadata,
            ))
            .await?;
        }
        Ok(())
    }

    fn handle_error(&self, resource: &Arc<Cluster>, err: &Self::Error) {
        error!("{:?} reconciliation error: {}", resource.metadata.name, err);
    }
}
