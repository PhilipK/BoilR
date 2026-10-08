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
                "run --command=bottles-cli com.usebottles.bottles run -b \"{}\" -p \"{}\"",
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
    if val.is_null() || (val.is_array() && val.as_array().is_some_and(|a| a.is_empty())) {
        return Ok(vec![]);
    }
    eyre::bail!("Unexpected JSON structure from bottles-cli");
}

fn get_bottles() -> eyre::Result<(Vec<Bottle>, bool)> {
    let output = get_bottles_output()?;
    let bottles = parse_bottles(&output.json)?;
    Ok((bottles, output.is_flatpak))
}

/// The outcome of one `bottles-cli ... list bottles` attempt.
enum ListAttempt {
    /// The command ran and exited successfully, with its stdout.
    Ran(String),
    /// The command ran but exited non-zero, with whatever it wrote to stderr.
    Failed(String),
    /// The command could not be spawned at all.
    Unavailable,
}

fn run_list_command(mut command: Command) -> ListAttempt {
    match command.arg("-j").arg("list").arg("bottles").output() {
        Ok(out) if out.status.success() => {
            ListAttempt::Ran(String::from_utf8_lossy(&out.stdout).into_owned())
        }
        Ok(out) => ListAttempt::Failed(String::from_utf8_lossy(&out.stderr).into_owned()),
        Err(_) => ListAttempt::Unavailable,
    }
}

fn flatpak_list_command() -> Command {
    #[cfg(not(feature = "flatpak"))]
    let mut command = Command::new("flatpak");
    #[cfg(feature = "flatpak")]
    let mut command = {
        let mut command = Command::new("flatpak-spawn");
        command.arg("--host").arg("flatpak");
        command
    };
    command
        .arg("run")
        .arg("--command=bottles-cli")
        .arg("com.usebottles.bottles");
    command
}

fn native_list_command() -> Command {
    #[cfg(not(feature = "flatpak"))]
    let command = Command::new("bottles-cli");
    #[cfg(feature = "flatpak")]
    let command = {
        let mut command = Command::new("flatpak-spawn");
        command.arg("--host").arg("bottles-cli");
        command
    };
    command
}

fn get_bottles_output() -> eyre::Result<BottlesOutput> {
    // 1. Try flatpak bottles-cli
    let flatpak = run_list_command(flatpak_list_command());

    // A command that ran successfully is authoritative even when it listed nothing:
    // bottles-cli exits cleanly with empty output when the user has no bottles, and that
    // is an empty list rather than a failure.
    if let ListAttempt::Ran(json) = &flatpak {
        return Ok(BottlesOutput {
            json: json.clone(),
            is_flatpak: true,
        });
    }

    // 2. Fallback to native bottles-cli
    if let ListAttempt::Ran(json) = run_list_command(native_list_command()) {
        return Ok(BottlesOutput {
            json,
            is_flatpak: false,
        });
    }

    // Nothing usable ran. Prefer the Flatpak stderr when there is one so a real Bottles
    // failure is not masked as a missing install; bottles_cli_error() keeps the "not found"
    // wording the Tauri interface keys off. With no stderr at all neither command is
    // installed, which must not reach the user as "0 games".
    match flatpak {
        ListAttempt::Failed(stderr) => Err(bottles_cli_error(&stderr)),
        _ => {
            eyre::bail!(
                "Bottles not found: neither the Flatpak com.usebottles.bottles nor a native \
             bottles-cli is installed"
            );
        }
    }
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

    /// When neither command could be spawned there is no stderr to forward, but the user
    /// must still see a "not found" message. The Tauri interface files errors containing
    /// "not found" under "not installed" rather than "needs a look"
    /// (apps/boilr-tauri/src/lib/format.ts), so wording this as a generic failure would
    /// show every user without Bottles a broken launcher again.
    #[test]
    fn neither_command_available_reads_as_not_found() {
        let err = match run_list_command(Command::new("boilr-no-such-bottles-cli")) {
            ListAttempt::Failed(stderr) => bottles_cli_error(&stderr),
            _ => eyre::eyre!(
                "Bottles not found: neither the Flatpak com.usebottles.bottles nor a native \
                 bottles-cli is installed"
            ),
        };
        assert!(
            err.to_string().contains("not found"),
            "expected a not-found error, got {}",
            err
        );
    }

    /// A Flatpak failure that is not a missing install has to keep its own stderr, so a
    /// real Bottles error is not hidden behind a "not found" message.
    #[test]
    fn flatpak_failure_keeps_its_stderr() {
        let err = bottles_cli_error("Traceback: bottles exploded");
        assert!(err.to_string().contains("bottles exploded"));
        assert!(!err.to_string().contains("not found"));
    }

    #[test]
    fn test_parse_bottles_empty_returns_empty_vec() -> eyre::Result<()> {
        let res = parse_bottles("")?;
        assert!(res.is_empty());

        let res = parse_bottles("   \n\t  ")?;
        assert!(res.is_empty());
        Ok(())
    }

    #[test]
    fn test_parse_bottles_valid_map() -> eyre::Result<()> {
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
        let res = parse_bottles(json)?;
        let [bottle] = res.as_slice() else {
            eyre::bail!("expected exactly one bottle, got {}", res.len());
        };
        assert_eq!(bottle.name, "my-bottle");
        assert_eq!(bottle.external_programs.len(), 1);
        assert_eq!(
            bottle
                .external_programs
                .get("prog1")
                .ok_or_else(|| eyre::eyre!("prog1 missing"))?
                .name,
            "Notepad"
        );
        Ok(())
    }

    #[test]
    fn test_parse_bottles_missing_external_programs() -> eyre::Result<()> {
        let json = r#"{
            "my-bottle": {
                "Name": "my-bottle"
            }
        }"#;
        let res = parse_bottles(json)?;
        let [bottle] = res.as_slice() else {
            eyre::bail!("expected exactly one bottle, got {}", res.len());
        };
        assert_eq!(bottle.name, "my-bottle");
        assert!(bottle.external_programs.is_empty());
        Ok(())
    }

    #[test]
    fn test_bottles_app_shortcut_generation() -> eyre::Result<()> {
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
        Ok(())
    }
}
