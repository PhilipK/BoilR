use crate::platforms::{
    load_settings, to_shortcuts_simple, FromSettingsString, GamesPlatform, NeedsProton,
    ShortcutToImport,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

use super::game::RockstarGame;
use super::RockstarSettings;

#[derive(Clone)]
pub struct RockstarPlatform {
    pub settings: RockstarSettings,
}

impl NeedsProton<RockstarPlatform> for RockstarGame {
    #[cfg(target_os = "windows")]
    fn needs_proton(&self, _platform: &RockstarPlatform) -> bool {
        false
    }

    #[cfg(target_family = "unix")]
    fn needs_proton(&self, _platform: &RockstarPlatform) -> bool {
        true
    }

    fn create_symlinks(&self, _platform: &RockstarPlatform) -> bool {
        false
    }
}

impl RockstarPlatform {
    fn get_shortcuts(&self) -> eyre::Result<Vec<RockstarGame>> {
        let mut games = vec![];
        for root in get_search_roots() {
            games.extend(discover_rockstar_game_executables(&root));
        }

        if games.is_empty() {
            return Ok(vec![RockstarGame {
                name: "Rockstar Games Launcher".to_string(),
                icon: String::new(),
                executable: get_launcher_path().ok_or_else(|| {
                    eyre::eyre!("Could not find Rockstar Games Launcher or installed games")
                })?,
            }]);
        }

        let mut seen = std::collections::HashSet::new();
        let mut unique_games = vec![];

        for path in games {
            let normalized = normalize_game_path(&path);
            if seen.insert(normalized.clone()) {
                unique_games.push(RockstarGame {
                    name: infer_game_name(&path),
                    icon: String::new(),
                    executable: path,
                });
            }
        }

        Ok(unique_games)
    }
}

fn get_search_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from(r"C:\Program Files\Rockstar Games"),
        PathBuf::from(r"C:\Program Files (x86)\Rockstar Games"),
    ];

    if let Ok(profile) = std::env::var("USERPROFILE") {
        roots.push(PathBuf::from(profile).join("AppData").join("Local").join("Rockstar Games"));
    }

    roots
}

fn discover_rockstar_game_executables(root: &Path) -> Vec<PathBuf> {
    let mut found = vec![];
    if !root.exists() {
        return found;
    }

    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if should_skip_rockstar_directory(&path) {
                        continue;
                    }
                    stack.push(path);
                } else if is_game_executable(&path) && !is_rockstar_launcher(&path) {
                    found.push(path);
                }
            }
        }
    }

    found.sort();
    found.dedup();
    found
}

fn should_skip_rockstar_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("Launcher") || name.eq_ignore_ascii_case("Social Club") || name.eq_ignore_ascii_case("redistributables"))
}

fn is_game_executable(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
        && !is_uninstall_executable(path)
}

fn is_uninstall_executable(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("uninstall.exe")
                || name.eq_ignore_ascii_case("unins000.exe")
                || name.contains("uninstall")
                || name.contains("unins")
        })
}

fn is_rockstar_launcher(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("Launcher.exe")
                || name.eq_ignore_ascii_case("Rockstar Games Launcher.exe")
                || name.eq_ignore_ascii_case("LauncherPatcher.exe")
        })
}

fn infer_game_name(path: &Path) -> String {
    path.parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .map(|name| name.replace('_', " "))
        .unwrap_or_else(|| "Rockstar game".to_string())
}

fn normalize_game_path(path: &Path) -> String {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf()).to_string_lossy().to_lowercase()
}

fn find_launcher_in_directories(root_dirs: &[PathBuf], candidate_names: &[&str]) -> Option<PathBuf> {
    root_dirs.iter().find_map(|root| {
        candidate_names.iter().find_map(|name| {
            let candidate = root.join(name);
            candidate.exists().then_some(candidate)
        })
    })
}

#[cfg(target_os = "windows")]
fn get_launcher_path() -> Option<PathBuf> {
    let roots = [
        PathBuf::from(r"C:\Program Files\Rockstar Games\Launcher"),
        PathBuf::from(r"C:\Program Files (x86)\Rockstar Games\Launcher"),
    ];
    find_launcher_in_directories(&roots, &["Launcher.exe", "Rockstar Games Launcher.exe"])
}

#[cfg(target_family = "unix")]
fn get_launcher_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let compat_root = Path::new(&home)
        .join(".steam")
        .join("steam")
        .join("steamapps")
        .join("compatdata");

    let mut launchers = vec![];
    if let Ok(entries) = std::fs::read_dir(&compat_root) {
        for entry in entries.flatten() {
            let prefix = entry.path();
            launchers.push(prefix.join("pfx").join("drive_c").join("Program Files").join("Rockstar Games").join("Launcher"));
            launchers.push(prefix.join("pfx").join("drive_c").join("Program Files (x86)").join("Rockstar Games").join("Launcher"));
        }
    }

    find_launcher_in_directories(&launchers, &["Launcher.exe", "Rockstar Games Launcher.exe"])
}

impl GamesPlatform for RockstarPlatform {
    fn name(&self) -> &str {
        "Rockstar Launcher"
    }

    fn code_name(&self) -> &str {
        "rockstar"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts_simple(self.get_shortcuts())
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Rockstar Launcher");
        ui.checkbox(&mut self.settings.enabled, "Import from Rockstar Launcher");
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }
}

impl FromSettingsString for RockstarPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        RockstarPlatform {
            settings: load_settings(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{discover_rockstar_game_executables, find_launcher_in_directories};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn discovers_rockstar_games_from_install_folders() {
        let unique = format!(
            "boilr-rockstar-games-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        let game_dir = root.join("Games").join("GTA V");
        let expected = game_dir.join("GTA5.exe");
        fs::create_dir_all(&game_dir).unwrap();
        fs::write(&expected, b"").unwrap();

        let found = discover_rockstar_game_executables(&root);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].as_path(), expected.as_path());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn finds_rockstar_launcher_in_known_location() {
        let unique = format!(
            "boilr-rockstar-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        let launcher_dir = root.join("Program Files").join("Rockstar Games").join("Launcher");
        let expected = launcher_dir.join("Launcher.exe");
        fs::create_dir_all(&launcher_dir).unwrap();
        fs::write(&expected, b"").unwrap();

        let found = find_launcher_in_directories(&[launcher_dir.clone()], &["Launcher.exe"]);

        assert_eq!(found.as_deref(), Some(expected.as_path()));

        let _ = fs::remove_dir_all(root);
    }
}
