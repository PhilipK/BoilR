use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UplaySettings {
    pub enabled: bool,
}

impl Default for UplaySettings {
    fn default() -> Self {
        #[cfg(target_family = "unix")]
        let enabled = false;
        #[cfg(target_family = "windows")]
        let enabled = true;
        Self { enabled }
    }
}
