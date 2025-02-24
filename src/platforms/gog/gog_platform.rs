use std::path::{Path, PathBuf};

use crate::platforms::{
    load_settings, to_shortcuts, FromSettingsString, GamesPlatform, NeedsProton, ShortcutToImport,
};

use super::{
    gog_config::GogConfig,
    gog_game::{GogGame, GogShortcut},
    GogSettings,
};

#[derive(Clone)]
pub struct GogPlatform {
    pub settings: GogSettings,
}

impl GogPlatform {
    fn get_shortcuts(&self) -> eyre::Result<Vec<GogShortcut>> {
        let gog_location = self
            .settings
            .location
            .as_ref()
            .map(|location| Path::new(&location).to_path_buf())
            .unwrap_or_else(default_location);
        if !gog_location.exists() {
            return Err(eyre::format_err!("Could not find path: {:?}", gog_location));
        }
        let config_path = gog_location.join("config.json");
        if !config_path.exists() {
            return Err(eyre::format_err!(
                "Config file not found: {:?}",
                config_path
            ));
        }
        get_shortcuts_from_config(self.settings.wine_c_drive.clone(), config_path)
    }
}

fn get_shortcuts_from_config(
    _wine_c_drive: Option<String>,
    config_path: PathBuf,
) -> eyre::Result<Vec<GogShortcut>> {
    let install_locations = get_install_locations(config_path)?;
    #[cfg(target_family = "unix")]
    let install_locations = if let Some(wine_c_drive) = &_wine_c_drive {
        fix_paths(wine_c_drive, install_locations)
    } else {
        install_locations
    };
    let mut game_folders = vec![];
    for install_location in install_locations {
        let path = Path::new(&install_location);
        if path.exists() {
            let dirs = path.read_dir();
            if let Ok(dirs) = dirs {
                for dir in dirs.flatten() {
                    if let Ok(file_type) = dir.file_type() {
                        if file_type.is_dir() {
                            game_folders.push(dir.path());
                        }
                    }
                }
            }
        }
    }
    let shortcuts = get_gog_shortcuts_from_game_folders(game_folders);
    Ok(shortcuts)
}

pub fn get_gog_shortcuts_from_game_folders(game_folders: Vec<PathBuf>) -> Vec<GogShortcut> {
    let games = get_games_from_game_folders(game_folders);

    get_shortcuts_from_games(games)
}

fn get_shortcuts_from_games(games: Vec<(GogGame, PathBuf)>) -> Vec<GogShortcut> {
    let mut shortcuts = vec![];
    for (game, game_folder) in games {
        if let Some(folder_path) = game_folder.to_str() {
            if let Some(tasks) = &game.play_tasks {
                if let Some(primary_task) = tasks.iter().find(|t| {
                    t.is_primary.unwrap_or_default()
                        && t.task_type == "FileTask"
                        && (t.category.as_ref().unwrap_or(&String::from("")) == "launcher"
                            || t.category.as_ref().unwrap_or(&String::from("")) == "game")
                }) {
                    if let Some(task_path) = &primary_task.path {
                        let full_path = game_folder.join(task_path);
                        if let Some(full_path) = full_path.to_str() {
                            let folder_path = folder_path.to_string();

                            let working_dir = match &primary_task.working_dir {
                                Some(working_dir) => game_folder
                                    .join(working_dir)
                                    .to_str()
                                    .unwrap_or(folder_path.as_str())
                                    .to_string(),
                                None => folder_path.to_string(),
                            };

                            #[cfg(target_family = "unix")]
                            let working_dir = working_dir.replace('\\', "/");

                            let full_path_string = full_path.to_string();

                            #[cfg(target_family = "unix")]
                            let full_path_string = full_path_string.replace('\\', "/");
                            let arguments = primary_task
                                .arguments
                                .as_ref()
                                .unwrap_or(&"".to_string())
                                .clone();
                            let shortcut = GogShortcut {
                                name: game.name,
                                game_folder: folder_path,
                                working_dir,
                                game_id: game.game_id,
                                path: full_path_string,
                                arguments,
                            };
                            shortcuts.push(shortcut);
                        }
                    }
                }
            }
        }
    }
    shortcuts
}

fn get_games_from_game_folders(game_folders: Vec<PathBuf>) -> Vec<(GogGame, PathBuf)> {
    let mut games = vec![];
    for game_folder in &game_folders {
        let mut game_folder = game_folder;
        let deep_game_folder = Path::new(&game_folder).join("game").to_path_buf();
        if deep_game_folder.exists() {
            game_folder = &deep_game_folder;
        }
        if let Ok(files) = game_folder.read_dir() {
            for file in files.flatten() {
                if let Some(file_name) = file.file_name().to_str() {
                    if file_name.starts_with("goggame-") {
                        if let Some(extension) = file.path().extension() {
                            if let Some(extension) = extension.to_str() {
                                if extension == "info" {
                                    // Finally we know we can parse this as a game
                                    if let Ok(content) = std::fs::read_to_string(file.path()) {
                                        if let Ok(gog_game) =
                                            serde_json::from_str::<GogGame>(&content)
                                        {
                                            games.push((gog_game, game_folder.clone()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    games
}

impl NeedsProton<GogPlatform> for GogShortcut {
    #[cfg(target_family = "unix")]
    fn needs_proton(&self, _platform: &GogPlatform) -> bool {
        true
    }

    #[cfg(not(target_family = "unix"))]
    fn needs_proton(&self, _platform: &GogPlatform) -> bool {
        false
    }

    #[cfg(not(target_family = "unix"))]
    fn create_symlinks(&self, _platform: &GogPlatform) -> bool {
        false
    }

    #[cfg(target_family = "unix")]
    fn create_symlinks(&self, platform: &GogPlatform) -> bool {
        platform.settings.create_symlinks
    }
}

#[cfg(target_family = "unix")]
fn fix_paths(wine_c_drive: &str, paths: Vec<String>) -> Vec<String> {
    paths
        .iter()
        .flat_map(|path| {
            if let Some(stripped) = path.strip_prefix("C:\\") {
                let path_buf = Path::new(wine_c_drive).join(stripped);
                path_buf.to_str().map(|s| s.to_string().replace('\\', "/"))
            } else {
                None
            }
        })
        .collect()
}

fn get_install_locations(path: PathBuf) -> eyre::Result<Vec<String>> {
    let data_res = std::fs::read_to_string(path)?;
    let config: GogConfig = serde_json::from_str(&data_res)?;
    let path_vec = match config.library_path {
        Some(path) => vec![path],
        None => vec![],
    };
    Ok(config.installation_paths.unwrap_or(path_vec))
}

pub fn default_location() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let key = "PROGRAMDATA";
        let program_data = std::env::var(key).unwrap_or_default();
        Path::new(&program_data).join("GOG.com").join("Galaxy")
    }
    #[cfg(target_family = "unix")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        Path::new(&home).join("Games/gog-galaxy/drive_c/ProgramData/GOG.com/Galaxy")
    }
}

impl FromSettingsString for GogPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        GogPlatform {
            settings: load_settings(s),
        }
    }
}

impl GamesPlatform for GogPlatform {
    fn name(&self) -> &str {
        "GOG"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts(self, self.get_shortcuts())
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("GoG Galaxy");
        ui.checkbox(&mut self.settings.enabled, "Import from GoG Galaxy");
        if self.settings.enabled {
            ui.horizontal(|ui| {
                let mut empty_string = "".to_string();
                let gog_location = self.settings.location.as_mut().unwrap_or(&mut empty_string);
                ui.label("GoG Galaxy Folder: ");
                if ui.text_edit_singleline(gog_location).changed() {
                    self.settings.location = Some(gog_location.to_string());
                }
            });
        }
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn code_name(&self) -> &str {
        "gog"
    }
}
