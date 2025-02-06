use serde::Serialize;

use crate::platforms::{load_settings, FromSettingsString, GamesPlatform};
use serde::Deserialize;
use std::io::Error;
use std::path::{Path, PathBuf};
use std::process::Command;
use steam_shortcuts_util::Shortcut;

use crate::platforms::ShortcutToImport;

#[derive(Clone, Deserialize, Default)]
pub struct GamePassPlatForm {
    settings: GamePassSettings,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct GamePassSettings {
    enabled: bool,
}

impl GamesPlatform for GamePassPlatForm {
    fn name(&self) -> &str {
        "Game Pass"
    }

    fn code_name(&self) -> &str {
        "gamepass"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<crate::platforms::ShortcutToImport>> {
        let command = include_str!("./game_pass_games.ps1");
        let res = run_powershell_command(command)?;
        let apps: Vec<AppInfo> = serde_json::from_str(&res)?;
        let windows_dir = std::env::var("WinDir").unwrap_or("C:\\Windows".to_string());
        let explorer = Path::new(&windows_dir)
            .join("explorer.exe")
            .to_string_lossy()
            .to_string();

        let name_getters: [fn(&AppInfo) -> eyre::Result<String>; 3] =
            [get_name_from_game, get_name_from_config, get_name_from_xml];

        let games_iter = apps
            .iter()
            .filter(|app| {
                !(app.display_name.contains("DisplayName")
                    || app.display_name.contains("ms-resource"))
            })
            .filter_map(|game| {
                let launch_url = format!("shell:AppsFolder\\{}", game.aum_id());
                name_getters
                    .iter()
                    .find_map(|&f| f(game).ok())
                    .map(|game_name| {
                        let shortcut = Shortcut::new(
                            "0",
                            &game_name,
                            &explorer,
                            &windows_dir,
                            "",
                            "",
                            &launch_url,
                        );
                        ShortcutToImport {
                            shortcut: shortcut.to_owned(),
                            needs_proton: false,
                            needs_symlinks: false,
                        }
                    })
            });

        Ok(games_iter.collect())
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Game Pass");
        ui.checkbox(&mut self.settings.enabled, "Import from Game Pass");
    }
}

fn get_name_from_xml(app_info: &AppInfo) -> eyre::Result<String> {
    use roxmltree::Document;
    let path_to_config = app_info.appx_manifest();
    let xml = std::fs::read_to_string(path_to_config)?;
    let doc = Document::parse(&xml)?;
    doc.descendants()
        .find(|n| n.has_tag_name("uap::VisualElements"))
        .and_then(|n| n.attribute("DisplayName"))
        .map(|n| n.to_string())
        .ok_or(eyre::format_err!("Name not found"))
}

fn get_name_from_game(app_info: &AppInfo) -> eyre::Result<String> {
    if !app_info.kind.is_game() {
        Err(eyre::format_err!("Not a game type"))
    } else {
        Ok(app_info.display_name.to_owned())
    }
}

fn get_name_from_config(app_info: &AppInfo) -> eyre::Result<String> {
    use roxmltree::Document;
    let path_to_config = app_info.microsoft_game_path();
    let xml = std::fs::read_to_string(path_to_config)?;
    let doc = Document::parse(&xml)?;
    doc.descendants()
        .find(|n| n.has_tag_name("ShellVisuals"))
        .and_then(|n| n.attribute("DefaultDisplayName"))
        .map(|n| n.to_string())
        .ok_or(eyre::format_err!("Name not found"))
}

#[derive(Deserialize, Debug)]
struct AppInfo {
    kind: Kind,
    display_name: String,
    install_location: String,
    family_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Kind {
    Null,
    Str(String),
    Array(Vec<String>),
}

impl AppInfo {
    fn aum_id(&self) -> String {
        format!("{}!{}", self.family_name, self.kind.as_ref())
    }

    fn microsoft_game_path(&self) -> PathBuf {
        Path::new(&self.install_location).join("MicrosoftGame.config")
    }

    fn appx_manifest(&self) -> PathBuf {
        Path::new(&self.install_location).join("AppxManifest.xml")
    }
}

impl AsRef<str> for Kind {
    fn as_ref(&self) -> &str {
        match self {
            Kind::Null => "",
            Kind::Str(s) => s.as_ref(),
            Kind::Array(array) => array.iter().next().map(|s| s.as_ref()).unwrap_or(""),
        }
    }
}

impl Kind {
    fn is_game(&self) -> bool {
        match self {
            Kind::Str(string) => string.eq("Game"),
            Kind::Array(strings) => strings.iter().any(|s| s == "Game"),
            _ => false,
        }
    }
}

fn run_powershell_command(cmd: &str) -> Result<String, Error> {
    let output = Command::new("powershell").arg("/c").arg(cmd).output()?;

    match output.status.success() {
        true => Ok(String::from_utf8_lossy(&output.stdout).to_string()),
        false => Err(Error::new(
            std::io::ErrorKind::Other,
            String::from_utf8_lossy(&output.stderr).to_string(),
        )),
    }
}

impl FromSettingsString for GamePassPlatForm {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        GamePassPlatForm {
            settings: load_settings(s),
        }
    }
}
