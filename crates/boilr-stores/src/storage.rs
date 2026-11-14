//! Sessions and libraries live in `<config folder>/stores/<store key>/`. Session files hold
//! tokens and cookies and libraries list what the user owns, so both are created readable by
//! the user only (mode 0600 on Unix; on Windows the config folder is under the user's own
//! `%APPDATA%`). Lock files and the record of the last failed refresh live next to the store
//! folders, in `.locks` and `.status`, so they outlive a sign-out's cleanup of the folder.

use std::{
    fs::{self, TryLockError},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use serde::{de::DeserializeOwned, Serialize};

use crate::{Failure, OwnedLibrary, Session, StoreId};

fn store_folder(store: StoreId) -> PathBuf {
    stores_root().join(store.key())
}

#[cfg(not(test))]
fn stores_root() -> PathBuf {
    boilr_core::config::get_config_folder().join("stores")
}

/// Tests never touch the user's real accounts.
#[cfg(test)]
pub(crate) fn stores_root() -> PathBuf {
    std::env::temp_dir().join(format!("boilr-stores-test-{}", std::process::id()))
}

/// Holds an OS file lock on one store's files until dropped. The classic app, the Tauri app
/// and `boilr --no-ui` may run at once; every change to a store's session or library is made
/// under this lock. It is only held for file work, never during a network request, so a
/// wait for it is short; a wait longer than [`FILE_LOCK_WAIT`] gives up instead of hanging.
/// The lock file lives outside the store's folder, so signing out can remove that folder
/// while another process waits for the lock.
pub(crate) fn lock_account(store: StoreId) -> eyre::Result<fs::File> {
    let started = Instant::now();
    loop {
        if let Some(file) = try_lock(&format!("{}.lock", store.key()))? {
            return Ok(file);
        }
        if started.elapsed() >= FILE_LOCK_WAIT {
            return Err(eyre::format_err!(
                "Another BoilR has been using the saved {} account for a while; try again in a moment",
                store.display_name()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// How long [`lock_account`] waits for another process's file work.
const FILE_LOCK_WAIT: Duration = Duration::from_secs(20);

/// Holds the OS lock that lets one process at a time refresh a store's library, network
/// requests included: two refreshes at once would both use, and rotate, the same refresh
/// token. Waits up to `wait` for another process's refresh, then returns `None`.
pub(crate) async fn lock_refresh(store: StoreId, wait: Duration) -> eyre::Result<Option<fs::File>> {
    let started = Instant::now();
    loop {
        if let Some(file) = try_lock(&format!("{}.refresh.lock", store.key()))? {
            return Ok(Some(file));
        }
        if started.elapsed() >= wait {
            return Ok(None);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Tries to take the OS lock on a file in the locks folder. `None` when another process (or
/// another handle in this one) holds it. A file system without lock support does not stop
/// BoilR: the file is returned unlocked.
fn try_lock(file_name: &str) -> eyre::Result<Option<fs::File>> {
    let folder = stores_root().join(".locks");
    fs::create_dir_all(&folder)?;
    let path = folder.join(file_name);
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(err)) if err.kind() == std::io::ErrorKind::Unsupported => {
            eprintln!("Could not lock {}: {err}", path.display());
            Ok(Some(file))
        }
        Err(TryLockError::Error(err)) => Err(err.into()),
    }
}

fn session_file(store: StoreId) -> PathBuf {
    store_folder(store).join("session.json")
}

fn library_file(store: StoreId) -> PathBuf {
    store_folder(store).join("library.json")
}

pub fn load_session(store: StoreId) -> eyre::Result<Option<Session>> {
    read_json(&session_file(store))
}

pub fn save_session(store: StoreId, session: &Session) -> eyre::Result<()> {
    write_json(&session_file(store), session)
}

pub fn load_library(store: StoreId) -> eyre::Result<Option<OwnedLibrary>> {
    read_json(&library_file(store))
}

pub fn save_library(store: StoreId, library: &OwnedLibrary) -> eyre::Result<()> {
    write_json(&library_file(store), library)
}

/// Removes the cached library and keeps the session.
pub(crate) fn delete_library(store: StoreId) -> eyre::Result<()> {
    remove_file(&library_file(store))
}

fn failure_file(store: StoreId) -> PathBuf {
    stores_root()
        .join(".status")
        .join(format!("{}.json", store.key()))
}

/// The last failed refresh, when none has succeeded since. An unreadable record counts as
/// none.
pub(crate) fn load_failure(store: StoreId) -> Option<Failure> {
    read_json(&failure_file(store)).ok().flatten()
}

/// Records a failed refresh, or with `None` that the last one worked.
pub(crate) fn save_failure(store: StoreId, failure: Option<&Failure>) -> eyre::Result<()> {
    match failure {
        Some(failure) => write_json(&failure_file(store), failure),
        None => remove_file(&failure_file(store)),
    }
}

fn remove_file(path: &Path) -> eyre::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// Signs out: removes the session and the cached library.
pub fn delete_account(store: StoreId) -> eyre::Result<()> {
    match fs::remove_dir_all(store_folder(store)) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> eyre::Result<Option<T>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// Writes to a temporary file and renames it over the target, so a crash never leaves half a
/// file. Every write has its own temporary file, so two writers never mix their bytes; the
/// last rename wins.
fn write_json<T: Serialize>(path: &Path, value: &T) -> eyre::Result<()> {
    let folder = path
        .parent()
        .ok_or_else(|| eyre::format_err!("{} has no parent folder", path.display()))?;
    fs::create_dir_all(folder)?;
    let tmp = path.with_extension(format!("json.{:016x}.tmp", rand::random::<u64>()));
    let bytes = serde_json::to_vec_pretty(value)?;
    let written = (|| -> std::io::Result<()> {
        let mut file = open_new(&tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    Ok(written?)
}

/// A new file only its owner can read.
#[cfg(unix)]
fn open_new(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_new(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

/// Tests that use the test folder, the pending logins or the failure records hold this, so
/// they run one at a time.
#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OwnedGame;

    #[test]
    fn round_trips_session_and_library() -> eyre::Result<()> {
        let _alone = test_lock();
        assert!(load_session(StoreId::Gog)?.is_none());
        let session = Session {
            account_name: Some("player".into()),
            data: serde_json::json!({"refresh_token": "abc"}),
        };
        save_session(StoreId::Gog, &session)?;
        let loaded = load_session(StoreId::Gog)?;
        assert_eq!(
            loaded.and_then(|s| s.account_name),
            Some("player".to_string())
        );

        let library = OwnedLibrary {
            account_name: None,
            fetched_at: 1,
            games: vec![OwnedGame::new(StoreId::Gog, "1207658924", "Unreal Gold")],
            partial: false,
        };
        save_library(StoreId::Gog, &library)?;
        assert_eq!(load_library(StoreId::Gog)?.map(|l| l.games.len()), Some(1));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for file in [session_file(StoreId::Gog), library_file(StoreId::Gog)] {
                let mode = fs::metadata(file)?.permissions().mode();
                assert_eq!(mode & 0o777, 0o600);
            }
        }

        // Writers at the same time take the account lock, as every caller does: each write
        // succeeds and none mixes its bytes with another's.
        let written: Vec<eyre::Result<()>> = std::thread::scope(|scope| {
            let writers: Vec<_> = (0..8)
                .map(|count| {
                    let library = OwnedLibrary {
                        fetched_at: count,
                        games: vec![OwnedGame::new(StoreId::Gog, "1", "Game"); 200],
                        ..library.clone()
                    };
                    scope.spawn(move || {
                        let _lock = lock_account(StoreId::Gog)?;
                        save_library(StoreId::Gog, &library)
                    })
                })
                .collect();
            writers
                .into_iter()
                .map(|writer| {
                    writer
                        .join()
                        .unwrap_or_else(|_| Err(eyre::format_err!("a writer panicked")))
                })
                .collect()
        });
        for result in written {
            result?;
        }
        assert_eq!(
            load_library(StoreId::Gog)?.map(|l| l.games.len()),
            Some(200)
        );
        let leftovers = fs::read_dir(store_folder(StoreId::Gog))?
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0);

        delete_account(StoreId::Gog)?;
        assert!(load_session(StoreId::Gog)?.is_none());
        Ok(())
    }
}
