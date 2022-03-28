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
    pub config_folder: Option<String>,
    pub legendary_location: Option<String>
}

impl From<HeroicGame> for ShortcutOwned {
    fn from(game: HeroicGame) -> Self {

        let legendary = game.legendary_location.unwrap_or("legendary".to_string());

        let icon = format!("\"{}\\{}\"", game.install_path, game.executable);
        let launch = match game.config_folder{
            Some(config_folder) => {
                format!("env XDG_CONFIG_HOME={} {}", config_folder, legendary)
            },
            None => {
                format!("{}", legendary)
            },
        };

        let launch_options = format!("launch {}",game.app_name);
        
        let shortcut = Shortcut::new(
            "0",
            game.title.as_str(),
            launch.as_str(),
            "",
            icon.as_str(),
            "",
            &launch_options.as_str()
        );
        let mut owned_shortcut = shortcut.to_owned();
        owned_shortcut.tags.push("Heroic".to_owned());
        owned_shortcut.tags.push("Ready TO Play".to_owned());
        owned_shortcut.tags.push("Installed".to_owned());

        owned_shortcut
    }
}
