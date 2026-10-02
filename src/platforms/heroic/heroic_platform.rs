use serde::Deserialize;

use super::{HeroicGame, HeroicGameType, HeroicSettings};
use eyre::WrapErr;

use crate::platforms::{load_settings, FromSettingsString, GamesPlatform};
use crate::platforms::{to_shortcuts, NeedsProton, ShortcutToImport};
use std::collections::HashMap;
use std::path::Path;

use std::path::PathBuf;

#[derive(Clone)]
pub struct HeroicPlatform {
    pub settings: HeroicSettings,
    #[cfg_attr(not(feature = "egui-ui"), allow(dead_code))]
    pub(crate) heroic_games: Option<Vec<HeroicGameType>>,
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum InstallationMode {
    FlatPak,
    UserBin,
}

#[derive(Deserialize)]
struct HeroicGogConfig {
    installed: Vec<HeroicGogPath>,
}

#[derive(Deserialize)]
struct HeroicGogPath {
    platform: String,
    #[serde(alias = "appName")]
    app_name: String,
    install_path: String,
}

#[derive(Deserialize, Debug, Clone)]
struct NileInstalledEntry {
    #[serde(alias = "appName", alias = "app_name")]
    id: String,
    #[serde(alias = "install_path", alias = "installPath")]
    path: String,
    #[serde(default)]
    #[allow(dead_code)]
    version: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
enum NileInstalledConfig {
    List(Vec<NileInstalledEntry>),
    Wrapped { installed: Vec<NileInstalledEntry> },
}

#[derive(Deserialize, Debug, Clone)]
struct NileProduct {
    id: String,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct NileLibraryEntry {
    product: NileProduct,
}

fn get_nile_config_location(install_mode: &InstallationMode, file_name: &str) -> PathBuf {
    let home_dir = std::env::var("HOME").unwrap_or_default();
    let config_dir = match install_mode {
        InstallationMode::FlatPak => {
            ".var/app/com.heroicgameslauncher.hgl/config/heroic/nile_config/nile"
        }
        InstallationMode::UserBin => ".config/heroic/nile_config/nile",
    };
    Path::new(&home_dir).join(config_dir).join(file_name)
}

fn parse_nile_games(
    installed_content: &str,
    library_content: Option<&str>,
    install_mode: InstallationMode,
) -> eyre::Result<Vec<HeroicGameType>> {
    let entries = match serde_json::from_str::<NileInstalledConfig>(installed_content)? {
        NileInstalledConfig::List(list) => list,
        NileInstalledConfig::Wrapped { installed } => installed,
    };

    // Titles are cosmetic: a library.json that does not parse falls back to folder names.

    let mut title_map = HashMap::new();
    if let Some(lib_content) = library_content {
        if let Ok(lib_entries) = serde_json::from_str::<Vec<NileLibraryEntry>>(lib_content) {
            for entry in lib_entries {
                if let Some(title) = entry.product.title {
                    title_map.insert(entry.product.id, title);
                }
            }
        }
    }

    let mut games = vec![];
    for entry in entries {
        let path = Path::new(&entry.path);
        let title = title_map
            .get(&entry.id)
            .cloned()
            .or_else(|| path.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| entry.id.clone());

        games.push(HeroicGameType::Heroic {
            title,
            app_name: entry.id,
            install_mode,
        });
    }

    Ok(games)
}

fn get_nile_games(install_modes: &[InstallationMode]) -> eyre::Result<Vec<HeroicGameType>> {
    let mut nile_shortcuts = vec![];

    for install_mode in install_modes {
        let installed_file = get_nile_config_location(install_mode, "installed.json");
        // No Nile config for this install mode: nothing is installed through it.
        if !installed_file.exists() {
            continue;
        }
        // A file that exists but cannot be read or parsed is an error, so Amazon
        // games do not silently vanish if Nile changes its format.
        let installed_content = std::fs::read_to_string(&installed_file)
            .wrap_err_with(|| format!("Could not read {}", installed_file.display()))?;

        let library_file = get_nile_config_location(install_mode, "library.json");
        let library_content = std::fs::read_to_string(&library_file).ok();

        let games = parse_nile_games(
            &installed_content,
            library_content.as_deref(),
            *install_mode,
        )
        .wrap_err_with(|| format!("Could not parse {}", installed_file.display()))?;

        nile_shortcuts.extend(games);
    }

    // dedup_by only drops adjacent duplicates (Flatpak and native Heroic both installed).
    nile_shortcuts.sort_by(|a, b| a.app_name().cmp(b.app_name()));
    nile_shortcuts.dedup_by(|a, b| a.app_name() == b.app_name());

    Ok(nile_shortcuts)
}

fn get_installed_json_location(install_mode: &InstallationMode) -> PathBuf {
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "".to_string());
    match install_mode {
        InstallationMode::FlatPak => Path::new(&home_dir)
            .join(".var/app/com.heroicgameslauncher.hgl/config/heroic/legendaryConfig/legendary/installed.json"),
        InstallationMode::UserBin => Path::new(&home_dir).join(".config/heroic/legendaryConfig/legendary/installed.json"),
    }
}

fn get_gog_installed_location(install_mode: &InstallationMode) -> PathBuf {
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "".to_string());
    match install_mode {
        InstallationMode::FlatPak => Path::new(&home_dir)
            .join(".var/app/com.heroicgameslauncher.hgl/config/heroic/gog_store/installed.json"),
        InstallationMode::UserBin => {
            Path::new(&home_dir).join(".config/heroic/gog_store/installed.json")
        }
    }
}

fn get_shortcuts_from_install_mode(
    install_mode: &InstallationMode,
) -> eyre::Result<Vec<HeroicGame>> {
    let installed_path = get_installed_json_location(install_mode);
    get_shortcuts_from_location(installed_path)
}

fn get_shortcuts_from_location<P: AsRef<Path>>(path: P) -> eyre::Result<Vec<HeroicGame>> {
    let installed_json_path = path.as_ref();
    if installed_json_path.exists() {
        let json = std::fs::read_to_string(installed_json_path)?;
        parse_installed_games(&json)
    } else {
        Ok(vec![])
    }
}

/// Legendary lists installed DLC next to the games in `installed.json`. A DLC can't be
/// launched on its own, so it must not become a shortcut (#557).
fn parse_installed_games(json: &str) -> eyre::Result<Vec<HeroicGame>> {
    let games_map = serde_json::from_str::<HashMap<String, HeroicGame>>(json)?;
    Ok(games_map
        .into_values()
        .filter(|game| !game.is_dlc)
        .collect())
}

impl HeroicPlatform {
    pub fn get_heroic_games(&self) -> eyre::Result<Vec<HeroicGameType>> {
        let install_modes = vec![InstallationMode::FlatPak, InstallationMode::UserBin];

        let mut heroic_games = self.get_epic_games(&install_modes)?;
        let gog_games = get_gog_games(&self.settings, &install_modes)?;
        heroic_games.extend(gog_games);
        let nile_games = get_nile_games(&install_modes)?;
        heroic_games.extend(nile_games);
        Ok(heroic_games)
    }
}

impl NeedsProton<HeroicPlatform> for HeroicGameType {
    #[cfg(not(target_family = "unix"))]
    fn needs_proton(&self, _platform: &HeroicPlatform) -> bool {
        false
    }

    #[cfg(target_family = "unix")]
    fn needs_proton(&self, _platform: &HeroicPlatform) -> bool {
        match self {
            HeroicGameType::Epic(_game) => true,
            HeroicGameType::Gog(_, is_windows) => *is_windows,
            HeroicGameType::Heroic { .. } => false,
        }
    }

    fn create_symlinks(&self, _platform: &HeroicPlatform) -> bool {
        false
    }
}

impl HeroicPlatform {
    pub fn get_epic_games(
        &self,
        install_modes: &[InstallationMode],
    ) -> eyre::Result<Vec<HeroicGameType>> {
        let mut shortcuts = vec![];
        for install_mode in install_modes {
            if let Ok(mut games) = get_shortcuts_from_install_mode(install_mode) {
                games.sort_by_key(|m| {
                    format!("{}-{}-{}", m.launch_parameters, m.executable, m.app_name)
                });
                games.dedup_by_key(|m| {
                    format!("{}-{}-{}", m.launch_parameters, m.executable, m.app_name)
                });

                for game in games {
                    if self.settings.is_heroic_launch(&game.app_name) {
                        shortcuts.push(HeroicGameType::Heroic {
                            title: game.title,
                            app_name: game.app_name,
                            install_mode: *install_mode,
                        });
                    } else if game.is_installed() {
                        shortcuts.push(HeroicGameType::Epic(game));
                    }
                }
            }
        }
        Ok(shortcuts)
    }
}
fn get_gog_games(
    settings: &HeroicSettings,
    install_modes: &[InstallationMode],
) -> eyre::Result<Vec<HeroicGameType>> {
    let mut gog_paths = vec![];
    for install_mode in install_modes {
        let config = get_gog_installed_location(install_mode);
        if config.exists() {
            if let Ok(config_string) = std::fs::read_to_string(config) {
                if let Ok(config) = serde_json::from_str::<HeroicGogConfig>(&config_string) {
                    for c in config.installed {
                        gog_paths.push((install_mode, c));
                    }
                }
            }
        }
    }

    let mut is_windows_map = HashMap::new();

    for (_, path) in gog_paths.iter() {
        is_windows_map.insert(path.app_name.clone(), path.platform == "windows");
    }

    let mut gog_shortcuts = vec![];

    let heroic_games = gog_paths
        .iter()
        .filter(|(_, p)| settings.is_heroic_launch(&p.app_name))
        .filter_map(|(install_mode, p)| {
            let path = Path::new(&p.install_path);
            if path.exists() {
                let title = path.file_name();
                Some(HeroicGameType::Heroic {
                    title: title.unwrap_or_default().to_string_lossy().to_string(),
                    app_name: p.app_name.clone(),
                    install_mode: **install_mode,
                })
            } else {
                None
            }
        });
    gog_shortcuts.extend(heroic_games);

    let game_folders = gog_paths
        .iter()
        .filter(|(_, p)| !settings.is_heroic_launch(&p.app_name))
        .filter_map(|(_, p)| {
            let path = Path::new(&p.install_path);
            if path.exists() {
                Some(path.to_path_buf())
            } else {
                None
            }
        })
        .collect();
    let direct_shortcuts = crate::platforms::get_gog_shortcuts_from_game_folders(game_folders);
    for shortcut in direct_shortcuts {
        let is_windows = is_windows_map.get(&shortcut.game_id).unwrap_or(&false);
        gog_shortcuts.push(HeroicGameType::Gog(shortcut, *is_windows));
    }

    Ok(gog_shortcuts)
}

impl FromSettingsString for HeroicPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        HeroicPlatform {
            heroic_games: None,
            settings: load_settings(s),
        }
    }
}

impl GamesPlatform for HeroicPlatform {
    fn name(&self) -> &str {
        "Heroic"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts(self, self.get_heroic_games())
    }

    #[cfg(feature = "egui-ui")]
    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Heroic");
        ui.checkbox(&mut self.settings.enabled, "Import from Heroic");
        ui.checkbox(
            &mut self.settings.default_launch_through_heroic,
            "Always launch games through Heroic",
        );
        let safe_mode_header = match (
            self.settings.default_launch_through_heroic,
            self.settings.launch_games_through_heroic.len(),
        ) {
            (false, 0) => "Force games to launch through Heroic Launcher".to_string(),
            (false, 1) => "One game forced to launch through Heroic Launcher".to_string(),
            (false, x) => format!("{x} games forced to launch through Heroic Launcher"),

            (true, 0) => "Force games to launch directly".to_string(),
            (true, 1) => "One game forced to launch directly".to_string(),
            (true, x) => format!("{x} games forced to launch directly"),
        };

        egui::CollapsingHeader::new(safe_mode_header).id_salt("Heroic_Launcher_safe_launch").show(ui, |ui| {
            if self.settings.default_launch_through_heroic{
                ui.label("Some games work best when launched directly, select those games below and BoilR will create shortcuts that launch the games directly.");
            } else {
                ui.label("Some games must be started from the Heroic Launcher, select those games below and BoilR will create shortcuts that opens the games through the Heroic Launcher.");
            }

            let manifests = self.heroic_games.get_or_insert_with(|| {
                let heroic_setting = self.settings.clone();
                let heroic_platform = HeroicPlatform{ settings:heroic_setting, heroic_games:None};
                heroic_platform.get_heroic_games().unwrap_or_default()
            });
            let safe_open_games = &mut self.settings.launch_games_through_heroic;

            for manifest in manifests{
                let key = manifest.app_name();
                let display_name = manifest.title();
                let mut safe_open = safe_open_games.contains(&display_name.to_string()) || safe_open_games.contains(&key.to_string());
                if ui.checkbox(&mut safe_open, display_name).clicked(){
                    if safe_open{
                        safe_open_games.push(key.to_string());
                    }else{
                        safe_open_games.retain(|m| m!= display_name && m!= key);
                    }
                }
            }
        });
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn code_name(&self) -> &str {
        "heroic"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use steam_shortcuts_util::shortcut::ShortcutOwned;

    #[test]
    fn test_parse_installed_games_skips_dlc() -> eyre::Result<()> {
        let json = r#"{
            "Eider": {
                "app_name": "Eider",
                "title": "HITMAN World of Assassination",
                "is_dlc": false,
                "install_path": "/home/deck/Games/Heroic/HITMAN3",
                "executable": "Launcher.exe",
                "launch_parameters": ""
            },
            "0a73eaedcac84bd28b567dbec764c5cb": {
                "app_name": "0a73eaedcac84bd28b567dbec764c5cb",
                "title": "HITMAN 3 - Seven Deadly Sins Collection",
                "is_dlc": true,
                "install_path": "/home/deck/Games/Heroic/HITMAN3",
                "executable": "",
                "launch_parameters": ""
            }
        }"#;

        let games = parse_installed_games(json)?;
        let [game] = games.as_slice() else {
            eyre::bail!("expected exactly one game, got {}", games.len());
        };
        assert_eq!(game.app_name, "Eider");
        Ok(())
    }

    #[test]
    fn test_parse_nile_games_list_format() -> eyre::Result<()> {
        let json = r#"[
            {
                "id": "amzn1.adg.product.test1",
                "path": "/home/deck/Games/nile/Fallout New Vegas",
                "version": "1.0"
            }
        ]"#;

        let games = parse_nile_games(json, None, InstallationMode::FlatPak)?;
        let [game] = games.as_slice() else {
            eyre::bail!("expected exactly one game, got {}", games.len());
        };
        assert_eq!(game.app_name(), "amzn1.adg.product.test1");
        assert_eq!(game.title(), "Fallout New Vegas");
        Ok(())
    }

    #[test]
    fn test_parse_nile_games_wrapped_format() -> eyre::Result<()> {
        let json = r#"{
            "installed": [
                {
                    "id": "amzn1.adg.product.test2",
                    "path": "/home/deck/Games/nile/Morrowind",
                    "version": "1.2"
                }
            ]
        }"#;

        let games = parse_nile_games(json, None, InstallationMode::UserBin)?;
        let [game] = games.as_slice() else {
            eyre::bail!("expected exactly one game, got {}", games.len());
        };
        assert_eq!(game.app_name(), "amzn1.adg.product.test2");
        assert_eq!(game.title(), "Morrowind");
        Ok(())
    }

    #[test]
    fn test_parse_nile_games_with_library_title() -> eyre::Result<()> {
        let installed_json = r#"[
            {
                "id": "amzn1.adg.product.test3",
                "path": "/home/deck/Games/nile/FNV",
                "version": "1.0"
            }
        ]"#;

        let library_json = r#"[
            {
                "product": {
                    "id": "amzn1.adg.product.test3",
                    "title": "Fallout: New Vegas Ultimate Edition"
                }
            }
        ]"#;

        let games = parse_nile_games(
            installed_json,
            Some(library_json),
            InstallationMode::FlatPak,
        )?;
        let [game] = games.as_slice() else {
            eyre::bail!("expected exactly one game, got {}", games.len());
        };
        assert_eq!(game.app_name(), "amzn1.adg.product.test3");
        assert_eq!(game.title(), "Fallout: New Vegas Ultimate Edition");
        Ok(())
    }

    #[test]
    fn test_parse_nile_games_rejects_unparseable_installed_json() {
        assert!(parse_nile_games("{ not json", None, InstallationMode::FlatPak).is_err());
    }

    #[test]
    fn test_parse_nile_games_falls_back_to_folder_name_on_bad_library() -> eyre::Result<()> {
        let installed_json = r#"[{"id": "amzn1.adg.product.test4", "path": "/games/Morrowind"}]"#;
        let games = parse_nile_games(installed_json, Some("not json"), InstallationMode::FlatPak)?;
        let [game] = games.as_slice() else {
            eyre::bail!("expected exactly one game, got {}", games.len());
        };
        assert_eq!(game.title(), "Morrowind");
        Ok(())
    }

    #[test]
    fn test_nile_shortcut_owned_conversion() {
        let game = HeroicGameType::Heroic {
            title: "Fallout: New Vegas".to_string(),
            app_name: "amzn1.adg.product.test1".to_string(),
            install_mode: InstallationMode::FlatPak,
        };

        let shortcut: ShortcutOwned = game.into();
        assert_eq!(shortcut.app_name, "Fallout: New Vegas");
        assert_eq!(shortcut.exe, "flatpak");
        assert!(shortcut
            .launch_options
            .contains("heroic://launch/amzn1.adg.product.test1"));
        assert!(shortcut.tags.contains(&"Heroic".to_string()));
        assert!(shortcut.tags.contains(&"Ready TO Play".to_string()));
        assert!(shortcut.tags.contains(&"Installed".to_string()));
    }
}
