use anyhow::Result;
use clap::Parser;
use xline_operator::config::Config;
use xline_operator::operator::Operator;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let config = Config::parse();
    Operator::new(config).generate_crds()
}
