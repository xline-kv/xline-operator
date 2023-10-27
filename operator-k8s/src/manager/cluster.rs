#![allow(unused)] // TODO remove

use std::collections::BTreeMap;
use std::sync::Arc;

use k8s_openapi::api::apps::v1::{StatefulSet, StatefulSetSpec};
use k8s_openapi::api::core::v1::{
    Container, ContainerPort, EnvVar, EnvVarSource, ObjectFieldSelector, PersistentVolumeClaim,
    PodSpec, PodTemplateSpec, Service, ServicePort, ServiceSpec, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta, OwnerReference};
use kube::{Resource, ResourceExt};
use operator_api::consts::{DEFAULT_BACKUP_DIR, DEFAULT_DATA_DIR};

use crate::consts::{
    ANNOTATION_INHERIT_LABELS_PREFIX, DEFAULT_SIDECAR_PORT, DEFAULT_XLINE_PORT,
    LABEL_CLUSTER_COMPONENT, LABEL_CLUSTER_NAME, LABEL_OPERATOR_VERSION, SIDECAR_PORT_NAME,
    XLINE_POD_NAME_ENV, XLINE_PORT_NAME,
};
use crate::crd::v1alpha1::Cluster;

/// Read objects from `XlineCluster`
pub(crate) struct Extractor<'a> {
    /// `XlineCluster`
    cluster: &'a Cluster,
}

/// The component of `XlineCluster`
#[derive(Copy, Clone)]
pub(crate) enum Component {
    /// A xline node
    Nodes,
    /// A service
    Service,
    /// A backup job
    BackupJob,
}

impl Component {
    /// Get the component name
    fn label(&self) -> &str {
        match *self {
            Component::Nodes => "nodes",
            Component::Service => "svc",
            Component::BackupJob => "backup",
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
    pub(crate) fn extract_ports(&self) -> (ContainerPort, ContainerPort, Vec<ServicePort>) {
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
    pub(crate) fn extract_pvc_template(&self) -> Vec<PersistentVolumeClaim> {
        self.cluster
            .spec
            .backup
            .iter()
            .filter_map(|spec| spec.storage.as_pvc().cloned())
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
            .filter_map(|spec| spec.storage.as_pvc().cloned())
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
    #[allow(clippy::expect_used)] // it is ok because xlinecluster always populated from the apiserver
    fn extract_owner_ref(&self) -> OwnerReference {
        self.cluster
            .controller_owner_ref(&())
            .expect("metadata doesn't have name or uid")
    }

    /// Extract name, namespace
    #[allow(clippy::expect_used)] // it is ok because xlinecluster has field validation
    pub(crate) fn extract_id(&self) -> (&str, &str) {
        let name = self
            .cluster
            .metadata
            .name
            .as_deref()
            .expect("xlinecluster resource should have a name");
        let namespace = self
            .cluster
            .metadata
            .namespace
            .as_deref()
            .expect("xlinecluster resource should have a namespace");
        (name, namespace)
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

    /// Get the selector labels
    fn selector_labels(name: &str, component: Component) -> BTreeMap<String, String> {
        BTreeMap::from([
            (LABEL_CLUSTER_NAME.to_owned(), name.to_owned()),
            (
                LABEL_CLUSTER_COMPONENT.to_owned(),
                component.label().to_owned(),
            ),
        ])
    }

    /// Get the general metadata
    fn general_metadata(&self, component: Component) -> ObjectMeta {
        let extractor = Extractor::new(self.cluster.as_ref());
        let mut labels = extractor.extract_inherit_labels();
        let (name, namespace) = extractor.extract_id();
        let owner_ref = extractor.extract_owner_ref();
        labels.extend(Self::selector_labels(name, component));
        _ = labels.insert(
            LABEL_OPERATOR_VERSION.to_owned(),
            env!("CARGO_PKG_VERSION").to_owned(),
        );
        ObjectMeta {
            labels: Some(labels), // it is used in selector
            name: Some(Self::component_name(name, component)),
            namespace: Some(namespace.to_owned()), // all subresources share the same namespace
            owner_references: Some(vec![owner_ref]), // allow k8s GC to automatically clean up itself when `XlineCluster` is deleted
            ..ObjectMeta::default()
        }
    }

    /// Get the node headless service
    pub(crate) fn node_service(&self) -> Service {
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
                            Component::Nodes.label().to_owned(),
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
    fn mount_volume_on_container(&self, container: &mut Container) {
        let extractor = Extractor::new(self.cluster.as_ref());
        let volume_mount = extractor.extract_additional_volume_mount();
        container
            .volume_mounts
            .get_or_insert(vec![])
            .extend(volume_mount);
    }

    /// Set the entrypoint of the container
    fn set_command(&self, container: &mut Container) {
        let size = self.cluster.spec.size;
        let extractor = Extractor::new(self.cluster.as_ref());
        let (name, namespace) = extractor.extract_id();
        let (xline_port, _, _) = extractor.extract_ports();
        let svc_name = Self::component_name(name, Component::Service);
        let mut members = vec![];
        for i in 0..=size {
            let node_name = format!("{}-{i}", Self::component_name(name, Component::Nodes));
            members.push(format!(
                "{node_name}={node_name}.{svc_name}.{namespace}.svc.{}:{}",
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
    fn xline_container(&self) -> Container {
        let mut container = self.cluster.spec.container.clone();
        self.mount_volume_on_container(&mut container);
        self.set_command(&mut container);
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
    pub(crate) fn pod_spec(&self) -> PodTemplateSpec {
        let extractor = Extractor::new(self.cluster.as_ref());
        let (name, _) = extractor.extract_id();
        let xline = self.xline_container();
        let labels = Self::selector_labels(name, Component::Nodes);
        PodTemplateSpec {
            metadata: Some(ObjectMeta {
                labels: Some(labels),
                ..ObjectMeta::default()
            }),
            spec: Some(PodSpec {
                init_containers: Some(vec![]),
                containers: vec![xline],
                affinity: self.cluster.spec.affinity.clone(),
                ..PodSpec::default()
            }),
        }
    }

    /// Get the statefulset
    pub(crate) fn sts(&self) -> StatefulSet {
        let size = self.cluster.spec.size;
        let extractor = Extractor::new(self.cluster.as_ref());
        let (name, _) = extractor.extract_id();
        let labels = Self::selector_labels(name, Component::Nodes);
        StatefulSet {
            metadata: self.general_metadata(Component::Nodes),
            spec: Some(StatefulSetSpec {
                replicas: Some(
                    i32::try_from(size)
                        .unwrap_or_else(|_| unreachable!("size should not overflow i32::MAX")),
                ),
                selector: LabelSelector {
                    match_expressions: None,
                    match_labels: Some(labels),
                },
                service_name: Self::component_name(name, Component::Service),
                volume_claim_templates: Some(extractor.extract_pvc_template()),
                template: self.pod_spec(),
                ..StatefulSetSpec::default()
            }),
            status: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static CLUSTER_1: &str = r#"
apiVersion: xlineoperator.xline.cloud/v1alpha
kind: XlineCluster
metadata:
  name: my-xline-cluster
  labels:
    app: my-xline-cluster
    appNamespace: default
  annotations:
    xlineoperator.datenlord.io/inherit-label-prefix: "app"
spec:
  size: 3
  container:
    image: "datenlord/xline"
    name: "my-xline"
    ports:
      - containerPort: 2379
        name: xline
    "#;

    static CLUSTER_2: &str = r#"
apiVersion: xlineoperator.xline.cloud/v1alpha
kind: XlineCluster
metadata:
  name: my-xline-cluster
spec:
  size: 5
  container:
    image: "datenlord/xline"
    name: "my-xline"
    ports:
      - containerPort: 3000
        name: xline
      - containerPort: 3001
        name: sidecar
  data:
    metadata:
      name: my-xline-cluster-data
    spec:
      accessModes: [ "ReadWriteOnce" ]
      storageClassName: "my-storage-class"
      resources:
        requests:
          storage: 1Gi
    "#;

    static CLUSTER_3: &str = r#"
apiVersion: xlineoperator.datenlord.io/v1alpha
kind: XlineCluster
metadata:
  name: my-xline-cluster
spec:
  size: 3
  backup:
    cron: "*/15 * * * *"
    pvc:
      metadata:
        name: backup-pvc
      spec:
        storageClassName: xline-backup
        accessModes: [ "ReadWriteOnce" ]
        resources:
          requests:
            storage: 1Gi
  container:
    image: "datenlord/xline"
    name: "my-xline"
    ports:
      - containerPort: 2379
        name: xline
    "#;

    static CLUSTER_4: &str = r#"
apiVersion: xlineoperator.datenlord.io/v1alpha
kind: XlineCluster
metadata:
  name: my-xline-cluster
spec:
  size: 3
  container:
    image: "datenlord/xline"
    name: "my-xline"
    ports:
      - containerPort: 2379
        name: xline
  pvcs:
    - metadata:
        name: xline-pvc
      spec:
        storageClassName: xline-backup
        accessModes: [ "ReadWriteOnce" ]
        resources:
          requests:
            storage: 1Gi
    "#;

    fn after_apiserver(cluster: &mut Cluster) {
        cluster.metadata.namespace = Some("default".to_owned()); // use default namespace if no namespace specified in the yaml
        cluster.metadata.uid = Some("this-is-a-random-uid".to_owned());
    }

    #[test]
    fn extract_ports_should_work() {
        for (cluster_raw, xline, sidecar) in [
            (CLUSTER_1, 2379, 2380),
            (CLUSTER_2, 3000, 3001),
            (CLUSTER_3, 2379, 2380),
            (CLUSTER_4, 2379, 2380),
        ] {
            let mut cluster: Cluster = serde_yaml::from_str(cluster_raw).unwrap();
            after_apiserver(&mut cluster);
            let extractor = Extractor::new(&cluster);
            let (xline_port, sidecar_port, service_ports) = extractor.extract_ports();
            assert_eq!(xline_port.container_port, xline);
            assert_eq!(sidecar_port.container_port, sidecar);
            assert_eq!(service_ports.len(), 2);
            assert_eq!(service_ports[0].name.as_deref(), Some(XLINE_PORT_NAME));
            assert_eq!(service_ports[0].port, xline);
            assert_eq!(service_ports[1].name.as_deref(), Some(SIDECAR_PORT_NAME));
            assert_eq!(service_ports[1].port, sidecar);
        }
    }

    #[test]
    fn extract_id_should_work() {
        for cluster_raw in [CLUSTER_1, CLUSTER_2, CLUSTER_3, CLUSTER_4] {
            let mut cluster: Cluster = serde_yaml::from_str(cluster_raw).unwrap();
            after_apiserver(&mut cluster);
            let extractor = Extractor::new(&cluster);
            let (name, namespace) = extractor.extract_id();
            assert_eq!(name, "my-xline-cluster");
            assert_eq!(namespace, "default");
        }
    }

    #[test]
    fn extract_pvc_should_work() {
        let mut cluster: Cluster = serde_yaml::from_str(CLUSTER_1).unwrap();
        after_apiserver(&mut cluster);
        let extractor = Extractor::new(&cluster);
        let pvcs = extractor.extract_pvc_template();
        assert_eq!(pvcs.len(), 0);

        let mut cluster: Cluster = serde_yaml::from_str(CLUSTER_2).unwrap();
        after_apiserver(&mut cluster);
        let extractor = Extractor::new(&cluster);
        let pvcs = extractor.extract_pvc_template();
        assert_eq!(pvcs.len(), 1);
        assert_eq!(
            pvcs[0].metadata.name.as_deref(),
            Some("my-xline-cluster-data")
        );

        let mut cluster: Cluster = serde_yaml::from_str(CLUSTER_3).unwrap();
        after_apiserver(&mut cluster);
        let extractor = Extractor::new(&cluster);
        let pvcs = extractor.extract_pvc_template();
        assert_eq!(pvcs.len(), 1);
        assert_eq!(pvcs[0].metadata.name.as_deref(), Some("backup-pvc"));

        let mut cluster: Cluster = serde_yaml::from_str(CLUSTER_4).unwrap();
        after_apiserver(&mut cluster);
        let extractor = Extractor::new(&cluster);
        let pvcs = extractor.extract_pvc_template();
        assert_eq!(pvcs.len(), 1);
        assert_eq!(pvcs[0].metadata.name.as_deref(), Some("xline-pvc"));
    }

    #[test]
    fn extract_volume_mount_should_work() {
        let mut cluster: Cluster = serde_yaml::from_str(CLUSTER_1).unwrap();
        after_apiserver(&mut cluster);
        let extractor = Extractor::new(&cluster);
        let volume_mount = extractor.extract_additional_volume_mount();
        assert_eq!(volume_mount.len(), 0);

        let mut cluster: Cluster = serde_yaml::from_str(CLUSTER_2).unwrap();
        after_apiserver(&mut cluster);
        let extractor = Extractor::new(&cluster);
        let volume_mount = extractor.extract_additional_volume_mount();
        assert_eq!(volume_mount.len(), 1);
        assert_eq!(volume_mount[0].name, "my-xline-cluster-data");
        assert_eq!(volume_mount[0].mount_path, DEFAULT_DATA_DIR);

        let mut cluster: Cluster = serde_yaml::from_str(CLUSTER_3).unwrap();
        after_apiserver(&mut cluster);
        let extractor = Extractor::new(&cluster);
        let volume_mount = extractor.extract_additional_volume_mount();
        assert_eq!(volume_mount.len(), 1);
        assert_eq!(volume_mount[0].name, "backup-pvc");
        assert_eq!(volume_mount[0].mount_path, DEFAULT_BACKUP_DIR);

        let mut cluster: Cluster = serde_yaml::from_str(CLUSTER_4).unwrap();
        after_apiserver(&mut cluster);
        let extractor = Extractor::new(&cluster);
        let volume_mount = extractor.extract_additional_volume_mount();
        assert_eq!(volume_mount.len(), 0);
    }

    #[test]
    fn extract_owner_ref_should_work() {
        for cluster_raw in [CLUSTER_1, CLUSTER_2, CLUSTER_3, CLUSTER_4] {
            let mut cluster: Cluster = serde_yaml::from_str(cluster_raw).unwrap();
            after_apiserver(&mut cluster);
            let extractor = Extractor::new(&cluster);
            let owner_ref = extractor.extract_owner_ref();
            assert_eq!(owner_ref.name, "my-xline-cluster");
        }
    }

    #[test]
    fn extract_inherit_labels_should_work() {
        let mut cluster: Cluster = serde_yaml::from_str(CLUSTER_1).unwrap();
        after_apiserver(&mut cluster);
        let extractor = Extractor::new(&cluster);
        let labels = extractor.extract_inherit_labels();
        assert_eq!(labels.len(), 2);
        assert_eq!(&labels["app"], "my-xline-cluster");
        assert_eq!(&labels["appNamespace"], "default");
    }

    #[test]
    fn factory_component_name_should_work() {
        assert_eq!(
            Factory::component_name("my-xline-cluster", Component::Nodes),
            "my-xline-cluster-nodes"
        );
        assert_eq!(
            Factory::component_name("my-xline-cluster", Component::Service),
            "my-xline-cluster-svc"
        );
        assert_eq!(
            Factory::component_name("my-xline-cluster", Component::BackupJob),
            "my-xline-cluster-backup"
        );
    }

    #[test]
    fn factory_general_metadata_should_work() {
        let cluster_1_metadata = r#"
labels:
  app: my-xline-cluster
  appNamespace: default
  xlinecluster/component: nodes
  xlinecluster/name: my-xline-cluster
  xlinecluster/operator-version: 0.1.0
name: my-xline-cluster-nodes
namespace: default
ownerReferences:
- apiVersion: xlineoperator.xline.cloud/v1alpha1
  controller: true
  kind: XlineCluster
  name: my-xline-cluster
  uid: this-is-a-random-uid
        "#
        .trim();

        let cluster_other_metadata = r#"
labels:
  xlinecluster/component: nodes
  xlinecluster/name: my-xline-cluster
  xlinecluster/operator-version: 0.1.0
name: my-xline-cluster-nodes
namespace: default
ownerReferences:
- apiVersion: xlineoperator.xline.cloud/v1alpha1
  controller: true
  kind: XlineCluster
  name: my-xline-cluster
  uid: this-is-a-random-uid
        "#
        .trim();

        for (cluster_raw, metadata_str) in [
            (CLUSTER_1, cluster_1_metadata),
            (CLUSTER_2, cluster_other_metadata),
            (CLUSTER_3, cluster_other_metadata),
            (CLUSTER_4, cluster_other_metadata),
        ] {
            let mut cluster: Cluster = serde_yaml::from_str(cluster_raw).unwrap();
            after_apiserver(&mut cluster);
            let factory = Factory::new(Arc::new(cluster), "cluster.local");
            let metadata = factory.general_metadata(Component::Nodes);
            let outputs = serde_yaml::to_string(&metadata).unwrap();
            assert_eq!(outputs.trim(), metadata_str);
        }
    }

    #[test]
    fn factory_node_service_should_work() {
        let spec = r#"
spec:
  ports:
  - name: xline
    port: 2379
  - name: sidecar
    port: 2380
  selector:
    xlinecluster/component: nodes
    xlinecluster/name: my-xline-cluster
        "#
        .trim();
        for cluster_raw in [CLUSTER_1, CLUSTER_3, CLUSTER_4] {
            let mut cluster: Cluster = serde_yaml::from_str(cluster_raw).unwrap();
            after_apiserver(&mut cluster);
            let factory = Factory::new(Arc::new(cluster), "cluster.local");
            let service = factory.node_service();
            let outputs = serde_yaml::to_string(&service).unwrap();
            assert!(outputs.contains(spec));
        }
    }
}
