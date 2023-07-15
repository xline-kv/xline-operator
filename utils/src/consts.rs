/// Default backup PV mount path in container, this path cannot be mounted by user
pub const DEFAULT_BACKUP_DIR: &str = "/xline-backup";
/// Default xline data dir, this path cannot be mounted by user
pub const DEFAULT_DATA_DIR: &str = "/usr/local/xline/data-dir";

/// Meta table name
pub const META_TABLE: &str = "meta";
/// KV table name
pub const KV_TABLE: &str = "kv";
/// Lease table name
pub const LEASE_TABLE: &str = "lease";
/// User table
pub const USER_TABLE: &str = "user";
/// Role table
pub const ROLE_TABLE: &str = "role";
/// Auth table
pub const AUTH_TABLE: &str = "auth";

/// They are copied from xline because the sidecar operator want to handle the storage engine directly
pub const XLINE_TABLES: [&str; 6] = [
    META_TABLE,
    KV_TABLE,
    LEASE_TABLE,
    AUTH_TABLE,
    USER_TABLE,
    ROLE_TABLE,
];
