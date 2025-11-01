use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct SteamSettings {
    pub location: Option<String>,
    pub create_collections: bool,
    pub optimize_for_big_picture: bool,
    pub stop_steam: bool,
    pub start_steam: bool,
}
