mod gog_config;
mod gog_game;
mod gog_platform;
mod gog_settings;

pub use gog_game::GogShortcut;
pub use gog_platform::get_gog_shortcuts_from_game_folders;
pub use gog_platform::GogPlatform;
pub(crate) use gog_settings::GogSettings;
