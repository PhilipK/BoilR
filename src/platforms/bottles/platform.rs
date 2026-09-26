use std::process::Command;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub is_flatpak: bool,
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
        if app.is_flatpak {
            let launch_parameter = format!(
                "run --command=bottles-cli com.usebottles.bottles run --args-replace -b \"{}\" -p \"{}\"",
                app.bottle, app.name
            );
            Shortcut::new("0", &app.name, "flatpak", "", "", "", &launch_parameter).to_owned()
        } else {
            let launch_parameter = format!("run -b \"{}\" -p \"{}\"", app.bottle, app.name);
            Shortcut::new("0", &app.name, "bottles-cli", "", "", "", &launch_parameter).to_owned()
        }
    }
}

#[derive(Debug, Clone)]
struct BottlesOutput {
    json: String,
    is_flatpak: bool,
}

fn parse_bottles(json: &str) -> eyre::Result<Vec<Bottle>> {
    let trimmed = json.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }
    if let Ok(map) = serde_json::from_str::<HashMap<String, Bottle>>(trimmed) {
        return Ok(map.into_values().collect());
    }
    if let Ok(list) = serde_json::from_str::<Vec<Bottle>>(trimmed) {
        return Ok(list);
    }
    let val: serde_json::Value = serde_json::from_str(trimmed)?;
    if val.is_null() || (val.is_array() && val.as_array().map_or(false, |a| a.is_empty())) {
        return Ok(vec![]);
    }
    eyre::bail!("Unexpected JSON structure from bottles-cli");
}

fn get_bottles() -> eyre::Result<(Vec<Bottle>, bool)> {
    let output = get_bottles_output()?;
    let bottles = parse_bottles(&output.json)?;
    Ok((bottles, output.is_flatpak))
}

fn get_bottles_output() -> eyre::Result<BottlesOutput> {
    // Bottles may be reachable as a Flatpak or as a native bottles-cli. A command that
    // ran successfully is authoritative, even when it printed nothing: bottles-cli exits
    // with empty output when the user simply has no bottles, and that is an empty list,
    // not a failure. Only when nothing ran at all do we surface an error, so a missing
    // Bottles install cannot be mistaken for "0 games".
    let mut flatpak_stderr: Option<String> = None;

    #[cfg(not(feature = "flatpak"))]
    {
        // 1. Try flatpak bottles-cli
        if let Ok(out) = Command::new("flatpak")
            .arg("run")
            .arg("--command=bottles-cli")
            .arg("com.usebottles.bottles")
            .arg("-j")
            .arg("list")
            .arg("bottles")
            .output()
        {
            if out.status.success() {
                return Ok(BottlesOutput {
                    json: String::from_utf8_lossy(&out.stdout).to_string(),
                    is_flatpak: true,
                });
            }
            flatpak_stderr = Some(String::from_utf8_lossy(&out.stderr).to_string());
        }

        // 2. Fallback to native bottles-cli
        if let Ok(out) = Command::new("bottles-cli")
            .arg("-j")
            .arg("list")
            .arg("bottles")
            .output()
        {
            if out.status.success() {
                return Ok(BottlesOutput {
                    json: String::from_utf8_lossy(&out.stdout).to_string(),
                    is_flatpak: false,
                });
            }
        }
    }
    #[cfg(feature = "flatpak")]
    {
        // 1. Try flatpak bottles-cli on host
        if let Ok(out) = Command::new("flatpak-spawn")
            .arg("--host")
            .arg("flatpak")
            .arg("run")
            .arg("--command=bottles-cli")
            .arg("com.usebottles.bottles")
            .arg("-j")
            .arg("list")
            .arg("bottles")
            .output()
        {
            if out.status.success() {
                return Ok(BottlesOutput {
                    json: String::from_utf8_lossy(&out.stdout).to_string(),
                    is_flatpak: true,
                });
            }
            flatpak_stderr = Some(String::from_utf8_lossy(&out.stderr).to_string());
        }

        // 2. Fallback to native bottles-cli on host
        if let Ok(out) = Command::new("flatpak-spawn")
            .arg("--host")
            .arg("bottles-cli")
            .arg("-j")
            .arg("list")
            .arg("bottles")
            .output()
        {
            if out.status.success() {
                return Ok(BottlesOutput {
                    json: String::from_utf8_lossy(&out.stdout).to_string(),
                    is_flatpak: false,
                });
            }
        }
    }

    // Nothing ran. If the Flatpak attempt did produce stderr, prefer it so a real Bottles
    // failure is not masked as a missing install; bottles_cli_error() keeps the "not found"
    // wording the Tauri interface keys off. Otherwise neither is installed.
    Err(match flatpak_stderr {
        Some(stderr) => bottles_cli_error(&stderr),
        None => eyre::eyre!(
            "Bottles not found: neither the Flatpak com.usebottles.bottles nor a native \
             bottles-cli is installed"
        ),
    })
}

/// Without Bottles, `flatpak run` fails with "... not installed" and prints nothing to parse.
fn bottles_cli_error(stderr: &str) -> eyre::Report {
    if stderr.contains("not installed") {
        eyre::eyre!("Bottles not found: the Flatpak com.usebottles.bottles is not installed")
    } else {
        eyre::eyre!("bottles-cli failed: {}", stderr.trim())
    }
}

#[derive(Deserialize, Debug)]
struct Bottle {
    #[serde(alias = "Name")]
    pub(crate) name: String,
    #[serde(alias = "External_Programs")]
    #[serde(default)]
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
        let (bottles, is_flatpak) = get_bottles()?;
        for bottle in bottles {
            for (_id, program) in bottle.external_programs {
                res.push(BottlesApp {
                    name: program.name,
                    bottle: bottle.name.clone(),
                    is_flatpak,
                });
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

    #[cfg(feature = "egui-ui")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_bottles_reads_as_not_found() {
        let err =
            bottles_cli_error("error: app/com.usebottles.bottles/x86_64/master not installed\n");
        assert!(err.to_string().contains("not found"));
        let err = bottles_cli_error("Traceback: boom");
        assert_eq!(err.to_string(), "bottles-cli failed: Traceback: boom");
    }

    #[test]
    fn test_parse_bottles_empty_returns_empty_vec() {
        let res = parse_bottles("").unwrap();
        assert!(res.is_empty());

        let res = parse_bottles("   \n\t  ").unwrap();
        assert!(res.is_empty());
    }

    #[test]
    fn test_parse_bottles_valid_map() {
        let json = r#"{
            "my-bottle": {
                "Name": "my-bottle",
                "External_Programs": {
                    "prog1": {
                        "Name": "Notepad"
                    }
                }
            }
        }"#;
        let res = parse_bottles(json).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].name, "my-bottle");
        assert_eq!(res[0].external_programs.len(), 1);
        assert_eq!(
            res[0].external_programs.get("prog1").unwrap().name,
            "Notepad"
        );
    }

    #[test]
    fn test_parse_bottles_missing_external_programs() {
        let json = r#"{
            "my-bottle": {
                "Name": "my-bottle"
            }
        }"#;
        let res = parse_bottles(json).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].name, "my-bottle");
        assert!(res[0].external_programs.is_empty());
    }

    #[test]
    fn test_bottles_app_shortcut_generation() {
        let flatpak_app = BottlesApp {
            name: "Game".to_string(),
            bottle: "gaming".to_string(),
            is_flatpak: true,
        };
        let shortcut: ShortcutOwned = flatpak_app.into();
        assert_eq!(shortcut.app_name, "Game");
        assert_eq!(shortcut.exe, "flatpak");
        assert!(shortcut.launch_options.contains("com.usebottles.bottles"));

        let native_app = BottlesApp {
            name: "Game".to_string(),
            bottle: "gaming".to_string(),
            is_flatpak: false,
        };
        let shortcut: ShortcutOwned = native_app.into();
        assert_eq!(shortcut.app_name, "Game");
        assert_eq!(shortcut.exe, "bottles-cli");
        assert_eq!(shortcut.launch_options, "run -b \"gaming\" -p \"Game\"");
    }
}
