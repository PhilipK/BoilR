//! Backups of Steam's shortcut files, and handing BoilR shortcuts over to the user.

use std::path::{Path, PathBuf};

use boilr::backups::{backup_shortcuts, load_backups, restore_backup};
use boilr::platforms::{get_platforms, platform_sections};
use boilr_core::{
    config::get_backups_folder,
    settings::{save_settings, Settings},
    steam::{get_shortcuts_for_user, get_shortcuts_paths},
    sync::{disconnect_shortcut, IsBoilRShortcut},
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BackupEntry {
    pub path: String,
    /// Steam account the backup belongs to.
    pub user_id: String,
    /// UTC time the backup was taken, as "YYYY-MM-DD HH:MM:SS".
    pub taken_at: String,
}

/// Backups are named `<user id>-<YYYY-MM-DD-HH-MM-SS>-shortcuts.vdf`.
fn parse_backup_name(path: &Path) -> Option<BackupEntry> {
    let name = path.file_name()?.to_str()?.strip_suffix("-shortcuts.vdf")?;
    let (user_id, date) = name.split_once('-')?;
    let parts: Vec<&str> = date.split('-').collect();
    if user_id.is_empty() || !user_id.chars().all(|c| c.is_ascii_digit()) || parts.len() != 6 {
        return None;
    }
    let [y, mo, d, h, mi, s] = [parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]];
    Some(BackupEntry {
        path: path.to_string_lossy().to_string(),
        user_id: user_id.to_string(),
        taken_at: format!("{y}-{mo}-{d} {h}:{mi}:{s}"),
    })
}

/// Only files directly inside BoilR's backup folder may be restored.
fn is_in_folder(path: &Path, folder: &Path) -> bool {
    match (path.canonicalize(), folder.canonicalize()) {
        (Ok(path), Ok(folder)) => path.parent() == Some(folder.as_path()),
        _ => false,
    }
}

fn settings() -> Result<Settings, String> {
    Settings::new().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_backups() -> Vec<BackupEntry> {
    load_backups()
        .iter()
        .filter_map(|p| parse_backup_name(p))
        .collect()
}

#[tauri::command]
pub fn create_backup() -> Result<Vec<BackupEntry>, String> {
    backup_shortcuts(&settings()?.steam);
    Ok(list_backups())
}

/// Restores a backup, backing up the current shortcuts first so the restore can be undone.
#[tauri::command]
pub fn restore_shortcuts(path: String) -> Result<Vec<BackupEntry>, String> {
    let path = PathBuf::from(path);
    if !is_in_folder(&path, &get_backups_folder()) {
        return Err("That file is not one of BoilR's backups.".to_string());
    }
    let steam = settings()?.steam;
    backup_shortcuts(&steam);
    if !restore_backup(&steam, &path) {
        return Err("No Steam account matches this backup.".to_string());
    }
    Ok(list_backups())
}

#[derive(Debug, Clone, Serialize)]
pub struct ManagedShortcut {
    pub app_id: u32,
    pub name: String,
}

/// Shortcuts BoilR currently manages (updates, and removes when the game is gone).
#[tauri::command(async)]
pub fn list_boilr_shortcuts() -> Result<Vec<ManagedShortcut>, String> {
    let users = get_shortcuts_paths(&settings()?.steam).map_err(|err| err.to_string())?;
    let mut result: Vec<ManagedShortcut> = users
        .iter()
        .filter_map(|user| get_shortcuts_for_user(user).ok())
        .flat_map(|info| info.shortcuts)
        .filter(|s| s.is_boilr_shortcut())
        .map(|s| ManagedShortcut {
            app_id: s.app_id,
            name: s.app_name,
        })
        .collect();
    result.sort_by_key(|s| s.name.to_lowercase());
    result.dedup_by_key(|s| s.app_id);
    Ok(result)
}

/// Hands a shortcut over to the user: BoilR stops updating or removing it, and won't re-add it.
#[tauri::command]
pub fn release_shortcut(app_id: u32) -> Result<Settings, String> {
    let mut settings = settings()?;
    disconnect_shortcut(&settings, app_id)?;
    if !settings.blacklisted_games.contains(&app_id) {
        settings.blacklisted_games.push(app_id);
    }
    save_settings(&settings, &platform_sections(&get_platforms()))
        .map_err(|err| err.to_string())?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_backup_names() {
        let entry = parse_backup_name(Path::new("/b/12345678-2026-09-27-06-58-12-shortcuts.vdf"));
        assert_eq!(
            entry,
            Some(BackupEntry {
                path: "/b/12345678-2026-09-27-06-58-12-shortcuts.vdf".to_string(),
                user_id: "12345678".to_string(),
                taken_at: "2026-09-27 06:58:12".to_string(),
            })
        );
    }

    #[test]
    fn ignores_other_files() {
        assert_eq!(parse_backup_name(Path::new("/b/notes.vdf")), None);
        assert_eq!(
            parse_backup_name(Path::new("/b/abc-2026-09-27-06-58-12-shortcuts.vdf")),
            None
        );
        assert_eq!(
            parse_backup_name(Path::new("/b/1-2026-09-27-shortcuts.vdf")),
            None
        );
    }

    #[test]
    fn only_restores_from_the_backup_folder() {
        let dir = std::env::temp_dir().join(format!("boilr-backup-test-{}", std::process::id()));
        let inside = dir.join("backup");
        std::fs::create_dir_all(&inside).expect("create folder");
        let good = inside.join("1-2026-09-27-06-58-12-shortcuts.vdf");
        let outside = dir.join("1-2026-09-27-06-58-12-shortcuts.vdf");
        std::fs::write(&good, b"x").expect("write");
        std::fs::write(&outside, b"x").expect("write");

        assert!(is_in_folder(&good, &inside));
        assert!(!is_in_folder(&outside, &inside));
        assert!(!is_in_folder(
            &inside.join("../1-2026-09-27-06-58-12-shortcuts.vdf"),
            &inside
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
