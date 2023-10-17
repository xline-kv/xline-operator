use std::fs::{metadata, read_dir, remove_file};
use std::ops::{AddAssign, Div};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::StreamExt;
use tokio::fs::File;
use tokio::io;
use tokio::io::AsyncWriteExt;
use tonic::Streaming;
use tracing::debug;
use xline_client::types::maintenance::SnapshotResponse;

use crate::backup::{Metadata, Provider, SNAPSHOT_SUFFIX};

/// The persistent volume backup
#[derive(Debug)]
pub(crate) struct Pv {
    /// The backup path
    pub(crate) backup_path: PathBuf,
}

#[async_trait]
impl Provider for Pv {
    async fn latest(&self) -> Result<Option<Metadata>> {
        let entries = read_dir(&self.backup_path)?;
        let snapshot = entries
            .filter_map(|item| {
                item.ok()
                    .and_then(|entry| entry.file_name().into_string().ok())
            })
            .filter(|item| item.ends_with(SNAPSHOT_SUFFIX))
            .map(|item| Metadata::try_from(Path::new(&item)))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .max_by_key(|metadata| metadata.revision);
        Ok(snapshot)
    }

    async fn save(&self, mut src: Streaming<SnapshotResponse>, metadata: &Metadata) -> Result<()> {
        let filename = metadata.to_string();
        let mut dst = File::create(Path::new(&self.backup_path).join(&filename)).await?;
        let Some(item) = src.next().await else {
            return Err(anyhow!("get the item from source failed"))
        };
        let mut item = item?;
        let mut size = item.blob.len();
        let mut buf = item.blob;
        while item.remaining_bytes > 0 {
            let Some(got) = src.next().await else {
                return Err(anyhow!("get the item from source failed"))
            };
            item = got?;
            size.add_assign(item.blob.len());
            buf.extend(item.blob);
        }
        debug!(
            "backup snapshot file: {filename}, size: {} KB",
            size.div(1024)
        );
        dst.write_all(&buf).await?;
        Ok(())
    }

    async fn load(&self, metadata: &Metadata) -> Result<PathBuf> {
        let filename = metadata.to_string();
        let path = Path::new(&self.backup_path).join(filename);
        // just return this path, the path is reachable by operator
        Ok(path)
    }

    async fn purge(&self, ttl: Duration) -> Result<()> {
        let entries = read_dir(&self.backup_path)?;
        let snapshot = entries
            .filter_map(|item| item.ok().map(|entry| entry.path()))
            .filter(|item| item.to_string_lossy().ends_with(SNAPSHOT_SUFFIX))
            .map(|snap| {
                let metadata = metadata(&snap)?;
                Result::<_, io::Error>::Ok((metadata, snap))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (metadata, path) in snapshot {
            let time = metadata.modified()?;
            let duration = SystemTime::now().duration_since(time)?;
            if duration > ttl {
                remove_file(&path)?;
            }
        }
        Ok(())
    }
}
