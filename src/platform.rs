use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

use crate::{amazon::AmazonPlatform, bottles::BottlesPlatform, egs::EpicPlatform};

pub trait Platform<T, E>
where
    T: Into<ShortcutOwned>,
{
    fn enabled(&self) -> bool;

    fn name(&self) -> &str;

    fn get_shortcuts(&self) -> Result<Vec<T>, E>;

    fn settings_valid(&self) -> SettingsValidity;

    #[cfg(target_family = "unix")]
    fn create_symlinks(&self) -> bool;

    // HOME/.local/share/Steam/config/config.vdf
    fn needs_proton(&self, input: &T) -> bool;
}

pub enum SettingsValidity {
    Valid,
    Invalid { reason: String },
}

pub enum PlatformEnum {
    Amazon(AmazonPlatform),
    Bottles(BottlesPlatform),
    Epic(EpicPlatform),
}

impl PlatformEnum {
    pub fn name(&self) -> &str {
        match self {
            PlatformEnum::Amazon(_) => "Amazon",
            PlatformEnum::Bottles(_) => "Bottles",
            PlatformEnum::Epic(_) => "Epic",
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            PlatformEnum::Amazon(p) => p.settings.enabled,
            PlatformEnum::Bottles(p) => p.settings.enabled,
            PlatformEnum::Epic(p) => p.settings.enabled,
        }
    }

    pub fn render_ui(&mut self, ui: &mut egui::Ui) {
        match self{
            PlatformEnum::Amazon(_) => todo!(),
            PlatformEnum::Bottles(_) => todo!(),
            PlatformEnum::Epic(p) => p.render_epic_settings(ui),
        }
    }

    pub fn get_shortcuts(&self) -> Result<Vec<ShortcutOwned>,String>{
        match self{
            PlatformEnum::Amazon(_) => todo!(),
            PlatformEnum::Bottles(_) => todo!(),
            PlatformEnum::Epic(p) => p.get_owned_shortcuts(),
        }
    }
}

impl PlatformEnum {
   
}
