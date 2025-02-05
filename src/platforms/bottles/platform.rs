use std::process::Command;

use eframe::epaint::ahash::HashMap;
use serde::{Deserialize, Serialize};

use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

use crate::platforms::{
    load_settings, to_shortcuts_simple, FromSettingsString, GamesPlatform, ShortcutToImport,
};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BottlesPlatform {
    pub settings: BottlesSettings,
}

impl FromSettingsString for BottlesPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        BottlesPlatform {
            settings: load_settings(s),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BottlesApp {
    pub name: String,
    pub bottle: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BottlesSettings {
    pub enabled: bool,
}

impl Default for BottlesSettings {
    fn default() -> Self {
        #[cfg(target_family = "unix")]
        let enabled = true;

        #[cfg(not(target_family = "unix"))]
        let enabled = false;

        Self { enabled }
    }
}

impl From<BottlesApp> for ShortcutOwned {
    fn from(app: BottlesApp) -> Self {
        //
        let launch_parameter = format!(
            "run --command=bottles-cli com.usebottles.bottles run --args-replace -b \"{}\" -p \"{}\"",
            app.bottle, app.name
        );
        Shortcut::new("0", &app.name, "flatpak", "", "", "", &launch_parameter).to_owned()
    }
}

fn get_bottles() -> eyre::Result<Vec<Bottle>> {
    let json = get_bottles_output()?;
    let map: HashMap<String, Bottle> = serde_json::from_str(json.as_str())?;
    let mut res = vec![];
    for (_, value) in map {
        res.push(value);
    }
    Ok(res)
}

fn get_bottles_output() -> eyre::Result<String> {
    let output = {
        #[cfg(not(feature = "flatpak"))]
        {
            let mut command = Command::new("flatpak");
            command
                .arg("run")
                .arg("--command=bottles-cli")
                .arg("com.usebottles.bottles")
                .arg("-j")
                .arg("list")
                .arg("bottles")
                .output()?
        }
        #[cfg(feature = "flatpak")]
        {
            let mut command = Command::new("flatpak-spawn");
            command
                .arg("--host")
                .arg("flatpak")
                .arg("run")
                .arg("--command=bottles-cli")
                .arg("com.usebottles.bottles")
                .arg("-j")
                .arg("list")
                .arg("bottles")
                .output()?
        }
    };
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[derive(Deserialize, Debug)]
struct Bottle {
    #[serde(alias = "Name")]
    pub(crate) name: String,
    #[serde(alias = "External_Programs")]
    pub(crate) external_programs: HashMap<String, Program>,
}

#[derive(Deserialize, Debug)]
struct Program {
    #[serde(alias = "Name")]
    pub(crate) name: String,
}

impl BottlesPlatform {
    fn get_botttles(&self) -> eyre::Result<Vec<BottlesApp>> {
        let mut res = vec![];
        let bottles = get_bottles()?;
        for bottle in bottles {
            for (_id, program) in bottle.external_programs {
                res.push(BottlesApp {
                    name: program.name,
                    bottle: bottle.name.clone(),
                })
            }
        }
        Ok(res)
    }
}

impl GamesPlatform for BottlesPlatform {
    fn name(&self) -> &str {
        "Bottles"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts_simple(self.get_botttles())
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Bottles");
        ui.checkbox(&mut self.settings.enabled, "Import from Bottles");
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn code_name(&self) -> &str {
        "bottles"
    }
}
