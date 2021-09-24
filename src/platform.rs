use crate::{egs::EpicGamesLauncherSettings, legendary::LegendarySettings};

pub enum Platform{
    EpicGames{
        settings: EpicGamesLauncherSettings,
    },
    Legendary{
        settings: LegendarySettings
    }
}



