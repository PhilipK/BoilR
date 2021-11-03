use serde::{Serialize,Deserialize};

#[derive(Debug,Serialize, Deserialize)]
pub struct SteamGridDbSettings {
    pub enabled: bool,
    pub auth_key: Option<String>,
    pub prefer_animated:bool,
}
