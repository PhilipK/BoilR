use std::path::{Path, PathBuf};

use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

use crate::platforms::NeedsProton;

use super::StandalonePlatform;

#[derive(Debug, Clone)]
pub(crate) struct StandaloneGame {
    pub(crate) title: String,
    pub(crate) executable: PathBuf,
}

impl From<StandaloneGame> for ShortcutOwned {
    fn from(game: StandaloneGame) -> Self {
        let executable = quoted_path(&game.executable);
        let start_dir = game
            .executable
            .parent()
            .map(quoted_path)
            .unwrap_or_default();
        let icon = game.executable.to_string_lossy().to_string();

        let mut shortcut =
            Shortcut::new("0", &game.title, &executable, &start_dir, &icon, "", "").to_owned();
        shortcut.tags.push("Standalone".to_owned());
        shortcut.tags.push("Ready TO Play".to_owned());
        shortcut.tags.push("Installed".to_owned());
        shortcut
    }
}

impl NeedsProton<StandalonePlatform> for StandaloneGame {
    fn needs_proton(&self, _platform: &StandalonePlatform) -> bool {
        cfg!(target_family = "unix")
    }

    fn create_symlinks(&self, _platform: &StandalonePlatform) -> bool {
        false
    }
}

fn quoted_path(path: &Path) -> String {
    format!("\"{}\"", path.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platforms::NeedsProton;

    #[test]
    fn creates_direct_shortcut_with_expected_metadata() {
        let executable = PathBuf::from("C:/Games/My Game/My Game.exe");
        let game = StandaloneGame {
            title: "My Game".to_owned(),
            executable: executable.clone(),
        };

        let shortcut: ShortcutOwned = game.into();

        assert_eq!(shortcut.app_name, "My Game");
        assert_eq!(shortcut.exe, "\"C:/Games/My Game/My Game.exe\"");
        assert_eq!(shortcut.start_dir, "\"C:/Games/My Game\"");
        assert_eq!(shortcut.icon, executable.to_string_lossy());
        assert_eq!(shortcut.launch_options, "");
        assert!(shortcut.tags.contains(&"Standalone".to_owned()));
        assert!(shortcut.tags.contains(&"Ready TO Play".to_owned()));
        assert!(shortcut.tags.contains(&"Installed".to_owned()));
    }

    #[test]
    fn proton_requirement_matches_target_family() {
        let game = StandaloneGame {
            title: "Game".to_owned(),
            executable: PathBuf::from("Game.exe"),
        };
        let platform = StandalonePlatform::default();

        assert_eq!(game.needs_proton(&platform), cfg!(target_family = "unix"));
        assert!(!game.create_symlinks(&platform));
    }
}
