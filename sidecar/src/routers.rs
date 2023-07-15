use std::sync::Arc;

use axum::http::StatusCode;
use axum::{Extension, Json};
use tokio::sync::Mutex;
use utils::health::{check_backup_volume, check_data_volume};

use crate::types::{MembershipChange, StatePayload};
use crate::xline::XlineHandle;

/// Return the current health condition according to the current node's storage volume and network status
/// The network status is verified upon returning the HTTP response.
#[allow(clippy::unused_async)] // This is required in axum
pub(crate) async fn health() -> StatusCode {
    if !check_data_volume() {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    if !check_backup_volume() {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    StatusCode::OK
}

/// Backup hook
pub(crate) async fn backup(Extension(handle): Extension<Arc<XlineHandle>>) -> StatusCode {
    if handle.backup().await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    StatusCode::OK
}

/// State route
pub(crate) async fn state(
    Extension(state): Extension<Arc<Mutex<StatePayload>>>,
) -> (StatusCode, Json<StatePayload>) {
    let state = state.lock().await;
    let payload = state.clone();
    (StatusCode::OK, Json(payload))
}

/// Proactively initiate membership change.
#[allow(clippy::unused_async)] // TODO remove when it is implemented
pub(crate) async fn membership(_change: Json<MembershipChange>) -> StatusCode {
    // TODO handle proactively membership change in server handle
    StatusCode::OK
}
