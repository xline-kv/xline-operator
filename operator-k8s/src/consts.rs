#![allow(unused)] // TODO remove

use std::time::Duration;

/// The default requeue duration to achieve eventual consistency
pub(crate) const DEFAULT_REQUEUE_DURATION: Duration = Duration::from_secs(600);
/// The field manager identifier of xline operator
pub(crate) const FIELD_MANAGER: &str = "xlineoperator.datenlord.io/operator";
/// The image used for cronjob to trigger backup
/// The following command line tool should be available in this image
/// 1. curl
/// 2. sh
pub(crate) const CRONJOB_IMAGE: &str = "curlimages/curl";
/// The name of xline port, the port with this name is considered to be the port of xline
pub(crate) const XLINE_PORT_NAME: &str = "xline";
/// The name of sidecar port, the port with this name is considered to be the port of sidecar
pub(crate) const SIDECAR_PORT_NAME: &str = "sidecar";
/// The default xline port
pub(crate) const DEFAULT_XLINE_PORT: i32 = 2379;
/// The default sidecar port
pub(crate) const DEFAULT_SIDECAR_PORT: i32 = 2380;
/// The environment name of the xline pod name
pub(crate) const XLINE_POD_NAME_ENV: &str = "XLINE_POD_NAME";
/// The annotation used to inherit labels in `XlineCluster`
pub(crate) const ANNOTATION_INHERIT_LABELS_PREFIX: &str =
    "xlineoperator.datenlord.io/inherit-label-prefix";
/// The label attach to subresources, indicate the xlinecluster name
pub(crate) const LABEL_CLUSTER_NAME: &str = "xlinecluster/name";
/// The label attach to subresources, indicate the component type of this subresource
pub(crate) const LABEL_CLUSTER_COMPONENT: &str = "xlinecluster/component";
/// Indicate the version of operator that creates this subresource
pub(crate) const LABEL_OPERATOR_VERSION: &str = "xlinecluster/operator-version";
