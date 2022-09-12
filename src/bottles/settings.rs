use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BottlesSettings {
    pub enabled: bool,
}
