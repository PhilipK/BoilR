use crate::platform::{Platform, SettingsValidity};

use super::{
    get_egs_manifests, get_manifests::EpicGamesManifestsError, EpicGamesLauncherSettings,
    ManifestItem,
};

pub struct EpicPlatform {
    settings: EpicGamesLauncherSettings,
}

impl EpicPlatform {
    pub fn new(settings: &EpicGamesLauncherSettings) -> Self {
        EpicPlatform {
            settings: settings.clone(),
        }
    }
}

impl Platform<ManifestItem, EpicGamesManifestsError> for EpicPlatform {
    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn name(&self) -> &str {
        "EGS"
    }

    fn get_shortcuts(&self) -> Result<Vec<ManifestItem>, EpicGamesManifestsError> {
        get_egs_manifests(&self.settings)
    }

    #[cfg(target_family = "unix")]
    fn create_symlinks(&self) -> bool {
        false
    }

    fn settings_valid(&self) -> SettingsValidity {
        let shortcuts_res = self.get_shortcuts();
        match shortcuts_res {
            Ok(_) => SettingsValidity::Valid,
            Err(err) => SettingsValidity::Invalid {
                reason: format!("{}", err),
            },
        }
    }

    fn needs_proton(&self, _input: &ManifestItem) -> bool {
        #[cfg(target_family = "unix")]
        return true;
        #[cfg(target_os = "windows")]
        return false;
    }
}
