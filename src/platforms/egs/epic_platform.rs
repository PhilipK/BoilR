use crate::platforms::{to_shortcuts, NeedsPorton, ShortcutToImport};

use super::{get_egs_manifests, EpicGamesLauncherSettings, ManifestItem};

#[derive(Clone)]
pub struct EpicPlatform {
    pub(crate) settings: EpicGamesLauncherSettings,
    pub(crate) epic_manifests: Option<Vec<ManifestItem>>,
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
