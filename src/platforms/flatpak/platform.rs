use serde::{Deserialize, Serialize};

use crate::platforms::{
    load_settings, to_shortcuts, FromSettingsString, GamesPlatform, NeedsProton, ShortcutToImport,
};

use super::FlatpakSettings;
use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FlatpakPlatform {
    pub settings: FlatpakSettings,
}

#[derive(Debug, Clone)]
pub struct FlatpakApp {
    pub name: String,
    pub id: String,
}

impl From<FlatpakApp> for ShortcutOwned {
    fn from(app: FlatpakApp) -> Self {
        let launch_parameter = format!("run {}", app.id);
        Shortcut::new("0", &app.name, "flatpak", "", "", "", &launch_parameter).to_owned()
    }
}

impl NeedsProton<FlatpakPlatform> for FlatpakApp {
    fn needs_proton(&self, _platform: &FlatpakPlatform) -> bool {
        false
    }

    fn create_symlinks(&self, _platform: &FlatpakPlatform) -> bool {
        false
    }
}

impl FlatpakPlatform {
    fn get_flatpak_apps(&self) -> eyre::Result<Vec<FlatpakApp>> {
        let output = get_flatpak_applications()?;
        Ok(parse_flatpak_list(&String::from_utf8_lossy(&output.stdout)))
    }
}

/// BoilR's own app ids (the release and test builds); BoilR never imports itself.
const BOILR_APP_ID: &str = "io.github.philipk.boilr";

/// Parses `flatpak list --app --columns=name,application` output.
fn parse_flatpak_list(output: &str) -> Vec<FlatpakApp> {
    output
        .lines()
        .filter_map(|line| {
            let mut split = line.split('\t');
            let name = split.next()?;
            let id = split.next()?;
            Some(FlatpakApp {
                name: name.to_string(),
                id: id.to_string(),
            })
        })
        .filter(|app| app.id != BOILR_APP_ID && !app.id.starts_with(&format!("{BOILR_APP_ID}.")))
        .collect()
}

fn get_flatpak_applications() -> std::io::Result<std::process::Output> {
    use std::process::Command;
    #[cfg(not(feature = "flatpak"))]
    {
        let mut command = Command::new("flatpak");
        command
            .arg("list")
            .arg("--app")
            .arg("--columns=name,application")
            .output()
    }
    #[cfg(feature = "flatpak")]
    {
        let mut command = Command::new("flatpak-spawn");
        command
            .arg("--host")
            .arg("flatpak")
            .arg("list")
            .arg("--app")
            .arg("--columns=name,application")
            .output()
    }
}

impl FromSettingsString for FlatpakPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        FlatpakPlatform {
            settings: load_settings(s),
        }
    }
}

impl GamesPlatform for FlatpakPlatform {
    fn name(&self) -> &str {
        "Flatpak"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts(self, self.get_flatpak_apps())
    }

    #[cfg(feature = "egui-ui")]
    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Flatpak");
        ui.checkbox(&mut self.settings.enabled, "Import from Flatpak");
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn code_name(&self) -> &str {
        "flatpak"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_apps_but_not_boilr_itself() {
        let output = "OrcaSlicer\tio.github.softfever.OrcaSlicer\n\
                      BoilR\tio.github.philipk.boilr\n\
                      Devel\tio.github.philipk.boilr.Devel\n\
                      Not BoilR\tio.github.philipk.boilrish\n";
        let ids: Vec<String> = parse_flatpak_list(output)
            .into_iter()
            .map(|a| a.id)
            .collect();
        assert_eq!(
            ids,
            vec![
                "io.github.softfever.OrcaSlicer",
                "io.github.philipk.boilrish"
            ]
        );
    }
}
