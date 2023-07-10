use clap::Parser;

/// Deploy operator config
#[derive(Debug, Parser)]
#[non_exhaustive]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// The address on which the HTTP server will listen to
    #[arg(long, default_value = "0.0.0.0:8080")]
    pub listen_addr: String,
    /// The namespace to deploy
    #[arg(long, default_value = "default")]
    pub namespace: String,
    /// Enable operator to work in all namespaces
    #[arg(long, default_value = "false")]
    pub cluster_wide: bool,
    /// Whether to create CRD regardless of current version on k8s
    #[arg(long, default_value = "false")]
    pub create_crd: bool,
    /// Force reconcile interval, default 600s [unit: second]
    #[arg(long, default_value = "600")]
    pub reconcile_interval: u64,
}
