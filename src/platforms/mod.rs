mod platform;
mod egs;
mod amazon;
mod bottles;
mod uplay;
mod itch;
mod flatpak;

pub use platform::*;

pub use amazon::AmazonSettings;
pub use egs::EpicGamesLauncherSettings;
pub use bottles::BottlesSettings;
pub use uplay::UplaySettings;
pub use itch::ItchSettings;
pub use flatpak::FlatpakSettings;