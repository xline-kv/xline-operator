/// CRD v1
mod v1;

/// Current CRD `XineCluster`
pub(crate) type Cluster = v1::Cluster;
/// Current CRD Backup Storage Specification
pub(crate) type StorageSpec = v1::StorageSpec;
