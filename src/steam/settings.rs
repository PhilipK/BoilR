use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct SteamSettings{
    pub location: Option<String>,
}