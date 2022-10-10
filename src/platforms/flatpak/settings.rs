use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FlatpakSettings {
    pub enabled: bool,
}

impl Default for FlatpakSettings {
    fn default() -> Self {
        #[cfg(target_family = "unix")]
        let enabled = true;

        #[cfg(not(target_family = "unix"))]
        let enabled = false;

        Self { enabled }
    }
}
