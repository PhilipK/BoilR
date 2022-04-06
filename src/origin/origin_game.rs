use std::path::PathBuf;

use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

#[derive(Clone)]
pub struct OriginGame {
    pub id: String,
    pub title: String,
    pub origin_location : Option<PathBuf>,
}

impl From<OriginGame> for ShortcutOwned {
    fn from(game: OriginGame) -> Self {
        let launch = format!("origin2://game/launch?offerIds={}&autoDownload=1", game.id);
        
        let mut owned_shortcut = if let Some(origin_location)  = game.origin_location{
            let origin_location = origin_location.to_string_lossy();
            Shortcut::new(
                "0", 
                game.title.as_str(), 
                &origin_location, 
                "", 
                "", 
                "", 
                launch.as_str()
            ).to_owned()
        }else{
            Shortcut::new(
                "0", 
                game.title.as_str(), 
                launch.as_str(), 
                "", 
                "", 
                "", 
                ""
            ).to_owned()
        };
        owned_shortcut.tags.push("Origin".to_owned());
        owned_shortcut.tags.push("Ready TO Play".to_owned());
        owned_shortcut.tags.push("Installed".to_owned());

        owned_shortcut
    }
}
