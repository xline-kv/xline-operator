use clippy_utilities::NumericCast;
use prometheus::{Histogram, HistogramOpts, HistogramTimer, IntCounterVec, Opts, Registry};

use std::iter::repeat;
use std::ops::Mul;

/// Controller v1alpha
mod v1alpha;
/// Controller v1alpha1
mod v1alpha1;

/// Current controller of cluster
pub(crate) type Controller = v1alpha::ClusterController;

/// Cluster metrics
pub(crate) struct ClusterMetrics {
    /// Reconcile duration histogram
    reconcile_duration: Histogram,
    /// Reconcile failed count
    reconcile_failed_count: IntCounterVec,
}

/// Label error
trait LabelError {
    /// Label
    fn label(&self) -> &str;
}

impl ClusterMetrics {
    /// Create a new cluster metrics
    #[allow(clippy::expect_used)]
    pub(crate) fn new() -> Self {
        Self {
            reconcile_duration: Histogram::with_opts(
                HistogramOpts::new(
                    "operator_reconcile_duration_seconds",
                    "Duration of operator reconcile loop in seconds",
                )
                .buckets(exponential_time_bucket(0.1, 2.0, 10)),
            )
            .expect(""),
            reconcile_failed_count: IntCounterVec::new(
                Opts::new(
                    "operator_reconcile_failed_count",
                    "Number of failed times the operator reconcile loop has run",
                ),
                &["reason"],
            )
            .expect("failed to create operator_reconcile_failed_count counter"),
        }
    }

    /// Register metrics
    pub(crate) fn register(&self, registry: &Registry) -> Result<(), prometheus::Error> {
        registry.register(Box::new(self.reconcile_duration.clone()))?;
        registry.register(Box::new(self.reconcile_failed_count.clone()))
    }

    /// Record duration
    fn record_duration(&self) -> HistogramTimer {
        self.reconcile_duration.start_timer()
    }

    /// Increment failed count
    fn incr_failed_count(&self, reason: &impl LabelError) {
        self.reconcile_failed_count
            .with_label_values(&[reason.label()])
            .inc();
    }
}

/// Returns a vector of time buckets for the reconcile duration histogram.
fn exponential_time_bucket(start: f64, factor: f64, count: usize) -> Vec<f64> {
    repeat(factor)
        .enumerate()
        .take(count)
        .map(|(i, f)| start.mul(f.powi(i.numeric_cast())))
        .collect::<Vec<_>>()
}
