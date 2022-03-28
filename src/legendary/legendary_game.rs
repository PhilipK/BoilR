use serde::{Deserialize, Serialize};
use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LegendaryGame {
    pub app_name: String,
    pub can_run_offline: bool,
    pub title: String,
    pub is_dlc: bool,
    pub install_path: String,
    pub executable: String,
}

impl From<LegendaryGame> for ShortcutOwned {
    fn from(game: LegendaryGame) -> Self {
        let exe = format!("\"{}\\{}\"", game.install_path, game.executable);
        let launch = format!("legendary launch {}", game.app_name);
        let mut start_dir = game.install_path.clone();
        if !game.install_path.starts_with('"') {
            start_dir = format!("\"{}\"", game.install_path);
        }
        let shortcut = Shortcut::new(
            "0",
            game.title.as_str(),
            launch.as_str(),
            start_dir.as_str(),
            exe.as_str(),
            "",
            "",
        );
        let mut owned_shortcut = shortcut.to_owned();
        owned_shortcut.tags.push("Legendary".to_owned());
        owned_shortcut.tags.push("Ready TO Play".to_owned());
        owned_shortcut.tags.push("Installed".to_owned());

        owned_shortcut
    }
}
