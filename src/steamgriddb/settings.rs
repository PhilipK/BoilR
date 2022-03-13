use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SteamGridDbSettings {
    pub enabled: bool,
    pub auth_key: Option<String>,
    pub prefer_animated: bool,
    pub safe_download:bool
}
