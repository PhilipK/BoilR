use crate::platforms::{
    load_settings, to_shortcuts, FromSettingsString, NeedsPorton, ShortcutToImport,
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

impl EpicPlatform {
    pub fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts(self, get_egs_manifests(&self.settings))
    }
}

impl NeedsPorton<EpicPlatform> for ManifestItem {
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
