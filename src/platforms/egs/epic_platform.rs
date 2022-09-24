
use crate::platforms::NeedsPorton;

use super::{get_egs_manifests, EpicGamesLauncherSettings, ManifestItem};

#[derive(Clone)]
pub struct EpicPlatform {
    pub(crate) settings: EpicGamesLauncherSettings,
    pub(crate) epic_manifests: Option<Vec<ManifestItem>>,
}

impl EpicPlatform {
    pub fn new(settings: &EpicGamesLauncherSettings) -> Self {
        EpicPlatform {
            settings: settings.clone(),
            epic_manifests: None,
        }
    }
}

impl EpicPlatform {

   pub(crate) fn get_epic_games(&self) -> eyre::Result<Vec<ManifestItem>> {
        get_egs_manifests(&self.settings)        
    }
}

impl NeedsPorton<EpicPlatform> for ManifestItem{
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