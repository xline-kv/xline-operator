use std::fs::{copy, metadata, read_dir, remove_file};
use std::ops::Div;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::io;
use tracing::debug;

use crate::snapshot::{Metadata, Provider, SNAPSHOT_SUFFIX};

/// The persistent volume snapshot
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

    async fn save<T: Send + Sync + AsRef<Path>>(&self, src: T, metadata: &Metadata) -> Result<()> {
        let src = src.as_ref();
        if !src.is_file() {
            return Err(anyhow!("source path is not a file"));
        }
        let filename = metadata.to_string();
        let dst = Path::new(&self.backup_path).join(&filename);
        let size = copy(src, dst)?;
        debug!(
            "backup snapshot file: {filename}, size: {} MB",
            size.div(0x0010_0000)
        );
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

#[cfg(test)]
mod test {
    use crate::snapshot::pv::Pv;
    use crate::snapshot::{Metadata, Provider};
    use std::fs::remove_file;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_pv() {
        let pv = Pv {
            backup_path: ".".parse().unwrap(),
        };
        let metadata1 = Metadata {
            name: "xline-cluster-0".to_owned(),
            revision: 10,
        };
        let metadata2 = Metadata {
            name: "xline-cluster-0".to_owned(),
            revision: 8,
        };
        let metadata3 = Metadata {
            name: "xline-cluster-1".to_owned(),
            revision: 8,
        };
        let _r = remove_file("xline-cluster-0.10.xline.backup");
        let _r = remove_file("xline-cluster-1.8.xline.backup");
        let _r = remove_file("xline-cluster-0.8.xline.backup");
        assert!(pv.latest().await.unwrap().is_none());
        pv.save("./src/lib.rs", &metadata1).await.unwrap();
        pv.save("./src/lib.rs", &metadata2).await.unwrap();
        pv.save("./src/lib.rs", &metadata3).await.unwrap();
        let latest = pv.latest().await.unwrap().unwrap();
        assert_eq!(
            latest,
            Metadata {
                name: "xline-cluster-0".to_owned(),
                revision: 10,
            }
        );
        assert_eq!(
            pv.load(&metadata3)
                .await
                .unwrap()
                .to_string_lossy()
                .to_string(),
            "./xline-cluster-1.8.xline.backup"
        );
        sleep(Duration::from_secs(3)).await;
        pv.purge(Duration::from_secs(1)).await.unwrap();
        assert!(pv.latest().await.unwrap().is_none());
    }
}
