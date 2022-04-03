use steam_shortcuts_util::shortcut::ShortcutOwned;

use crate::gog::GogShortcut;
use super::HeroicGame;


#[derive(Clone)]
pub enum HeroicGameType{
    Epic(HeroicGame),
    //The bool is if it is windows (true) or not (false)
    Gog(GogShortcut,bool)
}


impl From<HeroicGameType> for ShortcutOwned {
    fn from(heroic_game_type: HeroicGameType) -> Self {
        match heroic_game_type{
            HeroicGameType::Epic(epic) => epic.into(),
            HeroicGameType::Gog(gog,_) => gog.into(),
        }
    }
}
