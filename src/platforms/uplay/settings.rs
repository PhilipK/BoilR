use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UplaySettings {
    pub enabled: bool,
}
