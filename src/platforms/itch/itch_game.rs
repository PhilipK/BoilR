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
        let mut exe_string = exe.to_string_lossy().to_string();
        if exe_string.contains(' ') && !exe_string.starts_with('\"') {
            exe_string = format!("\"{exe_string}\"");
        }

        let mut start_dir_string = game.install_path;
        if start_dir_string.contains(' ') && !start_dir_string.starts_with('\"') {
            start_dir_string = format!("\"{start_dir_string}\"");
        }

        let shortcut = Shortcut::new(
            "0",
            game.title.as_str(),
            exe_string.as_str(),
            start_dir_string.as_str(),
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
