use steam_shortcuts_util::shortcut::ShortcutOwned;

use super::amazon::AmazonPlatform;
use super::bottles::BottlesPlatform;
use super::egs::EpicPlatform;
use super::flatpak::FlatpakPlatform;
use super::gog::GogPlatform;
use super::heroic::HeroicPlatform;
use super::itch::ItchPlatform;
use super::legendary::LegendaryPlatform;
use super::lutris::LutrisPlatform;
use super::origin::OriginPlatform;
use super::uplay::UplayPlatform;

#[derive(Clone)]
pub enum PlatformEnum {
    Amazon(AmazonPlatform),
    Bottles(BottlesPlatform),
    Epic(EpicPlatform),
    Uplay(UplayPlatform),
    Itch(ItchPlatform),
    Flatpak(FlatpakPlatform),
    Origin(OriginPlatform),
    Gog(GogPlatform),
    Heroic(HeroicPlatform),
    Lutris(LutrisPlatform),
    Legendary(LegendaryPlatform),
}

impl PlatformEnum {
    pub fn load_platform<A: AsRef<str>, B: AsRef<str>>(name: A, settings_string: B) -> Self {
        todo!("Implement loading settings")
    }

    pub fn serialize_settings(&self) -> (String, String) {
        todo!("Implement saving settings");
    }

    pub fn name(&self) -> &str {
        match self {
            PlatformEnum::Amazon(_) => "Amazon",
            PlatformEnum::Bottles(_) => "Bottles",
            PlatformEnum::Epic(_) => "Epic",
            PlatformEnum::Uplay(_) => "Uplay",
            PlatformEnum::Itch(_) => "Itch",
            PlatformEnum::Flatpak(_) => "Flatpak",
            PlatformEnum::Origin(_) => "Origin",
            PlatformEnum::Gog(_) => "Gog",
            PlatformEnum::Heroic(_) => "Heroic",
            PlatformEnum::Lutris(_) => "Lutris",
            PlatformEnum::Legendary(_) => "Legendary",
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
            PlatformEnum::Origin(p) => p.settings.enabled,
            PlatformEnum::Gog(p) => p.settings.enabled,
            PlatformEnum::Heroic(p) => p.settings.enabled,
            PlatformEnum::Lutris(p) => p.settings.enabled,
            PlatformEnum::Legendary(p) => p.settings.enabled,
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
            PlatformEnum::Origin(p) => p.render_origin_settings(ui),
            PlatformEnum::Gog(p) => p.render_gog_settings(ui),
            PlatformEnum::Heroic(p) => p.render_heroic_settings(ui),
            PlatformEnum::Lutris(p) => p.render_lutris_settings(ui),
            PlatformEnum::Legendary(p) => p.render_legendary_settings(ui),
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
            PlatformEnum::Origin(p) => p.get_shortcut_info(),
            PlatformEnum::Gog(p) => p.get_shortcut_info(),
            PlatformEnum::Heroic(p) => p.get_shortcut_info(),
            PlatformEnum::Lutris(p) => p.get_shortcut_info(),
            PlatformEnum::Legendary(p) => p.get_shortcut_info(),
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

pub type Platforms = [PlatformEnum; 11];

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
        PlatformEnum::Origin(OriginPlatform {
            settings: settings.origin.clone(),
        }),
        PlatformEnum::Gog(GogPlatform {
            settings: settings.gog.clone(),
        }),
        PlatformEnum::Heroic(HeroicPlatform {
            settings: settings.heroic.clone(),
            heroic_games: None,
        }),
        PlatformEnum::Lutris(LutrisPlatform {
            settings: settings.lutris.clone(),
        }),
        PlatformEnum::Legendary(LegendaryPlatform {
            settings: settings.legendary.clone(),
        }),
    ]
}

pub trait NeedsPorton<P> {
    fn needs_proton(&self, platform: &P) -> bool;

    fn create_symlinks(&self, platform: &P) -> bool;
}
