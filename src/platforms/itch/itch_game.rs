use std::path::Path;

use serde::{Deserialize, Serialize};
use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

use crate::platforms::NeedsProton;

use super::ItchPlatform;

#[derive(Serialize, Deserialize, Debug, Clone)]

pub struct ItchGame {
    pub install_path: String,
    pub executable: String,
    pub title: String,
}

impl From<ItchGame> for ShortcutOwned {
    fn from(game: ItchGame) -> Self {
        let exe = Path::new(&game.install_path).join(&game.executable);
        let exe = exe.to_string_lossy().to_string();
        let shortcut = Shortcut::new(
            "0",
            game.title.as_str(),
            exe.as_str(),
            &game.install_path,
            "",
            "",
            "",
        );

        let mut owned_shortcut = shortcut.to_owned();
        owned_shortcut.tags.push("Itch".to_owned());
        owned_shortcut.tags.push("Ready TO Play".to_owned());
        owned_shortcut.tags.push("Installed".to_owned());

        owned_shortcut
    }
}

impl NeedsProton<ItchPlatform> for ItchGame {
    fn needs_proton(&self, _platform: &ItchPlatform) -> bool {
        self.executable.ends_with("exe")
    }

    #[cfg(target_family = "unix")]
    fn create_symlinks(&self, platform: &ItchPlatform) -> bool {
        platform.settings.create_symlinks
    }

    #[cfg(not(target_family = "unix"))]
    fn create_symlinks(&self, _platform: &ItchPlatform) -> bool {
        false
    }
}
