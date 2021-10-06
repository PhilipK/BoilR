use crate::platform::Platform;

use super::{
    get_egs_manifests, get_manifests::EpicGamesManifestsError, EpicGamesLauncherSettings,
    ManifestItem,
};

pub struct EpicPlatform {
    settings: EpicGamesLauncherSettings,
}

impl EpicPlatform {
    pub fn new(settings: EpicGamesLauncherSettings) -> Self {
        EpicPlatform { settings }
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

    #[cfg(target_os = "linux")]
    fn create_symlinks(&self) -> bool {
        self.settings.create_symlinks
    }
}
