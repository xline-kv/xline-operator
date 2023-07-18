use std::time::Duration;

/// The default requeue duration to achieve eventual consistency
pub(super) const DEFAULT_REQUEUE_DURATION: Duration = Duration::from_secs(600);
/// The field manager identifier of xline operator
pub(super) const FIELD_MANAGER: &str = "xlineoperator.datenlord.io";
/// The emptyDir volume name of each pod if there is no data pvc specified
pub(crate) const DATA_EMPTY_DIR_NAME: &str = "xline-data-empty-dir";
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
