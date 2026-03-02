use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]

pub struct EASettings {
    pub enabled: bool,
}

impl Default for EASettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}
