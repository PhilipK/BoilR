use crate::{steam::SteamUsersInfo, steamgriddb::ImageType};

use super::{gametype::GameType, possible_image::PossibleImage, gamemode::GameMode};


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