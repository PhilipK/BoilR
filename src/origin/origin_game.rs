use std::path::PathBuf;

use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

#[derive(Clone)]
pub struct OriginGame {
    pub id: String,
    pub title: String,
    pub origin_location: Option<PathBuf>,
    pub origin_compat_folder: Option<PathBuf>,
}

impl From<OriginGame> for ShortcutOwned {
    fn from(game: OriginGame) -> Self {
        let launch = match game.origin_compat_folder{
            Some(compat_folder) => 
                format!("STEAM_COMPAT_DATA_PATH=\"{}\" %command% \"origin2://game/launch?offerIds={}&autoDownload=1&authCode=&cmdParams=\"", compat_folder.to_string_lossy().to_string(), game.id)
            ,
            None => format!(
            "\"origin2://game/launch?offerIds={}&autoDownload=1&authCode=&cmdParams=\"",
            game.id)
        };        
        let mut owned_shortcut = if let Some(origin_location) = game.origin_location {
            let origin_location = format!("\"{}\"", origin_location.to_string_lossy());
            Shortcut::new(
                "0",
                game.title.as_str(),
                &origin_location,
                "",
                "",
                "",
                launch.as_str(),
            )
            .to_owned()
        } else {
            Shortcut::new("0", game.title.as_str(), launch.as_str(), "", "", "", "").to_owned()
        };
        owned_shortcut.tags.push("Origin".to_owned());
        owned_shortcut.tags.push("Ready TO Play".to_owned());
        owned_shortcut.tags.push("Installed".to_owned());

        owned_shortcut
    }
}
