#![allow(dead_code)] // TODO remove when it is implemented

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Sidecar operator config
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Config {
    /// Name of this node
    pub name: String,
    /// Operators
    pub members: HashMap<String, String>,
    /// Check cluster health interval
    pub check_interval: Duration,
    /// Backup storage config
    pub backup: Option<Backup>,
}

/// Backup storage config
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Backup {
    /// S3 storage
    S3 {
        /// S3 path
        path: String,
        /// S3 secret
        secret: String,
    },
    /// PV storage
    PV {
        /// Mounted path of pv
        path: PathBuf,
    },
}

impl Config {
    /// Constructor
    #[must_use]
    #[inline]
    pub fn new(
        name: String,
        members: HashMap<String, String>,
        check_interval: Duration,
        backup: Option<Backup>,
    ) -> Self {
        Self {
            name,
            members,
            check_interval,
            backup,
        }
    }
}
