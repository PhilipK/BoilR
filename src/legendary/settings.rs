use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LegendarySettings {
    pub enabled: bool,
    pub executable: Option<String>,
}

impl Default for LegendarySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            executable: None,
        }
    }
}
