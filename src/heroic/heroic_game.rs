use std::path::Path;

use serde::{Deserialize, Serialize};
use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeroicGame {
    pub app_name: String,
    pub can_run_offline: bool,
    pub title: String,
    pub is_dlc: bool,
    pub install_path: String,
    pub executable: String,
}

impl From<HeroicGame> for ShortcutOwned {
    fn from(game: HeroicGame) -> Self {

        let home_dir = std::env::var("HOME").unwrap_or("".to_string());
        let legendary = "/var/lib/flatpak/app/com.heroicgameslauncher.hgl/current/active/files/bin/heroic/resources/app.asar.unpacked/build/bin/linux/";
        let home = Path::new(&home_dir);
        let config_folder = home.join("/.var/app/com.heroicgameslauncher.hgl/config");

        let exe = format!("\"{}\\{}\"", game.install_path, game.executable);
        let launch = format!("env XDG_CONFIG_HOME={} {} launch {}", config_folder.as_os_str().to_string_lossy(), legendary, game.app_name);
        let mut start_dir = game.install_path.clone();
        if !game.install_path.starts_with('"') {
            start_dir = format!("\"{}\"", game.install_path);
        }
        let shortcut = Shortcut::new(
            0,
            game.title.as_str(),
            launch.as_str(),
            start_dir.as_str(),
            exe.as_str(),
            "",
            "",
        );
        let mut owned_shortcut = shortcut.to_owned();
        owned_shortcut.tags.push("Heroic".to_owned());
        owned_shortcut.tags.push("Ready TO Play".to_owned());
        owned_shortcut.tags.push("Installed".to_owned());

        owned_shortcut
    }
}
