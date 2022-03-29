use std::path::Path;

use serde::{Deserialize, Serialize};
use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeroicGame {
    pub app_name: String,
    pub title: String,
    pub is_dlc: bool,
    pub install_path: String,
    pub executable: String,
    pub launch_parameters: String,
}

impl From<HeroicGame> for ShortcutOwned {
    fn from(game: HeroicGame) -> Self {
        let target_path = Path::new(&game.install_path).join(game.executable);
        let target = target_path.to_string_lossy();

        let shortcut = Shortcut::new(
            0,
            game.title.as_str(),
            &target,
            "",
            "",
            "",
            &game.launch_parameters.as_str(),
        );
        let mut owned_shortcut = shortcut.to_owned();
        owned_shortcut.tags.push("Heroic".to_owned());
        owned_shortcut.tags.push("Ready TO Play".to_owned());
        owned_shortcut.tags.push("Installed".to_owned());

        owned_shortcut
    }
}
