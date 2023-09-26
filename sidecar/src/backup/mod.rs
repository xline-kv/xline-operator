/// The persistent volume backup
pub(crate) mod pv;
/// The s3 backup
pub(crate) mod s3;

use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tonic::Streaming;
use xlineapi::SnapshotResponse;

/// Snapshot file suffix
const SNAPSHOT_SUFFIX: &str = "xline.backup";

/// Snapshot metadata
#[derive(Debug, PartialEq)]
pub(crate) struct Metadata {
    /// The name of this backup
    pub(crate) name: String,
    /// The revision of this backup
    pub(crate) revision: i64,
}

impl ToString for Metadata {
    fn to_string(&self) -> String {
        format!("{}.{}.{SNAPSHOT_SUFFIX}", self.name, self.revision)
    }
}

impl TryFrom<&Path> for Metadata {
    type Error = anyhow::Error;

    fn try_from(value: &Path) -> std::result::Result<Self, Self::Error> {
        let filename = value
            .file_name()
            .ok_or(anyhow!("backup file name not found, got {value:?}"))?
            .to_str()
            .ok_or(anyhow!("the backup path is not a valid unicode"))?;
        let mut split = filename.trim_end_matches(SNAPSHOT_SUFFIX).split('.');
        if let (Some(name), Some(revision)) = (split.next(), split.next()) {
            let revision: i64 = revision.parse()?;
            return Ok(Metadata {
                name: name.to_owned(),
                revision,
            });
        };
        Err(anyhow!(
            "invalid file name: {filename}, expect <name>.<revision>"
        ))
    }
}

/// Snapshot provider
#[async_trait]
pub(crate) trait Provider: Debug + Send + Sync + 'static {
    /// Get the latest backup metadata
    async fn latest(&self) -> Result<Option<Metadata>>;

    /// Save the backup at path src to this provider
    async fn save(&self, src: Streaming<SnapshotResponse>, metadata: &Metadata) -> Result<()>;

    /// Load a backup and generate a path to store
    async fn load(&self, metadata: &Metadata) -> Result<PathBuf>;

    /// Purge snapshots that exceed the TTL.
    async fn purge(&self, ttl: Duration) -> Result<()>;
}

#[cfg(test)]
mod test {
    use crate::backup::Metadata;
    use std::path::Path;

    #[test]
    fn test_metadata() {
        let metadata = Metadata {
            name: "xline-cluster-0".to_owned(),
            revision: 1,
        };
        let filename: String = metadata.to_string();
        let expect = "xline-cluster-0.1.xline.backup";
        assert_eq!(filename, expect);
        let metadata = Metadata::try_from(Path::new(expect)).unwrap();
        assert_eq!(
            &metadata,
            &Metadata {
                name: "xline-cluster-0".to_owned(),
                revision: 1,
            }
        );
    }
}
