mod get_manifests;
mod manifest_item;
mod settings;
mod epic_platform;

pub use manifest_item::*;
use get_manifests::get_egs_manifests;
pub use settings::EpicGamesLauncherSettings;
pub use epic_platform::*;