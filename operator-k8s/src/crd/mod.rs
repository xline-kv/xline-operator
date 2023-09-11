/// v1alpha1
/// Features:
///     1. Xline sidecar
///     2. PV backup
pub(crate) mod v1alpha1;

/// Current CRD `XineCluster`
pub(crate) use v1alpha1::Cluster;
