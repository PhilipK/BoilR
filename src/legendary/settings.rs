use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct LegendarySettings {
    pub enabled: bool,
    pub executable: Option<String>,
}