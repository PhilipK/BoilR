use std::path::{Path, PathBuf};

use steam_shortcuts_util::shortcut::{Shortcut, ShortcutOwned};

#[derive(Clone)]
pub(crate) struct CustomGame {
    pub(crate) name: String,
    pub(crate) icon: String,
    pub(crate) executable: PathBuf,
}

impl From<CustomGame> for ShortcutOwned {
    fn from(game: CustomGame) -> Self {
        let exe = format!("\"{}\"", game.executable.to_string_lossy());
        let start_dir = game
            .executable
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_string_lossy()
            .to_string();
        Shortcut::new("0", &game.name, &exe, &start_dir, &game.icon, "", "").to_owned()
    }
}
