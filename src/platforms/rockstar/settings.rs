use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RockstarSettings {
    pub enabled: bool,
}

impl Default for RockstarSettings {
    fn default() -> Self {
        #[cfg(target_family = "unix")]
        let enabled = false;
        #[cfg(target_os = "windows")]
        let enabled = true;
        Self { enabled }
    }
}
