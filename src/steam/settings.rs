use serde::{Serialize,Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SteamSettings{
    pub location: Option<String>,
}