mod amazon;
mod bottles;
mod egs;
mod flatpak;
mod gog;
mod heroic;
mod itch;
mod legendary;
mod lutris;
mod origin;
mod platform;
mod platforms_load;
mod uplay;
mod minigalaxy;

pub(crate) use platform::*;

pub(crate) use gog::get_gog_shortcuts_from_game_folders;
pub(crate) use gog::GogShortcut;
pub use platforms_load::get_platforms;
pub(crate) use platforms_load::load_settings;
pub(crate) use platforms_load::FromSettingsString;
pub use platforms_load::Platforms;
