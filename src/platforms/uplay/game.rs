use std::path::{Path, PathBuf};

use steam_shortcuts_util::shortcut::{Shortcut, ShortcutOwned};

use crate::platforms::NeedsPorton;

use super::UplayPlatform;

#[derive(Clone)]
pub(crate) struct UplayGame {
    pub(crate) name: String,
    pub(crate) icon: String,
    pub(crate) id: String,
    pub(crate) launcher: PathBuf,
}

impl From<UplayGame> for ShortcutOwned {
    fn from(game: UplayGame) -> Self {
        let launch = format!("\"uplay://launch/{}/0\"", game.id);
        let start_dir = game
            .launcher
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_string_lossy();
        let exe = format!("\"{}\"", game.launcher.to_string_lossy());
        Shortcut::new("0", &game.name, &exe, &start_dir, &game.icon, "", &launch).to_owned()
    }
}

impl NeedsPorton<UplayPlatform> for UplayGame{
    fn needs_proton(&self, _platform: &UplayPlatform) -> bool {
        false
    }

    fn create_symlinks(&self, _platform: &UplayPlatform) -> bool {
        false
    }
}