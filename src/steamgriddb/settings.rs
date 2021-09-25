use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SteamGridDbSettings {
    pub enabled: bool,
    pub auth_key: Option<String>,
}