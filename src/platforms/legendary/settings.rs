use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LegendarySettings {
    pub enabled: bool,
    pub executable: Option<String>,
}
