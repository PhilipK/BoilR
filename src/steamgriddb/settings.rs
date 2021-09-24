use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SteamGridDb {
    pub enabled: bool,
    pub auth_key: Option<String>,
}

impl Default for SteamGridDb {
    fn default() -> Self {
        Self {
            enabled: true,
            auth_key: None,
        }
    }
}
