use steam_shortcuts_util::shortcut::ShortcutOwned;

use crate::platform::{Platform, SettingsValidity};

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

impl Platform<ManifestItem, String> for EpicPlatform {
    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn name(&self) -> &str {
        "EGS"
    }

    fn get_shortcuts(&self) -> Result<Vec<ManifestItem>, String> {
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

impl EpicPlatform {
   pub(crate) fn get_owned_shortcuts(&self) -> Result<Vec<ShortcutOwned>, String> {
        get_egs_manifests(&self.settings).map(|ms| {
            ms.iter()
                .map(|m| {
                    //Remove this clone
                     m.clone().into()
                })
                .collect()
        })
    }
}
