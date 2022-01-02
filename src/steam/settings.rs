use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SteamSettings {
    pub location: Option<String>,
}
