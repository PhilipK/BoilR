#[cfg(feature = "egui-ui")]
use std::{collections::BTreeMap, path::Path};

use serde::{Deserialize, Serialize};

use crate::platforms::{
    load_settings, to_shortcuts, FromSettingsString, GamesPlatform, ShortcutToImport,
};

use super::scanner::scan_directories_with_selections;

#[cfg(feature = "egui-ui")]
use super::scanner::{find_unmatched_executables, ManualExecutableCandidate};

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub(crate) struct StandaloneSettings {
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) scan_directories: Vec<String>,
    #[serde(default)]
    pub(crate) selected_executables: Vec<String>,
}

#[derive(Clone, Default)]
pub(crate) struct StandalonePlatform {
    pub(crate) settings: StandaloneSettings,

    #[cfg(feature = "egui-ui")]
    pending_directory: String,

    #[cfg(feature = "egui-ui")]
    candidate_filter: String,

    #[cfg(feature = "egui-ui")]
    unmatched_executables: Option<Result<Vec<ManualExecutableCandidate>, String>>,
}

impl FromSettingsString for StandalonePlatform {
    fn from_settings_string<S: AsRef<str>>(input: S) -> Self {
        Self {
            settings: load_settings(input),

            #[cfg(feature = "egui-ui")]
            pending_directory: String::new(),

            #[cfg(feature = "egui-ui")]
            candidate_filter: String::new(),

            #[cfg(feature = "egui-ui")]
            unmatched_executables: None,
        }
    }
}

impl GamesPlatform for StandalonePlatform {
    fn name(&self) -> &str {
        "Standalone"
    }

    fn code_name(&self) -> &str {
        "standalone"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts(
            self,
            scan_directories_with_selections(
                &self.settings.scan_directories,
                &self.settings.selected_executables,
            ),
        )
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    #[cfg(feature = "egui-ui")]
    fn render_ui(&mut self, ui: &mut egui::Ui) {
        self.render_standalone_settings(ui);
    }
}

#[cfg(feature = "egui-ui")]
impl StandalonePlatform {
    fn render_standalone_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Standalone / Loose Folders");
        ui.checkbox(&mut self.settings.enabled, "Import standalone games");

        if !self.settings.enabled {
            return;
        }

        ui.label("Recursively imports .exe files whose names match an ancestor folder name.");

        let mut directories_changed = false;
        let mut remove_index = None;
        for (index, directory) in self.settings.scan_directories.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                if ui.text_edit_singleline(directory).changed() {
                    directories_changed = true;
                }
                if ui.button("Remove").clicked() {
                    remove_index = Some(index);
                }
            });
        }

        if let Some(index) = remove_index {
            if index < self.settings.scan_directories.len() {
                self.settings.scan_directories.remove(index);
                directories_changed = true;
            }
        }

        ui.horizontal(|ui| {
            ui.label("Folder:");
            ui.text_edit_singleline(&mut self.pending_directory);
            if ui.button("Add folder").clicked() {
                let directory = self.pending_directory.trim();
                let is_duplicate = self
                    .settings
                    .scan_directories
                    .iter()
                    .any(|existing| existing == directory);
                if !directory.is_empty() && !is_duplicate {
                    self.settings.scan_directories.push(directory.to_owned());
                    self.pending_directory.clear();
                    directories_changed = true;
                }
            }
        });

        if directories_changed {
            self.unmatched_executables = None;
        }

        ui.separator();
        ui.label("Executable overrides");
        ui.label("Choose an executable when its name does not match the game folder, such as Death Stranding 2 / DS2.exe.");
        ui.horizontal(|ui| {
            let scan_label = if self.unmatched_executables.is_some() {
                "Refresh candidates"
            } else {
                "Find unmatched executables"
            };
            if ui.button(scan_label).clicked() {
                self.unmatched_executables = Some(
                    find_unmatched_executables(&self.settings.scan_directories)
                        .map_err(|error| error.to_string()),
                );
            }
            if !self.settings.selected_executables.is_empty()
                && ui.button("Clear all selections").clicked()
            {
                self.settings.selected_executables.clear();
            }
        });

        match self.unmatched_executables.clone() {
            Some(Ok(candidates)) => self.render_unmatched_executables(ui, &candidates),
            Some(Err(error)) => {
                ui.colored_label(egui::Color32::RED, error);
            }
            None => {}
        }
    }

    fn render_unmatched_executables(
        &mut self,
        ui: &mut egui::Ui,
        candidates: &[ManualExecutableCandidate],
    ) {
        if candidates.is_empty() {
            ui.label("No unmatched executables found");
            return;
        }

        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.add(
                egui::TextEdit::singleline(&mut self.candidate_filter)
                    .hint_text("Game, executable, or path"),
            );
        });

        let filter = self.candidate_filter.trim().to_lowercase();
        let mut groups: BTreeMap<_, Vec<&ManualExecutableCandidate>> = BTreeMap::new();
        for candidate in candidates {
            let searchable = format!(
                "{} {} {}",
                candidate.title,
                candidate.executable.display(),
                candidate.game_directory.display()
            )
            .to_lowercase();
            if filter.is_empty() || searchable.contains(&filter) {
                groups
                    .entry(candidate.game_directory.clone())
                    .or_default()
                    .push(candidate);
            }
        }

        let visible_candidate_count: usize = groups.values().map(Vec::len).sum();
        ui.label(format!(
            "{} candidate{} in {} game folder{}; {} selected",
            visible_candidate_count,
            plural_suffix(visible_candidate_count),
            groups.len(),
            plural_suffix(groups.len()),
            self.settings.selected_executables.len()
        ));

        if groups.is_empty() {
            ui.label("No candidates match the filter");
            return;
        }

        let open_small_result_sets = groups.len() <= 4;
        for (game_directory, group) in groups {
            self.render_candidate_group(ui, &game_directory, &group, open_small_result_sets);
        }
    }

    fn render_candidate_group(
        &mut self,
        ui: &mut egui::Ui,
        game_directory: &Path,
        candidates: &[&ManualExecutableCandidate],
        open_by_default: bool,
    ) {
        let Some(first_candidate) = candidates.first() else {
            return;
        };
        let candidate_paths: Vec<String> = candidates
            .iter()
            .map(|candidate| candidate.executable.to_string_lossy().to_string())
            .collect();
        let selected_path = candidate_paths.iter().find(|path| {
            self.settings
                .selected_executables
                .iter()
                .any(|selected| selected == *path)
        });
        let selected_name = selected_path
            .and_then(|path| Path::new(path).file_name())
            .map(|name| name.to_string_lossy().to_string());
        let header = match selected_name {
            Some(name) => format!("{} — selected: {name}", first_candidate.title),
            None => format!(
                "{} — {} candidate{}",
                first_candidate.title,
                candidates.len(),
                plural_suffix(candidates.len())
            ),
        };

        egui::CollapsingHeader::new(header)
            .id_salt(("standalone-executables", game_directory))
            .default_open(open_by_default || selected_path.is_some())
            .show(ui, |ui| {
                ui.weak(game_directory.display().to_string());
                for candidate in candidates {
                    let path = candidate.executable.to_string_lossy().to_string();
                    let file_name = candidate
                        .executable
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.clone());
                    let selected = selected_path.is_some_and(|selected| selected == &path);
                    if ui.radio(selected, file_name).clicked() && !selected {
                        self.settings
                            .selected_executables
                            .retain(|existing| !candidate_paths.contains(existing));
                        self.settings.selected_executables.push(path.clone());
                    }
                    ui.add(
                        egui::Label::new(egui::RichText::new(path).monospace().weak())
                            .selectable(true)
                            .wrap(),
                    );
                }
                if selected_path.is_some() && ui.button("Clear selection for this game").clicked() {
                    self.settings
                        .selected_executables
                        .retain(|existing| !candidate_paths.contains(existing));
                }
            });
    }
}

#[cfg(feature = "egui-ui")]
fn plural_suffix(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_through_toml() -> eyre::Result<()> {
        let settings = StandaloneSettings {
            enabled: true,
            scan_directories: vec!["D:\\Games".to_owned(), "E:\\Portable Games".to_owned()],
            selected_executables: vec!["D:\\Games\\Death Stranding 2\\DS2.exe".to_owned()],
        };

        let serialized = toml::to_string(&settings)?;
        let deserialized: StandaloneSettings = toml::from_str(&serialized)?;

        assert!(deserialized.enabled);
        assert_eq!(deserialized.scan_directories, settings.scan_directories);
        assert_eq!(
            deserialized.selected_executables,
            settings.selected_executables
        );
        Ok(())
    }

    #[test]
    fn missing_directories_field_uses_default() -> eyre::Result<()> {
        let settings: StandaloneSettings = toml::from_str("enabled = true")?;

        assert!(settings.enabled);
        assert!(settings.scan_directories.is_empty());
        assert!(settings.selected_executables.is_empty());
        Ok(())
    }

    #[test]
    fn exposes_expected_platform_identity() {
        let platform = StandalonePlatform::default();

        assert_eq!(platform.name(), "Standalone");
        assert_eq!(platform.code_name(), "standalone");
        assert!(!platform.enabled());
    }

    #[cfg(feature = "egui-ui")]
    #[test]
    fn settings_ui_renders_at_narrow_width() {
        egui::__run_test_ui(|ui| {
            ui.set_max_width(100.0);
            let mut platform = StandalonePlatform {
                settings: StandaloneSettings {
                    enabled: true,
                    scan_directories: vec!["D:\\A very long unavailable folder name".to_owned()],
                    selected_executables: Vec::new(),
                },
                pending_directory: "E:\\Another long folder name".to_owned(),
                candidate_filter: String::new(),
                unmatched_executables: Some(Ok(vec![ManualExecutableCandidate {
                    title: "Death Stranding 2".to_owned(),
                    game_directory: std::path::PathBuf::from("D:\\Games\\Death Stranding 2"),
                    executable: std::path::PathBuf::from("D:\\Games\\Death Stranding 2\\DS2.exe"),
                }])),
            };

            platform.render_standalone_settings(ui);
        });
    }
}
