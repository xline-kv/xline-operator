use std::time::Duration;

/// The default requeue duration to achieve eventual consistency
pub(super) const DEFAULT_REQUEUE_DURATION: Duration = Duration::from_secs(600);
/// The field manager identifier of deploy operator
pub(super) const FIELD_MANAGER: &str = "xlineoperator.datenlord.io/deployoperator";
/// The emptyDir volume name of each pod if there is no data pvc specified
pub(crate) const DATA_EMPTY_DIR_NAME: &str = "xline-data-empty-dir";
