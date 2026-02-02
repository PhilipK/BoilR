use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use sysinfo::{Pid, System};

use crate::config::get_config_folder;

/// Returns the path to the lock file
fn get_lock_file_path() -> PathBuf {
    get_config_folder().join("boilr.lock")
}

/// Represents a lock on the application instance
pub struct InstanceLock {
    _file: File,
    path: PathBuf,
}

impl InstanceLock {
    /// Attempts to acquire an exclusive lock for this application instance.
    /// Returns Ok(InstanceLock) if successful, or Err with a message if another instance is running.
    pub fn acquire() -> Result<Self, String> {
        let lock_path = get_lock_file_path();

        // Ensure the config folder exists
        if let Some(parent) = lock_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Check if lock file exists and contains a valid PID
        if lock_path.exists() {
            if let Ok(mut file) = File::open(&lock_path) {
                let mut contents = String::new();
                if file.read_to_string(&mut contents).is_ok() {
                    if let Ok(pid) = contents.trim().parse::<usize>() {
                        // Check if process with that PID is still running
                        if is_process_running(pid) {
                            return Err(format!(
                                "Another instance of BoilR is already running (PID: {})",
                                pid
                            ));
                        }
                    }
                }
            }
        }

        // Try to create/overwrite the lock file
        match OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                let pid = std::process::id();
                if let Err(e) = write!(file, "{}", pid) {
                    return Err(format!("Failed to write lock file: {}", e));
                }

                Ok(InstanceLock {
                    _file: file,
                    path: lock_path,
                })
            }
            Err(e) => Err(format!("Failed to create lock file: {}", e)),
        }
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Check if a process with the given PID is running using sysinfo
fn is_process_running(pid: usize) -> bool {
    let mut system = System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    system.process(Pid::from(pid)).is_some()
}
