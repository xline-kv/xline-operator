#![allow(clippy::expect_used)] // it is safe to unwrap static metrics

use clippy_utilities::NumericCast;
use lazy_static::lazy_static;
use prometheus::{Encoder, Histogram, HistogramOpts, IntCounterVec, Opts, Registry};
use std::iter::repeat;
use std::ops::Mul;
use tracing::error;

/// Returns a vector of time buckets for the reconcile duration histogram.
fn exponential_time_bucket(start: f64, factor: f64, count: usize) -> Vec<f64> {
    repeat(factor)
        .enumerate()
        .take(count)
        .map(|(i, f)| start.mul(f.powi(i.numeric_cast())))
        .collect::<Vec<_>>()
}

lazy_static! {
    pub(crate) static ref REGISTRY: Registry = Registry::new();
    pub(crate) static ref RECONCILE_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new(
            "operator_reconcile_duration_seconds",
            "Duration of operator reconcile loop in seconds",
        )
        .buckets(exponential_time_bucket(0.1, 2.0, 10))
    )
    .expect("failed to create operator_reconcile_duration_seconds histogram");
    pub(crate) static ref RECONCILE_FAILED_COUNT: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "operator_reconcile_failed_count",
            "Number of failed times the operator reconcile loop has run"
        ),
        &["reason"]
    )
    .expect("failed to create operator_reconcile_failed_count counter");
}

/// init metrics
pub(crate) fn init() {
    REGISTRY
        .register(Box::new(RECONCILE_DURATION.clone()))
        .expect("failed to register operator_reconcile_duration_seconds histogram");
    REGISTRY
        .register(Box::new(RECONCILE_FAILED_COUNT.clone()))
        .expect("failed to register operator_reconcile_failed_count counter");
}

/// metrics handler
#[allow(clippy::unused_async)] // require by axum
pub(crate) async fn metrics() -> String {
    let mut buf1 = Vec::new();
    let encoder = prometheus::TextEncoder::new();
    let metric_families = REGISTRY.gather();
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
