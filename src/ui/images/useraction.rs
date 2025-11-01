use boilr_core::{steam::SteamUsersInfo, steamgriddb::ImageType};

use super::{gamemode::GameMode, gametype::GameType, possible_image::PossibleImage};

#[derive(Debug)]
pub enum UserAction {
    CorrectGridId,
    UserSelected(SteamUsersInfo),
    ShortcutSelected(GameType),
    ImageTypeSelected(ImageType),
    ImageTypeCleared(ImageType, bool),
    ImageSelected(PossibleImage),
    GridIdChanged(usize),
    SetGamesMode(GameMode),
    BackButton,
    NoAction,
    ClearImages,
    DownloadAllImages,
    RefreshImages,
}
