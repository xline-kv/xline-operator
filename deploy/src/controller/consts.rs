use std::time::Duration;

/// Default recover requeue duration
pub(super) const DEFAULT_REQUEUE_DURATION: Duration = Duration::from_secs(600);
/// The field manager identifier of deploy operator
pub(super) const FIELD_MANAGER: &str = "xlineoperator.datenlord.io/deployoperator";
/// The emptyDir volume name of each pod
pub(crate) const DATA_EMPTY_DIR_NAME: &str = "xline-data-empty-dir";
