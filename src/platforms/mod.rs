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

pub use gog::get_gog_shortcuts_from_game_folders;
pub use gog::GogShortcut;

