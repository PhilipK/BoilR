
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
