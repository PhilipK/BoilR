use steam_shortcuts_util::shortcut::{Shortcut, ShortcutOwned};

pub(crate) struct Game {
    pub(crate) name: String,
    pub(crate) icon: String,
    pub(crate) id: String,
}

impl From<Game> for ShortcutOwned {
    fn from(game: Game) -> Self {
        let launch = format!("uplay://launch/{}", game.id);
        Shortcut::new(0, &game.name, &launch, "", &game.icon, "", "").to_owned()
    }
}
