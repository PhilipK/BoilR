use crate::platforms::{
    load_settings, to_shortcuts_simple, FromSettingsString, GamesPlatform, NeedsProton,
    ShortcutToImport,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

use super::game::CustomGame;
use super::CustomSettings;

#[derive(Clone)]
pub struct CustomPlatform {
    pub settings: CustomSettings,
}

impl NeedsProton<CustomPlatform> for CustomGame {
    fn needs_proton(&self, _platform: &CustomPlatform) -> bool {
        false
    }

    fn create_symlinks(&self, _platform: &CustomPlatform) -> bool {
        false
    }
}

impl CustomPlatform {
    fn get_shortcuts(&self) -> eyre::Result<Vec<CustomGame>> {
        let mut games = vec![];
        for folder in self.settings.effective_folders() {
            let folder = folder.trim();
            if folder.is_empty() {
                continue;
            }

            let root = Path::new(folder);
            games.extend(discover_custom_game_executables(root));
        }

        games.sort_by(|a, b| a.executable.cmp(&b.executable));
        Ok(games)
    }
}

fn discover_custom_game_executables(root: &Path) -> Vec<CustomGame> {
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
                    stack.push(path);
                } else if is_game_executable(&path) {
                    found.push(CustomGame {
                        name: infer_game_name(&path),
                        icon: String::new(),
                        executable: path,
                    });
                }
            }
        }
    }

    let mut preferred: std::collections::BTreeMap<PathBuf, CustomGame> = std::collections::BTreeMap::new();
    for candidate in found {
        let group_key = group_key_for_candidate(root, &candidate.executable);
        let candidate_depth = candidate.executable.components().count();
        match preferred.get(&group_key) {
            Some(existing) if candidate_depth < existing.executable.components().count() => {
                preferred.insert(group_key, candidate);
            }
            None => {
                preferred.insert(group_key, candidate);
            }
            _ => {}
        }
    }

    let mut result: Vec<_> = preferred.into_values().collect();
    result.sort_by(|a, b| a.executable.cmp(&b.executable));
    result
}

fn group_key_for_candidate(root: &Path, path: &Path) -> PathBuf {
    let mut current = path.parent().unwrap_or(path);
    while let Some(parent) = current.parent() {
        if parent == root {
            return current.to_path_buf();
        }
        if is_non_game_folder_name(current.file_name().and_then(|name| name.to_str())) {
            current = parent;
            continue;
        }
        return current.to_path_buf();
    }
    root.to_path_buf()
}

fn is_non_game_folder_name(name: Option<&str>) -> bool {
    matches!(
        name,
        Some(
            "Binaries"
            | "Binary"
            | "Bin"
            | "Win64"
            | "Win32"
            | "x64"
            | "x86"
            | "Data"
            | "Engine"
            | "Plugins"
            | "Launcher"
        )
    )
}

fn infer_game_name(path: &Path) -> String {
    path.parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .map(|name| name.replace('_', " "))
        .unwrap_or_else(|| path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("Custom game").to_string())
}

fn is_game_executable(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
        && !is_uninstall_executable(path)
        && !is_runtime_executable(path)
}

fn is_runtime_executable(path: &Path) -> bool {
    let path_components = path.ancestors().skip(1).filter_map(|ancestor| ancestor.file_name()).filter_map(|name| name.to_str());
    if path_components.clone().any(|component| is_non_game_folder_name(Some(component))) {
        return true;
    }

    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            let lower = name.to_ascii_lowercase();
            lower.contains("shipping") || lower.contains("launcher")
        })
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

impl GamesPlatform for CustomPlatform {
    fn name(&self) -> &str {
        "Custom"
    }

    fn code_name(&self) -> &str {
        "custom"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts_simple(self.get_shortcuts())
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Custom folders");
        ui.checkbox(&mut self.settings.enabled, "Import from custom folders");
        ui.horizontal(|ui| {
            ui.label("Folders containing game executables");
            if ui.button("+").on_hover_text("Add another custom folder").clicked() {
                self.settings.folders.push(String::new());
            }
        });

        if self.settings.folders.is_empty() && !self.settings.folder.trim().is_empty() {
            self.settings.folders.push(self.settings.folder.clone());
            self.settings.folder.clear();
        }

        if self.settings.folders.is_empty() {
            self.settings.folders.push(String::new());
        }

        let mut remove_folder = None;
        for (index, folder) in self.settings.folders.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(folder);
                if ui.button("🗑").on_hover_text("Remove this folder").clicked() {
                    remove_folder = Some(index);
                }
            });
        }

        if let Some(index) = remove_folder {
            if self.settings.folders.len() > 1 {
                self.settings.folders.remove(index);
            } else {
                self.settings.folders[0].clear();
            }
        }
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }
}

impl FromSettingsString for CustomPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        CustomPlatform {
            settings: load_settings(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::discover_custom_game_executables;
    use std::{fs, time::{SystemTime, UNIX_EPOCH}};

    #[test]
    fn discovers_games_from_a_custom_folder() {
        let unique = format!(
            "boilr-custom-games-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        let game_dir = root.join("MyGame");
        let expected = game_dir.join("game.exe");
        fs::create_dir_all(&game_dir).unwrap();
        fs::write(&expected, b"").unwrap();

        let found = discover_custom_game_executables(&root);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].executable, expected);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prefers_the_shallowest_game_executable_per_folder() {
        let unique = format!(
            "boilr-custom-games-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        let game_dir = root.join("Stray");
        let shallow_executable = game_dir.join("Stray.exe");
        let nested_executable = game_dir.join("Binaries/Win64/Stray-Win64-Shipping.exe");
        fs::create_dir_all(nested_executable.parent().unwrap()).unwrap();
        fs::write(&shallow_executable, b"").unwrap();
        fs::write(&nested_executable, b"").unwrap();

        let found = discover_custom_game_executables(&root);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].executable, shallow_executable);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn skips_runtime_binaries_in_win64_folders() {
        let unique = format!(
            "boilr-custom-games-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        let runtime_executable = root.join("Stray/Hk_project/Binaries/Win64/Stray-Win64-Shipping.exe");
        fs::create_dir_all(runtime_executable.parent().unwrap()).unwrap();
        fs::write(&runtime_executable, b"").unwrap();

        let found = discover_custom_game_executables(&root);

        assert!(found.is_empty());

        let _ = fs::remove_dir_all(root);
    }
}
