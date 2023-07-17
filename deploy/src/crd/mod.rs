/// CRD v1alpha
mod v1alpha;

/// Current CRD `XineCluster`
pub(crate) type Cluster = v1alpha::Cluster;
/// Current CRD Backup Storage Specification
pub(crate) type StorageSpec = v1alpha::StorageSpec;
