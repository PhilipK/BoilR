use std::collections::HashMap;

use serde::de::DeserializeOwned;
use steam_shortcuts_util::shortcut::ShortcutOwned;

use crate::settings::load_setting_sections;

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

const PLATFORM_NAMES: [&str; 11] = [
    "amazon",
    "bottles",
    "epic_games",
    "flatpak",
    "gog",
    "heroic",
    "itch",
    "legendary",
    "lutris",
    "origin",
    "uplay",
];

pub trait FromSettingsString {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self;
}

pub(crate) fn load_settings<Setting, S: AsRef<str>>(input: S) -> Setting
where
    Setting: Default,
    Setting: DeserializeOwned,
{
    let str = input.as_ref();
    match toml::from_str(str) {
        Ok(k) => k,
        Err(err) => {
            if !str.is_empty(){
                eprintln!("Error reading settings file {:?}", err);
            }
            Setting::default()
        }
    }
}

pub fn load_platform<A: AsRef<str>, B: AsRef<str>>(
    name: A,
    settings_string: B,
) -> eyre::Result<Box<dyn GamesPlatform>> {
    let name = name.as_ref();
    let s = settings_string.as_ref();
    match name {
        "amazon" => load::<AmazonPlatform>(s),
        "bottles" => load::<BottlesPlatform>(s),
        "epic_games" => load::<EpicPlatform>(s),
        "uplay" => load::<UplayPlatform>(s),
        "itch" => load::<ItchPlatform>(s),
        "flatpak" => load::<FlatpakPlatform>(s),
        "gog" => load::<GogPlatform>(s),
        "heroic" => load::<HeroicPlatform>(s),
        "legendary" => load::<LegendaryPlatform>(s),
        "lutris" => load::<LutrisPlatform>(s),
        "origin" => load::<OriginPlatform>(s),
        _ => Err(eyre::format_err!("Unknown platform named {name}")),
    }
}

fn load<T>(s: &str) -> eyre::Result<Box<dyn GamesPlatform>>
where
    T: FromSettingsString,
    T: GamesPlatform,
    T: 'static,
{
    Ok(Box::new(T::from_settings_string(s)))
}

pub struct ShortcutToImport {
    pub shortcut: ShortcutOwned,
    pub needs_proton: bool,
    pub needs_symlinks: bool,
}

pub(crate) fn to_shortcuts<T, P>(
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

pub(crate) fn to_shortcuts_simple<T>(
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

pub type Platforms = Vec<Box<dyn GamesPlatform>>;

pub fn get_platforms() -> Platforms {
    let sections = load_setting_sections();
    let sections = match sections {
        Ok(s) => s,
        Err(err) => {
            eprintln!(
                "Could not load platform settings, using defaults: Error: {:?}",
                err
            );
            HashMap::new()
        }
    };

    let mut platforms = vec![];
    for name in PLATFORM_NAMES {
        let default =String::from("");
        let settings = sections.get(name).unwrap_or(&default);
        match load_platform(name, settings) {
            Ok(platform) => platforms.push(platform),
            Err(e) => eprintln!("Could not load platform {name}, gave error: {e}"),
        }
    }
    platforms
}

pub trait NeedsPorton<P> {
    fn needs_proton(&self, platform: &P) -> bool;

    fn create_symlinks(&self, platform: &P) -> bool;
}

use dyn_clone::DynClone;

pub trait GamesPlatform
where
    Self: std::marker::Send,
    Self: std::marker::Sync,
    Self: DynClone,
{
    fn name(&self) -> &str;

    fn enabled(&self) -> bool;

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>>;

    fn render_ui(&mut self, ui: &mut egui::Ui);
}

dyn_clone::clone_trait_object!(GamesPlatform);
