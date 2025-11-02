mod epic_platform;
#[cfg(feature = "egui-ui")]
mod epic_ui;
mod get_manifests;
mod manifest_item;
mod paths;
mod settings;

pub use epic_platform::*;
use get_manifests::get_egs_manifests;
pub(crate) use manifest_item::*;
use paths::*;
