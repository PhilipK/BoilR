#[cfg(target_family = "unix")]
mod bottles;
#[cfg(target_family = "unix")]
mod flatpak;
#[cfg(target_family = "unix")]
mod heroic;
#[cfg(target_family = "unix")]
mod legendary;
#[cfg(target_family = "unix")]
mod lutris;
#[cfg(target_family = "unix")]
mod minigalaxy;

#[cfg(not(target_family = "unix"))]
mod amazon;

#[cfg(not(target_family = "unix"))]
mod playnite;

#[cfg(not(target_family = "unix"))]
mod gamepass;




mod gog;
mod itch;
mod origin;
mod platform;
mod platforms_load;
mod uplay;

mod egs;
pub(crate) use platform::*;

#[cfg(target_family = "unix")]
pub(crate) use gog::get_gog_shortcuts_from_game_folders;
#[cfg(target_family = "unix")]
pub(crate) use gog::GogShortcut;


pub use platforms_load::get_platforms;
pub(crate) use platforms_load::load_settings;
pub(crate) use platforms_load::FromSettingsString;
pub use platforms_load::Platforms;
