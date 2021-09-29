use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct ItchSettings {
    pub enabled: bool,
    pub location: Option<String>,
}