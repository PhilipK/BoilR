use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

#[derive(Debug, Clone)]
pub struct AmazonGame{
    pub title : String,
    pub id : String
}



impl From<AmazonGame> for ShortcutOwned {
    fn from(game: AmazonGame) -> Self {
        let launch = format!("amazon-games://play/{}", game.id);
        Shortcut::new(
            "0",
            game.title.as_str(),
            launch.as_str(),
            "",
            "",
            "",
            "",
        )
        .to_owned()
    }
}
