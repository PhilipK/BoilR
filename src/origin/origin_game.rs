use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};
pub struct OriginGame {
    pub id: String,
    pub title: String,
}

impl From<OriginGame> for ShortcutOwned {
    fn from(game: OriginGame) -> Self {
        let launch = format!("origin2://game/launch?offerIds={}&autoDownload=1", game.id);
        let shortcut = Shortcut::new(0, game.title.as_str(), launch.as_str(), "", "", "", "");
        let mut owned_shortcut = shortcut.to_owned();
        owned_shortcut.tags.push("Origin".to_owned());
        owned_shortcut.tags.push("Ready TO Play".to_owned());
        owned_shortcut.tags.push("Installed".to_owned());

        owned_shortcut
    }
}
