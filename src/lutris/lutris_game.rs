use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

#[derive(Clone)]
pub struct LutrisGame {
    pub index: String,
    pub name: String,
    pub id: String,
    pub platform: String,
}

impl From<LutrisGame> for ShortcutOwned {
    fn from(game: LutrisGame) -> Self {
        let options = format!("lutris:rungame/{}", game.id);
        Shortcut::new(
            "0",
            game.name.as_str(),
            "lutris",
            "",
            "",
            "",
            &options.as_str(),
        )
        .to_owned()
    }
}
