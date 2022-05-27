use std::path::{PathBuf, Path};

use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

#[derive(Debug, Clone)]
pub struct AmazonGame {
    pub title: String,
    pub id: String,
    pub launcher_path:PathBuf,
}

impl From<AmazonGame> for ShortcutOwned {
    fn from(game: AmazonGame) -> Self {
        let launch = format!("amazon-games://play/{}", game.id);
        let exe = game.launcher_path.to_string_lossy().to_string();
        let start_dir= game.launcher_path.parent().unwrap_or_else(||Path::new("")).to_string_lossy().to_string();
        Shortcut::new("0", game.title.as_str(), exe.as_str(), start_dir.as_str(), "", "", launch.as_str()).to_owned()
    }
}
