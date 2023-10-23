use clap::Parser;

/// Xline operator config
#[derive(Debug, Parser)]
#[non_exhaustive]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// The namespace to work, default to cluster wide
    #[arg(long, value_parser=namespace_mode_parser, default_value = "")]
    pub namespace: Namespace,
    /// The address on which the heartbeat HTTP server will listen to
    #[arg(long, default_value = "0.0.0.0:8080")]
    pub listen_addr: String,
    /// Whether to create CRD regardless of current version on k8s
    #[arg(long, default_value = "false")]
    pub create_crd: bool,
    /// Whether to enable auto migration if CRD version is less than current version
    #[arg(long, default_value = "false")]
    pub auto_migration: bool,
    /// The kubernetes cluster DNS suffix
    #[arg(long, default_value = "cluster.local")]
    pub cluster_suffix: String,
    /// Maximum interval between accepted `HeartbeatStatus`
    #[arg(long, default_value = "2")]
    pub heartbeat_period: u64,
    /// Sidecar unreachable counter threshold
    #[arg(long, default_value = "4")]
    pub unreachable_thresh: usize,
}

/// The namespace to work, `ClusterWide` means work with all namespaces
#[allow(clippy::exhaustive_enums)] // it is clear that this enum is exhaustive
#[derive(Clone, Debug)]
pub enum Namespace {
    /// A single namespace
    Single(String),
    /// All namespaces
    ClusterWide,
}

/// parse namespace mode
#[allow(clippy::unnecessary_wraps)] // required by clap
fn namespace_mode_parser(value: &str) -> Result<Namespace, String> {
    if value.is_empty() {
        return Ok(Namespace::ClusterWide);
    }
    Ok(Namespace::Single(value.to_owned()))
}
