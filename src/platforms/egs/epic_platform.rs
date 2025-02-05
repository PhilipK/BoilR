use crate::platforms::{
    load_settings, to_shortcuts, FromSettingsString, GamesPlatform, NeedsProton, ShortcutToImport,
};

use super::{get_egs_manifests, settings::EpicGamesLauncherSettings, ManifestItem};

#[derive(Clone)]
pub struct EpicPlatform {
    pub(crate) settings: EpicGamesLauncherSettings,
    pub(crate) epic_manifests: Option<Vec<ManifestItem>>,
}

impl FromSettingsString for EpicPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        EpicPlatform {
            settings: load_settings(s),
            epic_manifests: None,
        }
    }
}

impl NeedsProton<EpicPlatform> for ManifestItem {
    fn needs_proton(&self, _platform: &EpicPlatform) -> bool {
        #[cfg(target_family = "unix")]
        return true;
        #[cfg(target_os = "windows")]
        return false;
    }

    fn create_symlinks(&self, _platform: &EpicPlatform) -> bool {
        false
    }
}

impl GamesPlatform for EpicPlatform {
    fn name(&self) -> &str {
        "Epic"
    }

    fn code_name(&self) -> &str {
        "epic_games"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts(self, get_egs_manifests(&self.settings))
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        self.render_epic_settings(ui)
    }

    fn get_settings_serilizable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }
}
