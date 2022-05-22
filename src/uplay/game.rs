use std::path::{PathBuf, Path};

use steam_shortcuts_util::shortcut::{Shortcut, ShortcutOwned};

#[derive(Clone)]
pub(crate) struct Game {
    pub(crate) name: String,
    pub(crate) icon: String,
    pub(crate) id: String,
    pub(crate) launcher: PathBuf
}

impl From<Game> for ShortcutOwned {
    fn from(game: Game) -> Self {
        let launch = format!("\"uplay://launch/{}/0\"", game.id);
        let start_dir = game.launcher.parent().unwrap_or(Path::new("")).to_string_lossy();
        let exe = format!("\"{}\"",game.launcher.to_string_lossy());
        Shortcut::new("0", &game.name, &exe, &start_dir, &game.icon, "", &launch).to_owned()
    }
}
