use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct SteamGridDbSettings {
    pub enabled: bool,
    pub auth_key: Option<String>,
    pub prefer_animated: bool,
}
