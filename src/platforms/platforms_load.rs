use serde::de::DeserializeOwned;
use std::collections::HashMap;

use super::GamesPlatform;

use crate::{settings::load_setting_sections, platforms::folders::platform::FoldersPlatform};
const PLATFORM_NAMES: [&str; 15] = [
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
    "minigalaxy",
    "playnite",
    "gamepass",
    "folders"
];

pub type Platforms = Vec<Box<dyn GamesPlatform>>;

pub fn load_platform<A: AsRef<str>, B: AsRef<str>>(
    name: A,
    settings_string: B,
) -> eyre::Result<Box<dyn GamesPlatform>> {
    let name = name.as_ref();
    let s = settings_string.as_ref();

    #[cfg(not(target_family = "unix"))]
    {
        //Windows only platforms
        use super::amazon::AmazonPlatform;
        use super::playnite::PlaynitePlatform;
        use super::gamepass::GamePassPlatForm;
        match name {
            "amazon" => return load::<AmazonPlatform>(s),
            "playnite" => return load::<PlaynitePlatform>(s),
            "gamepass" => return load::<GamePassPlatForm>(s),
            _ => {}
        }
    }

    #[cfg(target_family = "unix")]
    {
        use super::bottles::BottlesPlatform;
        use super::flatpak::FlatpakPlatform;
        use super::heroic::HeroicPlatform;
        use super::legendary::LegendaryPlatform;
        use super::lutris::LutrisPlatform;
        use super::minigalaxy::MiniGalaxyPlatform;
        //Linux only platforms
        match name {
            "bottles" => return load::<BottlesPlatform>(s),
            "flatpak" => return load::<FlatpakPlatform>(s),
            "minigalaxy" => return load::<MiniGalaxyPlatform>(s),
            "legendary" => return load::<LegendaryPlatform>(s),
            "lutris" => return load::<LutrisPlatform>(s),
            "heroic" => return load::<HeroicPlatform>(s),
            _ => {}
        }
    }

    //Common platforms
    use super::egs::EpicPlatform;
    use super::gog::GogPlatform;
    use super::itch::ItchPlatform;
    use super::origin::OriginPlatform;
    use super::uplay::UplayPlatform;

    match name {
        "epic_games" => load::<EpicPlatform>(s),
        "uplay" => load::<UplayPlatform>(s),
        "itch" => load::<ItchPlatform>(s),
        "gog" => load::<GogPlatform>(s),
        "origin" => load::<OriginPlatform>(s),
        "folders" => load::<FoldersPlatform>(s),
        _ => Err(eyre::format_err!("Unknown platform named {name}")),
    }
}

pub fn get_platforms() -> Platforms {
    let sections = load_setting_sections();
    let sections = match sections {
        Ok(s) => s,
        Err(err) => {
            eprintln!(
                "Could not load platform settings, using defaults: Error: {err:?}"
            );
            HashMap::new()
        }
    };

    let mut platforms = vec![];
    for name in PLATFORM_NAMES {
        let default = String::from("");
        let settings = sections.get(name).unwrap_or(&default);
        match load_platform(name, settings) {
            Ok(platform) => platforms.push(platform),
            Err(e) => eprintln!("Could not load platform {name}, gave error: {e}"),
        }
    }
    platforms
}

pub fn load_settings<Setting, S: AsRef<str>>(input: S) -> Setting
where
    Setting: Default,
    Setting: DeserializeOwned,
{
    let str = input.as_ref();
    match toml::from_str(str) {
        Ok(k) => k,
        Err(err) => {
            if !str.is_empty() {
                eprintln!("Error reading settings file {err:?}");
            }
            Setting::default()
        }
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

pub trait FromSettingsString {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self;
}
