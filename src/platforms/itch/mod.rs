mod butler_db_parser;
mod itch_game;
mod itch_platform;
mod receipt;
mod settings;

pub use itch_platform::ItchPlatform;
pub use settings::ItchSettings;
pub(crate) use itch_game::ItchGame;