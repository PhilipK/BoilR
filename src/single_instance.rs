use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use crate::config::get_config_folder;

/// Returns the path to the lock file
fn get_lock_file_path() -> PathBuf {
    get_config_folder().join("boilr.lock")
}

/// Represents a lock on the application instance.
///
/// Uses an OS file lock rather than a PID written to the file: the OS releases the lock
/// when the process exits, even after a crash or kill, so a leftover lock file never blocks
/// startup. A stored PID could not do this inside Flatpak, where BoilR usually runs as
/// PID 2 and would always find "itself" running.
pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    /// Attempts to acquire an exclusive lock for this application instance.
    /// Returns Ok(InstanceLock) if successful, or Err with a message if another instance is running.
    pub fn acquire() -> Result<Self, String> {
        acquire_at(&get_lock_file_path())
    }
}

fn acquire_at(lock_path: &Path) -> Result<InstanceLock, String> {
    if let Some(parent) = lock_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|e| format!("Failed to open lock file: {}", e))?;

    match file.try_lock() {
        Ok(()) => Ok(InstanceLock { _file: file }),
        Err(TryLockError::WouldBlock) => {
            Err("Another instance of BoilR is already running".to_string())
        }
        Err(TryLockError::Error(e)) if e.kind() == std::io::ErrorKind::Unsupported => {
            // A filesystem without lock support should not stop BoilR from starting.
            eprintln!("Could not lock {}: {}", lock_path.display(), e);
            Ok(InstanceLock { _file: file })
        }
        Err(TryLockError::Error(e)) => {
            Err(format!("Failed to lock {}: {}", lock_path.display(), e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_lock_path(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("boilr-lock-test-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("boilr.lock")
    }

    #[test]
    fn second_lock_is_refused_while_first_is_held() {
        let path = temp_lock_path("held");
        let first = acquire_at(&path);
        assert!(first.is_ok());
        assert!(acquire_at(&path).is_err());
    }

    #[test]
    fn lock_is_available_again_after_release() {
        let path = temp_lock_path("released");
        drop(acquire_at(&path));
        assert!(acquire_at(&path).is_ok());
    }

    #[test]
    fn leftover_lock_file_does_not_block_startup() {
        // A lock file left behind by a killed 1.10.0 instance contains a PID.
        let path = temp_lock_path("leftover");
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")));
        let _ = std::fs::write(&path, "2");
        assert!(acquire_at(&path).is_ok());
    }
}
