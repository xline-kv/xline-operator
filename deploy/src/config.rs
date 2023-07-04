use clap::Parser;

/// Deploy operator config
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// The address on which the HTTP server will listen to
    #[arg(long, default_value = "0.0.0.0:8080")]
    listen_addr: String,
    /// The namespace to deploy
    #[arg(long, default_value = "default")]
    namespace: String,
    /// Enable operator to work in all namespaces
    #[arg(long, default_value = "false")]
    cluster_wide: bool,
    /// Whether to create CRD upon startup
    #[arg(long, default_value = "true")]
    create_crd: bool,
}
