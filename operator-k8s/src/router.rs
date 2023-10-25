use axum::{Extension, Json};
use flume::Sender;
use operator_api::HeartbeatStatus;
use prometheus::{Encoder, Registry};
use tracing::error;

/// metrics handler
#[allow(clippy::unused_async)] // require by axum
pub(crate) async fn metrics(Extension(registry): Extension<Registry>) -> String {
    let mut buf1 = Vec::new();
    let encoder = prometheus::TextEncoder::new();
    let metric_families = registry.gather();
    if let Err(err) = encoder.encode(&metric_families, &mut buf1) {
        error!("failed to encode custom metrics: {}", err);
        return String::new();
    }
    let mut res = String::from_utf8(buf1).unwrap_or_default();
    let mut buf2 = Vec::new();
    if let Err(err) = encoder.encode(&prometheus::gather(), &mut buf2) {
        error!("failed to encode prometheus metrics: {}", err);
        return String::new();
    }
    res.push_str(&String::from_utf8_lossy(&buf2));
    res
}

/// healthz handler
#[allow(clippy::unused_async)] // require by axum
pub(crate) async fn healthz() -> &'static str {
    "healthy"
}

/// sidecar monitor handler
#[allow(clippy::unused_async)] // require by axum
pub(crate) async fn sidecar_monitor(
    Extension(status_tx): Extension<Sender<HeartbeatStatus>>,
    Json(status): Json<HeartbeatStatus>,
) {
    if let Err(e) = status_tx.send(status) {
        error!("channel send error: {e}");
    }
}
