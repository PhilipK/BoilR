mod epic_platform;
mod get_manifests;
mod manifest_item;
mod paths;
mod settings;
mod epic_ui;

pub use epic_platform::*;
use get_manifests::get_egs_manifests;
pub(crate) use manifest_item::*;
pub use paths::*;
pub use settings::EpicGamesLauncherSettings;
