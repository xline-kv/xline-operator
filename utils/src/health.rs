use crate::consts::{DEFAULT_BACKUP_DIR, DEFAULT_DATA_DIR};
use std::fs::{read_to_string, remove_file, write};
use std::path::Path;

/// The text to test the volume is working fine, just for testing purposes :)
const TEST_TEXT: &str = "If I become a cat, I won't have to work anymore :)";
/// The test file name
const TEST_FILENAME: &str = "working_mans_dream";

/// Check if the volume under the path is working fine
fn check_volume(path: &Path) -> bool {
    let filename = format!("{TEST_FILENAME}_{}", uuid::Uuid::new_v4());
    let path = path.join(filename);
    if write(&path, TEST_TEXT).is_err() {
        return false;
    }
    let content = read_to_string(&path);
    if remove_file(&path).is_err() {
        return false;
    }
    match content {
        Ok(content) => {
            if content != TEST_TEXT {
                return false;
            }
        }
        Err(_) => return false,
    }
    true
}

/// Check if the data volume is working fine
#[inline]
#[must_use]
pub fn check_data_volume() -> bool {
    check_volume(Path::new(DEFAULT_DATA_DIR))
}

/// Check if the backup volume is working fine
#[inline]
#[must_use]
pub fn check_backup_volume() -> bool {
    let backup_volume = Path::new(DEFAULT_BACKUP_DIR);
    if !backup_volume.exists() {
        // If the backup volume path does not exist, it means that there are no available backup options.
        return true;
    }
    check_volume(backup_volume)
}

#[cfg(test)]
mod test {
    use crate::health::check_volume;
    use std::path::Path;

    #[test]
    fn check_volume_return_ok() {
        assert!(check_volume(Path::new(".")));
    }
}
