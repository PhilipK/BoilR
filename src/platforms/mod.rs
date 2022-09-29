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
mod platforms_load;

pub(crate) use platform::*;

pub(crate) use gog::get_gog_shortcuts_from_game_folders;
pub(crate) use gog::GogShortcut;
pub(crate) use platforms_load::FromSettingsString;
pub(crate) use platforms_load::load_settings;
pub use platforms_load::Platforms;
pub use platforms_load::get_platforms;
