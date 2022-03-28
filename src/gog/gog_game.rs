use std::path::Path;

use serde::{Deserialize, Serialize};
use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct GogGame {
    pub name: String,
    #[serde(alias = "gameId")]
    pub game_id: String,
    #[serde(alias = "playTasks")]
    pub play_tasks: Option<Vec<PlayTask>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct PlayTask {
    pub category: Option<String>,
    #[serde(alias = "isPrimary")]
    pub is_primary: Option<bool>,
    pub name: Option<String>,
    pub path: Option<String>,
    #[serde(alias = "type")]
    pub task_type: String,
    #[serde(alias = "workingDir")]
    pub working_dir: Option<String>,
}

pub(crate) struct GogShortcut {
    pub name: String,
    pub game_folder: String,
    pub path: String,
    pub working_dir: String,
    pub game_id: String,
}

impl From<GogShortcut> for ShortcutOwned {
    fn from(gogs: GogShortcut) -> Self {
        let exe = Path::new(&gogs.game_folder).join(gogs.path);
        let icon_file = format!("goggame-{}.ico", gogs.game_id);
        let icon_path = Path::new(&gogs.game_folder).join(&icon_file);
        let icon = if icon_path.exists() {
            icon_path.to_str().unwrap().to_string()
        } else {
            exe.to_str().unwrap_or("").to_string()
        };
        let shortcut = Shortcut::new(
            "0",
            gogs.name.as_str(),
            exe.to_str().unwrap(),
            gogs.working_dir.as_str(),
            icon.as_str(),
            "",
            "",
        );
        let mut owned_shortcut = shortcut.to_owned();
        owned_shortcut.tags.push("Gog".to_owned());
        owned_shortcut.tags.push("Ready TO Play".to_owned());
        owned_shortcut.tags.push("Installed".to_owned());

        owned_shortcut
    }
}
