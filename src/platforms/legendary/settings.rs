use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LegendarySettings {
    pub enabled: bool,
    pub executable: Option<String>,
}

impl Default for LegendarySettings {
    fn default() -> Self {
        #[cfg(target_family = "unix")]
        let enabled = true;
        #[cfg(target_family = "windows")]
        let enabled = false;
        Self {
            enabled,
            executable: Default::default(),
        }
    }
}
