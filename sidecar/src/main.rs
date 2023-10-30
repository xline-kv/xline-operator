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
    // clippy::self_named_module_files, false positive
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

use std::borrow::ToOwned;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use operator_api::consts::DEFAULT_DATA_DIR;
use operator_api::XlineConfig;
use tracing::debug;
use xline_sidecar::sidecar::Sidecar;
use xline_sidecar::types::{
    BackendConfig, BackupConfig, Config, MemberConfig, MonitorConfig, RegistryConfig,
};

/// `DEFAULT_DATA_DIR` to String
fn default_data_dir() -> String {
    DEFAULT_DATA_DIR.to_owned()
}

/// Command line interface
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The name of this sidecar, and is shared with xline node name
    #[arg(long)]
    name: String,
    /// The cluster name of this sidecar
    #[arg(long)]
    cluster_name: String,
    /// The host of each member at initial, [node_name] -> [node_host]
    /// Need to include at least the pair of this node
    #[arg(long, value_parser = parse_members)]
    init_members: HashMap<String, String>,
    /// The xline server port
    #[arg(long)]
    xline_port: u16,
    /// Sidecar web server port
    #[arg(long)]
    sidecar_port: u16,
    /// Reconcile cluster interval, default 20 [unit: seconds]
    #[arg(long, default_value = "20")]
    reconcile_interval: u64,
    /// The sidecar backend, when you use different operators, the backend may different.
    /// e.g:
    ///   "k8s,pod=xline-pod-1,container=xline,namespace=default" for k8s backend
    ///   "local" for local backend
    #[arg(long, value_parser = parse_backend)]
    backend: BackendConfig,
    /// The xline executable path, default to "xline"
    #[arg(long, default_value = "xline")]
    xline_executable: String,
    /// The xline storage engine, default to "rocksdb"
    #[arg(long, default_value = "rocksdb")]
    xline_storage_engine: String,
    /// The xline data directory, default to "/usr/local/xline/data-dir"
    #[arg(long, default_value_t = default_data_dir())]
    xline_data_dir: String,
    /// Set if this xline node is a leader node, default to false
    #[arg(long, default_value = "false")]
    xline_is_leader: bool,
    /// The xline additional parameter
    #[arg(long)]
    xline_additional: Option<String>,
    /// Enable backup, choose a storage type.
    /// e.g:
    ///    s3:bucket_name    for s3 (not available)
    ///    pv:/path/to/dir   for pv
    #[arg(long, value_parser = parse_backup_type)]
    backup: Option<BackupConfig>,
    /// Monitor(Operator) address, set to enable heartbeat and configuration discovery
    #[arg(long, alias = "operator-addr")]
    monitor_addr: Option<String>,
    /// Heartbeat interval, it is enabled if --monitor_addr is set.
    #[arg(long, alias = "operator_heartbeat_interval", default_value = "10")]
    heartbeat_interval: u64,
    /// Set registry to enable configuration discovery
    /// e.g:
    ///    sts:name:namespace           for k8s statefulset
    ///    http:register_server_addr    for http registry
    #[arg(long, value_parser = parse_registry)]
    registry: Option<RegistryConfig>,
}

impl From<Cli> for Config {
    fn from(value: Cli) -> Self {
        Self {
            name: value.name.clone(),
            cluster_name: value.cluster_name,
            init_member: MemberConfig {
                members: value.init_members,
                xline_port: value.xline_port,
                sidecar_port: value.sidecar_port,
            },
            reconcile_interval: Duration::from_secs(value.reconcile_interval),
            backend: value.backend,
            xline: XlineConfig {
                name: value.name, // xline server has a same name with sidecar
                executable: value.xline_executable,
                storage_engine: value.xline_storage_engine,
                data_dir: value.xline_data_dir,
                is_leader: value.xline_is_leader,
                additional: value.xline_additional,
            },
            backup: value.backup,
            monitor: value.monitor_addr.map(|addr| MonitorConfig {
                monitor_addr: addr,
                heartbeat_interval: Duration::from_secs(value.heartbeat_interval),
            }),
            registry: value.registry,
        }
    }
}

/// parse backup type
fn parse_backup_type(value: &str) -> Result<BackupConfig, String> {
    if value.is_empty() {
        return Err("backup type is empty".to_owned());
    }
    let mut items: Vec<_> = value.split([':', ' ', ',', '-']).collect();
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
            Ok(BackupConfig::S3 { bucket })
        }
        "pv" => {
            if items.len() != 1 {
                return Err(format!(
                    "pv backup type requires 1 argument, got {}",
                    items.len()
                ));
            }
            let path = items.remove(0).to_owned();
            Ok(BackupConfig::PV {
                path: PathBuf::from(path),
            })
        }
        _ => Err(format!("unknown backup type: {backup_type}")),
    }
}

/// Parse backend
fn parse_backend(value: &str) -> Result<BackendConfig, String> {
    if value.is_empty() {
        return Err("backend is empty".to_owned());
    }
    let mut items: Vec<_> = value.split(',').collect();
    let backend = items.remove(0);
    match backend {
        "k8s" => {
            let mut pod_name = String::new();
            let mut container_name = String::new();
            let mut namespace = String::new();
            while let Some(item) = items.pop() {
                let Some((k, v)) = item.split_once('=') else {
                    return Err(format!("k8s backend got unexpected argument {item}, expect <key>=<value>"));
                };
                match k {
                    "pod" => pod_name = v.to_owned(),
                    "container" => container_name = v.to_owned(),
                    "namespace" => namespace = v.to_owned(),
                    _ => return Err(format!("k8s backend got unexpected argument {item}, expect one of 'pod', 'container', 'namespace'")),
                }
            }
            if pod_name.is_empty() || container_name.is_empty() || namespace.is_empty() {
                return Err("k8s backend must set 'pod', 'container', 'namespace'".to_owned());
            }
            Ok(BackendConfig::K8s {
                pod_name,
                container_name,
                namespace,
            })
        }
        "local" => Ok(BackendConfig::Local),
        _ => Err(format!("unknown backend: {backend}")),
    }
}

/// parse members from string
/// # Errors
/// Return error when pass wrong args
fn parse_members(s: &str) -> Result<HashMap<String, String>, String> {
    let mut map = HashMap::new();
    for pair in s.split(',') {
        if let Some((id, addr)) = pair.split_once('=') {
            let _ignore = map.insert(id.to_owned(), addr.to_owned());
        } else {
            return Err(format!(
                "parse the pair '{pair}' error, expect '<id>=<addr>'",
            ));
        }
    }
    Ok(map)
}

/// parse registry from string
/// # Errors
/// Return error when pass wrong args
fn parse_registry(s: &str) -> Result<RegistryConfig, String> {
    if s.is_empty() {
        return Err("registry type is empty".to_owned());
    }
    let mut items: Vec<_> = s.split([':', ' ', ',', '-']).collect();
    let kind = items.remove(0);
    match kind {
        "sts" => {
            if items.len() != 2 {
                return Err(format!(
                    "sts registry type requires 2 argument, got {}",
                    items.len()
                ));
            }
            let name = items.remove(0).to_owned();
            let namespace = items.remove(0).to_owned();
            Ok(RegistryConfig::Sts { name, namespace })
        }
        "http" => {
            if items.len() != 1 {
                return Err(format!(
                    "http registry type requires 1 argument, got {}",
                    items.len()
                ));
            }
            let server_addr = items.remove(0).to_owned();
            Ok(RegistryConfig::Http { server_addr })
        }
        _ => Err(format!("unknown registry type: {kind}")),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    debug!("{:?}", cli);

    Sidecar::new(cli.into()).run().await
}

#[cfg(test)]
mod test {
    use crate::{parse_backend, parse_backup_type, parse_members, parse_registry, Cli};
    use clap::Parser;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use xline_sidecar::types::{BackendConfig, BackupConfig, RegistryConfig};

    fn full_parameter() -> Vec<&'static str> {
        vec![
            "sidecar_exe",
            "--name=node1",
            "--cluster-name=my-xline-cluster",

            "--init-members=node1=127.0.0.1,node2=127.0.0.2,node3=127.0.0.3",

            "--xline-port=2379",
            "--sidecar-port=2380",

            "--reconcile-interval=20",
            "--backend=k8s,pod=xline-pod-1,container=xline,namespace=default",

            "--xline-executable=/usr/local/bin/xline",
            "--xline-storage-engine=rocksdb",
            "--xline-data-dir=/usr/local/xline/data-dir",
            "--xline-is-leader",
            "--xline-additional='--auth-public-key /mnt/public.pem --auth-private-key /mnt/private.pem'",

            "--backup=s3:bucket_name",

            "--operator-addr=xline-operator.svc.default.cluster.local:8080",

            "--heartbeat-interval=10",

            "--registry=http:server_addr"
        ]
    }

    #[test]
    fn test_parse_backup_type() {
        let test_cases = [
            ("", Err("backup type is empty".to_owned())),
            (
                "s3:bucket_name",
                Ok(BackupConfig::S3 {
                    bucket: "bucket_name".to_owned(),
                }),
            ),
            (
                "s3:bucket:name",
                Err("s3 backup type requires 1 arguments, got 2".to_owned()),
            ),
            (
                "s3",
                Err("s3 backup type requires 1 arguments, got 0".to_owned()),
            ),
            (
                "pv:/home",
                Ok(BackupConfig::PV {
                    path: PathBuf::from("/home"),
                }),
            ),
            (
                "pv:/home:/paopao",
                Err("pv backup type requires 1 argument, got 2".to_owned()),
            ),
            (
                "pv",
                Err("pv backup type requires 1 argument, got 0".to_owned()),
            ),
            (
                "_invalid_",
                Err("unknown backup type: _invalid_".to_owned()),
            ),
        ];
        for (test_case, res) in test_cases {
            assert_eq!(parse_backup_type(test_case), res);
        }
    }

    #[test]
    fn test_parse_backend() {
        let test_cases = [
            (
                "k8s,pod=my-pod,container=my-container,namespace=my-namespace",
                Ok(BackendConfig::K8s {
                    pod_name: "my-pod".to_owned(),
                    container_name: "my-container".to_owned(),
                    namespace: "my-namespace".to_owned(),
                }),
            ),
            ("local", Ok(BackendConfig::Local)),
            ("", Err("backend is empty".to_owned())),
            (
                "k8s,pod=my-pod,invalid-arg,namespace=my-namespace",
                Err(
                    "k8s backend got unexpected argument invalid-arg, expect <key>=<value>"
                        .to_owned(),
                ),
            ),
            (
                "k8s,pod=my-pod,container=my-container",
                Err("k8s backend must set 'pod', 'container', 'namespace'".to_owned()),
            ),
            (
                "unknown-backend",
                Err("unknown backend: unknown-backend".to_owned()),
            ),
        ];
        for (input, expected) in test_cases {
            let result = parse_backend(input);
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_parse_members() {
        let test_cases = vec![
            (
                "id1=addr1,id2=addr2,id3=addr3",
                Ok([("id1", "addr1"), ("id2", "addr2"), ("id3", "addr3")]
                    .iter()
                    .map(|&(id, addr)| (id.to_owned(), addr.to_owned()))
                    .collect()),
            ),
            (
                "id1=addr1",
                Ok(std::iter::once(&("id1", "addr1"))
                    .map(|&(id, addr)| (id.to_owned(), addr.to_owned()))
                    .collect()),
            ),
            (
                "",
                Err("parse the pair '' error, expect '<id>=<addr>'".to_owned()),
            ),
            (
                "id1=addr1,id2",
                Err("parse the pair 'id2' error, expect '<id>=<addr>'".to_owned()),
            ),
            (
                "id1=addr1,id2=addr2,",
                Err("parse the pair '' error, expect '<id>=<addr>'".to_owned()),
            ),
        ];

        for (input, expected) in test_cases {
            let result = parse_members(input);
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_parse_registry() {
        let test_cases = [
            (
                "sts",
                Err("sts registry type requires 2 argument, got 0".to_owned()),
            ),
            (
                "http",
                Err("http registry type requires 1 argument, got 0".to_owned()),
            ),
            ("", Err("registry type is empty".to_owned())),
            (
                "sts:sts_name",
                Err("sts registry type requires 2 argument, got 1".to_owned()),
            ),
            (
                "sts:sts_name:sts_namespace",
                Ok(RegistryConfig::Sts {
                    name: "sts_name".to_owned(),
                    namespace: "sts_namespace".to_owned(),
                }),
            ),
            (
                "http:server_addr",
                Ok(RegistryConfig::Http {
                    server_addr: "server_addr".to_owned(),
                }),
            ),
            ("unknown", Err("unknown registry type: unknown".to_owned())),
        ];
        for (input, expected) in test_cases {
            let result = parse_registry(input);
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_parse_cli_should_success() {
        let cli = Cli::parse_from(full_parameter());
        assert_eq!(cli.name, "node1");
        assert_eq!(
            cli.init_members,
            HashMap::from([
                ("node1".to_owned(), "127.0.0.1".to_owned()),
                ("node2".to_owned(), "127.0.0.2".to_owned()),
                ("node3".to_owned(), "127.0.0.3".to_owned()),
            ])
        );
        assert_eq!(cli.xline_port, 2379);
        assert_eq!(cli.sidecar_port, 2380);
        assert_eq!(
            cli.monitor_addr.unwrap_or_default(),
            "xline-operator.svc.default.cluster.local:8080"
        );
        assert_eq!(cli.reconcile_interval, 20);
        assert_eq!(cli.heartbeat_interval, 10);
        assert_eq!(
            cli.backup,
            Some(BackupConfig::S3 {
                bucket: "bucket_name".to_owned(),
            })
        );
        assert_eq!(cli.xline_executable, "/usr/local/bin/xline");
        assert_eq!(cli.xline_storage_engine, "rocksdb");
        assert_eq!(cli.xline_data_dir, "/usr/local/xline/data-dir");
        assert!(cli.xline_is_leader);
        assert_eq!(
            cli.xline_additional,
            Some(
                "'--auth-public-key /mnt/public.pem --auth-private-key /mnt/private.pem'"
                    .to_owned()
            )
        );
        assert_eq!(
            cli.backend,
            BackendConfig::K8s {
                pod_name: "xline-pod-1".to_owned(),
                container_name: "xline".to_owned(),
                namespace: "default".to_owned(),
            }
        );
        assert_eq!(
            cli.registry,
            Some(RegistryConfig::Http {
                server_addr: "server_addr".to_owned()
            })
        );
    }
}
