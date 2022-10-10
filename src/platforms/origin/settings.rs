use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]

pub struct OriginSettings {
    pub enabled: bool,
}

impl Default for OriginSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}
