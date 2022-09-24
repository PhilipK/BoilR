mod platform;
mod egs;
mod amazon;
mod bottles;
mod uplay;
mod itch;
mod flatpak;
mod origin;
mod gog;
mod heroic;
mod lutris;
mod legendary;

pub use platform::*;

pub use amazon::AmazonSettings;
pub use egs::EpicGamesLauncherSettings;
pub use bottles::BottlesSettings;
pub use uplay::UplaySettings;
pub use itch::ItchSettings;
pub use flatpak::FlatpakSettings;
pub use origin::OriginSettings;

pub use gog::GogSettings;
pub use gog::get_gog_shortcuts_from_game_folders;
pub use gog::GogShortcut;

pub use heroic::HeroicSettings;
pub use lutris::LutrisSettings;
pub use legendary::LegendarySettings;
