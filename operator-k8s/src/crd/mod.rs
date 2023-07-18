/// v1alpha
/// Features:
///     1. Basic deployment
///     2. Scale cluster
///     3. Xline data PV
pub(crate) mod v1alpha;

/// v1alpha1
/// Features:
///     1. Xline sidecar
///     2. PV backup
pub(crate) mod v1alpha1;

/// Current CRD `XineCluster`
pub(crate) type Cluster = v1alpha::Cluster;
