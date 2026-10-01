use super::game_list_parser::parse_lutris_games;
use super::lutris_game::LutrisGame;
use super::settings::LutrisSettings;
use crate::platforms::{
    load_settings, to_shortcuts_simple, FromSettingsString, GamesPlatform, ShortcutToImport,
};
use std::process::Command;

#[derive(Clone)]
pub struct LutrisPlatform {
    pub settings: LutrisSettings,
}

impl LutrisPlatform {
    fn get_shortcuts(&self) -> eyre::Result<Vec<LutrisGame>> {
        let output = get_lutris_command_output(&self.settings)?;
        let games = parse_lutris_games(output.as_str());
        let installed = self.settings.installed;
        let mut res = vec![];
        for mut game in games {
            let service = if installed {
                game.runner.clone().unwrap_or_default()
            } else {
                game.service.clone().unwrap_or_default()
            };
            if service != "steam" {
                game.settings = Some(self.settings.clone());
                res.push(game);
            }
        }
        Ok(res)
    }
}

fn get_lutris_command_output(settings: &LutrisSettings) -> eyre::Result<String> {
    let output = if settings.flatpak {
        let flatpak_image = &settings.flatpak_image;
        #[cfg(not(feature = "flatpak"))]
        {
            let mut command = Command::new("flatpak");
            command.arg("run").arg(flatpak_image).arg("--json");
            if settings.installed {
                command.arg("-lo").output()?
            } else {
                command.arg("-a").output()?
            }
        }
        #[cfg(feature = "flatpak")]
        {
            let mut command = Command::new("flatpak-spawn");
            command
                .arg("--host")
                .arg("flatpak")
                .arg("run")
                .arg(flatpak_image)
                .arg("--json");
            if settings.installed {
                command.arg("-lo").output()?
            } else {
                command.arg("-a").output()?
            }
        }
    } else {
        // Inside BoilR's Flatpak sandbox a native Lutris is only reachable on the host (#262).
        #[cfg(not(feature = "flatpak"))]
        let mut command = Command::new(&settings.executable);
        #[cfg(feature = "flatpak")]
        let mut command = {
            let mut command = Command::new("flatpak-spawn");
            command.arg("--host").arg(&settings.executable);
            command
        };
        command.arg("--json");
        let output = if settings.installed {
            command.arg("-lo").output()?
        } else {
            command.arg("-a").output()?
        };
        #[cfg(feature = "flatpak")]
        if !output.status.success() && output.stdout.is_empty() {
            let on_host = host_has_command(&settings.executable);
            return Err(host_lutris_error(
                &settings.executable,
                on_host,
                &String::from_utf8_lossy(&output.stderr),
            ));
        }
        output
    };

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Whether the host can find `executable`. flatpak-spawn's own error text is translated on
/// the host, so ask the host shell instead of matching that text.
#[cfg(feature = "flatpak")]
fn host_has_command(executable: &str) -> bool {
    Command::new("flatpak-spawn")
        .args([
            "--host",
            "sh",
            "-c",
            "command -v -- \"$1\"",
            "sh",
            executable,
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// A failed host run of a native Lutris. "not found" marks it as not installed in the UI.
#[cfg(any(feature = "flatpak", test))]
fn host_lutris_error(executable: &str, on_host: bool, stderr: &str) -> eyre::Report {
    if !on_host {
        return eyre::eyre!("Lutris not found: {executable} is not installed on the host");
    }
    match stderr.lines().rev().find(|l| !l.trim().is_empty()) {
        Some(line) => eyre::eyre!("{executable} failed on the host: {}", line.trim()),
        None => eyre::eyre!("{executable} failed on the host"),
    }
}

impl FromSettingsString for LutrisPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        LutrisPlatform {
            settings: load_settings(s),
        }
    }
}

impl GamesPlatform for LutrisPlatform {
    fn name(&self) -> &str {
        "Lutris"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts_simple(self.get_shortcuts())
    }

    #[cfg(feature = "egui-ui")]
    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Lutris");
        ui.checkbox(&mut self.settings.enabled, "Import from Lutris");
        if self.settings.enabled {
            ui.checkbox(&mut self.settings.installed, "Search installed only");
            ui.checkbox(&mut self.settings.flatpak, "Flatpak version");
            if !self.settings.flatpak {
                ui.horizontal(|ui| {
                    let lutris_location = &mut self.settings.executable;
                    ui.label("Lutris Location: ");
                    ui.text_edit_singleline(lutris_location);
                });
            } else {
                ui.horizontal(|ui| {
                    let flatpak_image = &mut self.settings.flatpak_image;
                    ui.label("Flatpak image");
                    ui.text_edit_singleline(flatpak_image);
                });
            }
        }
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn code_name(&self) -> &str {
        "lutris"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_host_lutris_reads_as_not_found() {
        let error = host_lutris_error("lutris", false, "Datei oder Verzeichnis nicht gefunden");
        assert!(error.to_string().contains("not found"));
    }

    #[test]
    fn failing_host_lutris_keeps_its_last_error_line() {
        let stderr = "Traceback (most recent call last):\n  File \"x\"\nKeyError: 'games'\n\n";
        let error = host_lutris_error("lutris", true, stderr);
        assert_eq!(
            error.to_string(),
            "lutris failed on the host: KeyError: 'games'"
        );
    }
}
