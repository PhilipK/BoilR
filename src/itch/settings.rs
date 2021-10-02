use serde::{Serialize,Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ItchSettings {
    pub enabled: bool,
    pub location: Option<String>,
}