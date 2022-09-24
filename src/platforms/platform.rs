use steam_shortcuts_util::shortcut::ShortcutOwned;

use super::amazon::AmazonPlatform;
use super::bottles::BottlesPlatform;
use super::egs::EpicPlatform;
use super::flatpak::FlatpakPlatform;
use super::itch::ItchPlatform;
use super::uplay::UplayPlatform;

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

#[derive(Clone)]
pub enum PlatformEnum {
    Amazon(AmazonPlatform),
    Bottles(BottlesPlatform),
    Epic(EpicPlatform),
    Uplay(UplayPlatform),
    Itch(ItchPlatform),
    Flatpak(FlatpakPlatform),
}

impl PlatformEnum {
    pub fn name(&self) -> &str {
        match self {
            PlatformEnum::Amazon(_) => "Amazon",
            PlatformEnum::Bottles(_) => "Bottles",
            PlatformEnum::Epic(_) => "Epic",
            PlatformEnum::Uplay(_) => "Uplay",
            PlatformEnum::Itch(_) => "Itch",
            PlatformEnum::Flatpak(_) => "Flatpak",
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            PlatformEnum::Amazon(p) => p.settings.enabled,
            PlatformEnum::Bottles(p) => p.settings.enabled,
            PlatformEnum::Epic(p) => p.settings.enabled,
            PlatformEnum::Uplay(p) => p.settings.enabled,
            PlatformEnum::Itch(p) => p.settings.enabled,
            PlatformEnum::Flatpak(p) => p.settings.enabled,
        }
    }

    pub fn render_ui(&mut self, ui: &mut egui::Ui) {
        match self {
            PlatformEnum::Amazon(p) => p.render_amazon_settings(ui),
            PlatformEnum::Bottles(p) => p.render_bottles_settings(ui),
            PlatformEnum::Epic(p) => p.render_epic_settings(ui),
            PlatformEnum::Uplay(p) => p.render_uplay_settings(ui),
            PlatformEnum::Itch(p) => p.render_itch_settings(ui),
            PlatformEnum::Flatpak(p) => p.render_flatpak_settings(ui),
        }
    }

    pub fn get_shortcuts(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        match self {
            PlatformEnum::Amazon(p) => p.get_shortcut_info(),
            PlatformEnum::Bottles(p) => p.get_shortcut_info(),
            PlatformEnum::Epic(p) => p.get_shortcut_info(),
            PlatformEnum::Uplay(p) => p.get_shortcut_info(),
            PlatformEnum::Itch(p) => p.get_shortcut_info(),
            PlatformEnum::Flatpak(p) => p.get_shortcut_info(),
        }
    }
}

pub struct ShortcutToImport {
    pub shortcut: ShortcutOwned,
    pub needs_proton: bool,
    pub needs_symlinks: bool,
}

pub fn to_shortcuts<T, P>(
    platform: &P,
    into_shortcuts: Result<Vec<T>, eyre::ErrReport>,
) -> eyre::Result<Vec<ShortcutToImport>>
where
    T: Into<ShortcutOwned>,
    T: NeedsPorton<P>,
{
    let shortcuts = into_shortcuts?;
    let mut shortcut_info = vec![];
    for m in shortcuts {
        let needs_proton = m.needs_proton(platform);
        let needs_symlinks = m.create_symlinks(platform);
        let shortcut = m.into();
        shortcut_info.push(ShortcutToImport {
            shortcut,
            needs_proton,
            needs_symlinks,
        });
    }
    Ok(shortcut_info)
}

pub fn to_shortcuts_simple<T>(
    into_shortcuts: Result<Vec<T>, eyre::ErrReport>,
) -> eyre::Result<Vec<ShortcutToImport>>
where
    T: Into<ShortcutOwned>,
{
    let shortcuts = into_shortcuts?;
    let mut shortcut_info = vec![];
    for m in shortcuts {
        let needs_proton = false;
        let needs_symlinks = false;
        let shortcut = m.into();
        shortcut_info.push(ShortcutToImport {
            shortcut,
            needs_proton,
            needs_symlinks,
        });
    }
    Ok(shortcut_info)
}

pub type Platforms = [PlatformEnum; 6];

pub fn get_platforms(settings: &crate::settings::Settings) -> Platforms {
    [
        PlatformEnum::Epic(EpicPlatform {
            epic_manifests: None,
            settings: settings.epic_games.clone(),
        }),
        PlatformEnum::Amazon(AmazonPlatform {
            settings: settings.amazon.clone(),
        }),
        PlatformEnum::Bottles(BottlesPlatform {
            settings: settings.bottles.clone(),
        }),
        PlatformEnum::Uplay(UplayPlatform {
            settings: settings.uplay.clone(),
        }),
        PlatformEnum::Itch(ItchPlatform {
            settings: settings.itch.clone(),
        }),
        PlatformEnum::Flatpak(FlatpakPlatform {
            settings: settings.flatpak.clone(),
        }),
    ]
}

pub trait NeedsPorton<P> {
    fn needs_proton(&self, platform: &P) -> bool;

    fn create_symlinks(&self, platform: &P) -> bool;
}
