/// Controller v1alpha
mod v1alpha;
/// Controller v1alpha1
mod v1alpha1;

/// Current controller of cluster
pub(crate) type Controller = v1alpha::ClusterController;
