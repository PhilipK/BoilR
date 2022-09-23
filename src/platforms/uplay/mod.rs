mod game;
mod platform;
mod settings;

pub use platform::UplayPlatform;
pub use settings::UplaySettings;
pub(crate) use platform::get_uplay_games;
