use std::path::{Path, PathBuf};

use steam_shortcuts_util::shortcut::{Shortcut, ShortcutOwned};

#[derive(Clone)]
pub(crate) struct UplayGame {
    pub(crate) name: String,
    pub(crate) icon: String,
    pub(crate) id: String,
    pub(crate) launcher: PathBuf,
    pub(crate) launcher_compat_folder: Option<PathBuf>,
    pub(crate) launch_id: usize,
}

impl From<UplayGame> for ShortcutOwned {
    fn from(game: UplayGame) -> Self {
        let launch = match game.launcher_compat_folder{
            Some(compat_folder) => format!
            ("STEAM_COMPAT_DATA_PATH=\"{}\" %command% \"uplay://launch/{}/{}\"",
             compat_folder.to_string_lossy(), game.id, game.launch_id),
            None => format!("\"uplay://launch/{}/{}\"", game.id, game.launch_id)
        };
        let start_dir = game
            .launcher
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_string_lossy();
        let exe = format!("\"{}\"", game.launcher.to_string_lossy());
        Shortcut::new("0", &game.name, &exe, &start_dir, &game.icon, "", &launch).to_owned()
    }
}
