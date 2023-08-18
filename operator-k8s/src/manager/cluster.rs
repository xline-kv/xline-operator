#![allow(unused)] // remove when implemented

use crate::consts::{
    ANNOTATION_INHERIT_LABELS_PREFIX, DEFAULT_SIDECAR_PORT, DEFAULT_XLINE_PORT,
    LABEL_CLUSTER_COMPONENT, LABEL_CLUSTER_NAME, SIDECAR_PORT_NAME, XLINE_POD_NAME_ENV,
    XLINE_PORT_NAME,
};
use crate::crd::v1alpha1::{Cluster, StorageSpec};

use std::collections::BTreeMap;
use std::sync::Arc;

use k8s_openapi::api::core::v1::{
    Container, ContainerPort, EnvVar, EnvVarSource, GRPCAction, ObjectFieldSelector,
    PersistentVolumeClaim, PersistentVolumeClaimVolumeSource, Pod, PodSpec, PodTemplateSpec, Probe,
    Service, ServicePort, ServiceSpec, Volume, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference};
use kube::{Resource, ResourceExt};
use utils::consts::{DEFAULT_BACKUP_DIR, DEFAULT_DATA_DIR};

/// Read objects from `XlineCluster`
pub(crate) struct Extractor<'a> {
    /// `XlineCluster`
    cluster: &'a Cluster,
}

/// The component of `XlineCluster`
#[derive(Copy, Clone)]
pub(crate) enum Component {
    /// A xline node
    Node,
    /// A service
    Service,
    /// A backup job
    BackupJob,
}

impl Component {
    /// Get the component name
    fn label(&self) -> &str {
        match *self {
            Component::Node => "node",
            Component::Service => "srv",
            Component::BackupJob => "job",
        }
    }
}

impl<'a> Extractor<'a> {
    /// Constructor
    pub(crate) fn new(cluster: &'a Cluster) -> Self {
        Self { cluster }
    }

    /// Extract the exposed ports in `XlineCluster`
    /// Return the xline port, sidecar port, and a list of service ports
    /// gathered from all exposed ports, which will be used to build a `Service`.
    /// If the `XlineCluster` does not specified the xline ports (a port with name 'xline') or
    /// the sidecar ports (a port with name 'sidecar'), the default port (xline: 2379, sidecar: 2380)
    /// will be used.
    fn extract_ports(&self) -> (ContainerPort, ContainerPort, Vec<ServicePort>) {
        // expose all the container's ports
        let mut xline_port = None;
        let mut sidecar_port = None;
        let container_ports = self
            .cluster
            .spec
            .container
            .ports
            .clone()
            .unwrap_or_default();
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

    /// Extract all PVC templates
    /// The PVC template is used to create PVC for every pod
    fn extract_pvc_template(&self) -> Vec<PersistentVolumeClaim> {
        self.cluster
            .spec
            .backup
            .iter()
            .filter_map(|spec| {
                if let StorageSpec::Pvc { pvc } = spec.storage.clone() {
                    Some(pvc)
                } else {
                    None
                }
            })
            .chain(self.cluster.spec.data.iter().cloned())
            .chain(self.cluster.spec.pvcs.iter().flatten().cloned())
            .collect()
    }

    /// Extract volume mount for backup and data pvc
    /// Other pvc should be mounted by user
    fn extract_additional_volume_mount(&self) -> Vec<VolumeMount> {
        self.cluster
            .spec
            .backup
            .iter()
            .filter_map(|spec| {
                if let StorageSpec::Pvc { pvc } = spec.storage.clone() {
                    Some(pvc)
                } else {
                    None
                }
            })
            .map(|pvc| VolumeMount {
                name: pvc.name_any(), // because the volume name is the same as pvc template name, we can use it in volume mount
                mount_path: DEFAULT_BACKUP_DIR.to_owned(),
                ..VolumeMount::default()
            })
            .chain(
                self.cluster
                    .spec
                    .data
                    .iter()
                    .cloned()
                    .map(|pvc| VolumeMount {
                        name: pvc.name_any(),
                        mount_path: DEFAULT_DATA_DIR.to_owned(),
                        ..VolumeMount::default()
                    }),
            )
            .collect()
    }

    /// Extract owner reference
    fn extract_owner_ref(&self) -> OwnerReference {
        // unwrap controller_owner_ref is always safe
        let Some(owner_ref) = self.cluster.controller_owner_ref(&()) else { unreachable!("kube-runtime has undergone some changes.") };
        owner_ref
    }

    /// Extract name, namespace
    #[allow(clippy::expect_used)] // it is ok because xlinecluster has field validation
    fn extract_id(&self) -> (&str, &str) {
        let namespace = self
            .cluster
            .metadata
            .namespace
            .as_deref()
            .expect("xlinecluster resource should have a namespace");
        let name = self
            .cluster
            .metadata
            .name
            .as_deref()
            .expect("xlinecluster resource should have a name");
        (namespace, name)
    }

    /// Extract inherit labels
    fn extract_inherit_labels(&self) -> BTreeMap<String, String> {
        let Some(prefix) = self
            .cluster
            .metadata
            .annotations
            .as_ref()
            .and_then(|annotations| annotations.get(ANNOTATION_INHERIT_LABELS_PREFIX)) else { return BTreeMap::new() };
        let prefix: Vec<_> = prefix
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect();
        let Some(labels) = self.cluster.metadata.labels.as_ref() else { return BTreeMap::new() };
        labels
            .iter()
            .filter_map(|(l, v)| {
                prefix
                    .iter()
                    .find(|p| l.starts_with(*p))
                    .and(Some((l.clone(), v.clone())))
            })
            .collect()
    }
}

/// Factory generate the objects in k8s
pub(crate) struct Factory {
    /// The kubernetes cluster dns suffix
    cluster_suffix: String,
    /// `XlineCluster`
    cluster: Arc<Cluster>,
}

impl Factory {
    /// Constructor
    pub(crate) fn new(cluster: Arc<Cluster>, cluster_suffix: &str) -> Self {
        Self {
            cluster_suffix: cluster_suffix.to_owned(),
            cluster,
        }
    }

    /// Get the full component name
    fn component_name(cluster_name: &str, component: Component) -> String {
        format!("{cluster_name}-{}", component.label())
    }

    /// Get the general metadata
    fn general_metadata(&self, component: Component) -> ObjectMeta {
        let extractor = Extractor::new(self.cluster.as_ref());
        let mut labels = extractor.extract_inherit_labels();
        let (name, namespace) = extractor.extract_id();
        let owner_ref = extractor.extract_owner_ref();
        let _ig = labels.insert(LABEL_CLUSTER_NAME.to_owned(), name.to_owned());
        let __ig = labels.insert(
            LABEL_CLUSTER_COMPONENT.to_owned(),
            component.label().to_owned(),
        );
        ObjectMeta {
            labels: Some(labels),                              // it is used in selector
            name: Some(Self::component_name(name, component)), // all subresources share the same name
            namespace: Some(namespace.to_owned()), // all subresources share the same namespace
            owner_references: Some(vec![owner_ref]), // allow k8s GC to automatically clean up itself
            ..ObjectMeta::default()
        }
    }

    /// Get the node headless service
    fn node_service(&self) -> Service {
        let extractor = Extractor::new(self.cluster.as_ref());
        let (_, _, service_ports) = extractor.extract_ports();
        let (name, _) = extractor.extract_id();
        Service {
            metadata: self.general_metadata(Component::Service),
            spec: Some(ServiceSpec {
                cluster_ip: None,
                ports: Some(service_ports),
                selector: Some(
                    [
                        (LABEL_CLUSTER_NAME.to_owned(), name.to_owned()),
                        (
                            LABEL_CLUSTER_COMPONENT.to_owned(),
                            Component::Node.label().to_owned(),
                        ),
                    ]
                    .into(),
                ),
                ..ServiceSpec::default()
            }),
            ..Service::default()
        }
    }

    /// Mount the additional volumes on the container
    #[allow(clippy::unused_self)]
    fn mount_volume_on_container(&self, container: &mut Container) {
        let extractor = Extractor::new(self.cluster.as_ref());
        let volume_mount = extractor.extract_additional_volume_mount();
        container
            .volume_mounts
            .get_or_insert(vec![])
            .extend(volume_mount);
    }

    /// Set the entrypoint of the container
    fn set_command(&self, container: &mut Container, index: usize) {
        let extractor = Extractor::new(self.cluster.as_ref());
        let (name, namespace) = extractor.extract_id();
        let (xline_port, _, _) = extractor.extract_ports();
        let srv_name = Self::component_name(name, Component::Service);
        let mut members = vec![];
        // the node before this index has already been added to the members
        // we use the members from 0 to index to build the initial cluster config for this node
        // and then do membership change to update the cluster config
        for i in 0..=index {
            let node_name = format!("{}-{i}", Self::component_name(name, Component::Node));
            members.push(format!(
                "{node_name}={node_name}.{srv_name}.{namespace}.svc.{}:{}",
                self.cluster_suffix, xline_port.container_port
            ));
        }
        // TODO add additional arguments config to CRD and append to the command
        let xline_cmd = format!("xline --name $({XLINE_POD_NAME_ENV}) --storage-engine rocksdb --data-dir {DEFAULT_DATA_DIR} --members {}", members.join(","))
            .split_whitespace()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        // TODO we need a sidecar systemd process to take care of xline
        container.command = Some(xline_cmd);
    }

    /// Get the xline container
    fn xline_container(&self, index: usize) -> Container {
        let mut container = self.cluster.spec.container.clone();
        self.mount_volume_on_container(&mut container);
        self.set_command(&mut container, index);
        // we need to set the env variable to get the pod name in the container
        container.env = Some(vec![EnvVar {
            name: XLINE_POD_NAME_ENV.to_owned(),
            value_from: Some(EnvVarSource {
                field_ref: Some(ObjectFieldSelector {
                    field_path: "metadata.name".to_owned(),
                    ..ObjectFieldSelector::default()
                }),
                ..EnvVarSource::default()
            }),
            ..EnvVar::default()
        }]);
        container
    }

    /// Get the node pod
    fn node_pod(&self, index: usize) -> PodTemplateSpec {
        let extractor = Extractor::new(self.cluster.as_ref());
        let (name, _) = extractor.extract_id();
        let node_name = format!("{}-{index}", Self::component_name(name, Component::Node));
        let xline = self.xline_container(index);
        let volumes = extractor
            .extract_pvc_template()
            .into_iter()
            .map(|pvc_template| Volume {
                name: pvc_template.name_any(), // the volume name is the same as pvc template name
                persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                    claim_name: format!("{}-{}", pvc_template.name_any(), node_name), // the pvc detail name is template name + node name
                    ..PersistentVolumeClaimVolumeSource::default()
                }),
                ..Volume::default()
            })
            .collect();
        let mut meta = self.general_metadata(Component::Node);
        meta.name = Some(node_name);
        PodTemplateSpec {
            metadata: Some(meta),
            spec: Some(PodSpec {
                init_containers: Some(vec![]),
                containers: vec![xline],
                affinity: self.cluster.spec.affinity.clone(),
                volumes: Some(volumes),
                ..PodSpec::default()
            }),
        }
    }

    /// Get the pvc for a node pod
    fn pvc(&self, index: usize) -> Vec<PersistentVolumeClaim> {
        let extractor = Extractor::new(self.cluster.as_ref());
        let mut pvcs = extractor.extract_pvc_template();
        let (name, _) = extractor.extract_id();
        let node_name = format!("{}-{index}", Self::component_name(name, Component::Node));
        for pvc in &mut pvcs {
            pvc.metadata.name = Some(format!("{}-{}", pvc.name_any(), node_name));
        }
        pvcs
    }
}
