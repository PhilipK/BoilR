mod epic_platform;
mod get_manifests;
mod manifest_item;
mod settings;

pub use epic_platform::*;
pub use get_manifests::get_default_location;
use get_manifests::get_egs_manifests;
pub(crate) use manifest_item::*;
pub use settings::EpicGamesLauncherSettings;
