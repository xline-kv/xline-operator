//! xline-operator
#![deny(
    // The following are allowed by default lints according to
    // https://doc.rust-lang.org/rustc/lints/listing/allowed-by-default.html

    absolute_paths_not_starting_with_crate,
    // box_pointers, async trait must use it
    // elided_lifetimes_in_paths,  // allow anonymous lifetime
    explicit_outlives_requirements,
    keyword_idents,
    macro_use_extern_crate,
    meta_variable_misuse,
    missing_abi,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    // must_not_suspend, unstable
    non_ascii_idents,
    // non_exhaustive_omitted_patterns, unstable
    noop_method_call,
    pointer_structural_match,
    rust_2021_incompatible_closure_captures,
    rust_2021_incompatible_or_patterns,
    rust_2021_prefixes_incompatible_syntax,
    rust_2021_prelude_collisions,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unsafe_code,
    unsafe_op_in_unsafe_fn,
    unstable_features,
    // unused_crate_dependencies, the false positive case blocks us
    unused_extern_crates,
    unused_import_braces,
    unused_lifetimes,
    unused_qualifications,
    unused_results,
    variant_size_differences,
    warnings, // treat all warnings as errors

    clippy::all,
    clippy::pedantic,
    clippy::cargo,

    // The followings are selected restriction lints for rust 1.57
    clippy::as_conversions,
    clippy::clone_on_ref_ptr,
    clippy::create_dir,
    clippy::dbg_macro,
    clippy::decimal_literal_representation,
    // clippy::default_numeric_fallback, too verbose when dealing with numbers
    clippy::disallowed_script_idents,
    clippy::else_if_without_else,
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::exit,
    clippy::expect_used,
    clippy::filetype_is_file,
    clippy::float_arithmetic,
    clippy::float_cmp_const,
    clippy::get_unwrap,
    clippy::if_then_some_else_none,
    // clippy::implicit_return, it's idiomatic Rust code.
    clippy::indexing_slicing,
    // clippy::inline_asm_x86_att_syntax, stick to intel syntax
    clippy::inline_asm_x86_intel_syntax,
    clippy::integer_arithmetic,
    // clippy::integer_division, required in the project
    clippy::let_underscore_must_use,
    clippy::lossy_float_literal,
    clippy::map_err_ignore,
    clippy::mem_forget,
    clippy::missing_docs_in_private_items,
    clippy::missing_enforced_import_renames,
    clippy::missing_inline_in_public_items,
    // clippy::mod_module_files, mod.rs file is used
    clippy::modulo_arithmetic,
    clippy::multiple_inherent_impl,
    // clippy::panic, allow in application code
    // clippy::panic_in_result_fn, not necessary as panic is banned
    clippy::pattern_type_mismatch,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::rc_buffer,
    clippy::rc_mutex,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::same_name_method,
    clippy::self_named_module_files,
    // clippy::shadow_reuse, it’s a common pattern in Rust code
    // clippy::shadow_same, it’s a common pattern in Rust code
    clippy::shadow_unrelated,
    clippy::str_to_string,
    clippy::string_add,
    clippy::string_to_string,
    clippy::todo,
    clippy::unimplemented,
    clippy::unnecessary_self_imports,
    clippy::unneeded_field_pattern,
    // clippy::unreachable, allow unreachable panic, which is out of expectation
    clippy::unwrap_in_result,
    clippy::unwrap_used,
    // clippy::use_debug, debug is allow for debug log
    clippy::verbose_file_reads,
    clippy::wildcard_enum_match_arm,

    // The followings are selected lints from 1.61.0 to 1.67.1
    clippy::as_ptr_cast_mut,
    clippy::derive_partial_eq_without_eq,
    clippy::empty_drop,
    clippy::empty_structs_with_brackets,
    clippy::format_push_string,
    clippy::iter_on_empty_collections,
    clippy::iter_on_single_items,
    clippy::large_include_file,
    clippy::manual_clamp,
    clippy::suspicious_xor_used_as_pow,
    clippy::unnecessary_safety_comment,
    clippy::unnecessary_safety_doc,
    clippy::unused_peekable,
    clippy::unused_rounding,

    // The followings are selected restriction lints from rust 1.68.0 to 1.70.0
    // clippy::allow_attributes, still unstable
    clippy::impl_trait_in_params,
    clippy::let_underscore_untyped,
    clippy::missing_assert_message,
    clippy::multiple_unsafe_ops_per_block,
    clippy::semicolon_inside_block,
    // clippy::semicolon_outside_block, already used `semicolon_inside_block`
    clippy::tests_outside_test_module
)]
#![allow(
    clippy::panic, // allow debug_assert, panic in production code
    clippy::multiple_crate_versions, // caused by the dependency, can't be fixed
)]

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Parser;
use tracing::debug;

use xline_sidecar::operator::Operator;
use xline_sidecar::types::{Backup, Config};

/// Command line interface
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The name of this node
    #[arg(long)]
    name: String, // used in xline and deployment operator to identify this node
    /// The host ip of each member, [node_name] -> [node_host]
    #[arg(long, value_parser = parse_members)]
    members: HashMap<String, String>,
    /// The xline server port
    #[arg(long)]
    xline_port: u16,
    /// Operator web server port
    #[arg(long)]
    operator_port: u16,
    /// The xline container name
    #[arg(long)]
    container_name: String,
    /// Check health interval, default 20 [unit: seconds]
    #[arg(long, default_value = "20")]
    check_interval: u64,
    /// Enable backup, choose a storage type, e.g. s3:bucket_name or pv:/path/to/dir
    #[arg(long, value_parser=parse_backup_type)]
    backup: Option<Backup>,
    /// The xline executable path, default "xline"
    #[arg(long, default_value = "xline")]
    xline_executable: String,
    /// Storage engine used in xline
    #[arg(long)]
    storage_engine: String,
    /// The directory path contains xline server data if the storage_engine is rocksdb
    #[arg(long)]
    data_dir: PathBuf,
    /// Whether this node is leader or not
    #[arg(long, default_value = "false")]
    is_leader: bool,
    /// Additional arguments, it will be appended behind the required parameters,
    /// e.g "--jaeger_offline true"
    #[arg(long)]
    additional: Option<String>,
}

impl From<Cli> for Config {
    fn from(value: Cli) -> Self {
        let mut config = Self {
            start_cmd: String::new(),
            name: value.name.clone(),
            container_name: value.container_name,
            xline_port: value.xline_port,
            operator_port: value.operator_port,
            check_interval: std::time::Duration::from_secs(value.check_interval),
            backup: value.backup,
            members: value.members,
        };
        config.start_cmd = format!(
            "{} --name {} --members {} --storage-engine {} --data-dir {}",
            value.xline_executable,
            value.name,
            config
                .xline_members()
                .into_iter()
                .map(|(name, addr)| format!("{name}={addr}"))
                .collect::<Vec<_>>()
                .join(","),
            value.storage_engine,
            value.data_dir.to_string_lossy(),
        );
        if value.is_leader {
            config.start_cmd.push(' ');
            config.start_cmd.push_str("--is-leader");
        }
        if let Some(additional) = value.additional {
            config.start_cmd.push(' ');
            let pat: &[_] = &['\'', '"'];
            config.start_cmd.push_str(additional.trim_matches(pat));
        }
        config
    }
}

/// parse backup type
fn parse_backup_type(value: &str) -> Result<Backup, String> {
    debug!("parse backup type: {}", value);
    let mut items: Vec<_> = value.split([':', ' ', ',', '-']).collect();
    if items.is_empty() {
        return Err("backup type is empty".to_owned());
    }
    let backup_type = items.remove(0);
    match backup_type {
        "s3" => {
            if items.len() != 1 {
                return Err(format!(
                    "s3 backup type requires 1 arguments, got {}",
                    items.len()
                ));
            }
            let bucket = items.remove(0).to_owned();
            Ok(Backup::S3 { bucket })
        }
        "pv" => {
            if items.len() != 1 {
                return Err(format!(
                    "pv backup type requires 1 argument, got {}",
                    items.len()
                ));
            }
            let path = items.remove(0).to_owned();
            Ok(Backup::PV {
                path: PathBuf::from(path),
            })
        }
        _ => Err(format!("unknown backup type: {backup_type}")),
    }
}

/// parse members from string
/// # Errors
/// Return error when pass wrong args
#[inline]
pub fn parse_members(s: &str) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for pair in s.split(',') {
        if let Some((id, addr)) = pair.split_once('=') {
            let _ignore = map.insert(id.to_owned(), addr.to_owned());
        } else {
            return Err(anyhow!(
                "parse the pair '{}' error, expect '<id>=<addr>'",
                pair
            ));
        }
    }
    Ok(map)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    debug!("{:?}", cli);

    Operator::new(cli.into()).run().await
}

#[cfg(test)]
mod test {
    use crate::Cli;
    use clap::Parser;
    use std::collections::HashMap;
    use xline_sidecar::types::{Backup, Config};

    fn full_parameter() -> Vec<&'static str> {
        vec![
            "sidecar_exe",
            "--name=node1",
            "--members=node1=127.0.0.1",
            "--xline-port=2379",
            "--operator-port=2380",
            "--container-name=xline",
            "--check-interval=60",
            "--backup=s3:bucket_name",
            "--xline-executable=/usr/local/bin/xline",
            "--storage-engine=rocksdb",
            "--data-dir=/usr/local/xline/data-dir",
            "--is-leader",
            "--additional='--auth-public-key /mnt/public.pem --auth-private-key /mnt/private.pem'",
        ]
    }

    #[test]
    fn test_parse_cli_should_success() {
        let cli = Cli::parse_from(full_parameter());
        assert_eq!(cli.name, "node1");
        assert_eq!(
            cli.members,
            HashMap::from([("node1".to_owned(), "127.0.0.1".to_owned())])
        );
        assert_eq!(cli.xline_port, 2379);
        assert_eq!(cli.operator_port, 2380);
        assert_eq!(cli.container_name, "xline");
        assert_eq!(cli.check_interval, 60);
        assert_eq!(
            cli.backup,
            Some(Backup::S3 {
                bucket: "bucket_name".to_owned(),
            })
        );
        assert_eq!(cli.xline_executable, "/usr/local/bin/xline");
        assert_eq!(cli.storage_engine, "rocksdb");
        assert_eq!(cli.data_dir.to_string_lossy(), "/usr/local/xline/data-dir");
        assert!(cli.is_leader);
        assert_eq!(
            cli.additional,
            Some(
                "'--auth-public-key /mnt/public.pem --auth-private-key /mnt/private.pem'"
                    .to_owned()
            )
        );
    }

    #[test]
    fn test_gen_start_cmd() {
        let config: Config = Cli::parse_from(full_parameter()).into();
        assert_eq!(config.start_cmd, "/usr/local/bin/xline --name node1 --members node1=127.0.0.1:2379 --storage-engine rocksdb --data-dir /usr/local/xline/data-dir --is-leader --auth-public-key /mnt/public.pem --auth-private-key /mnt/private.pem");
    }
}
