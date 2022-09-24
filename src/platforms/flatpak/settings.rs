use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FlatpakSettings {
    pub enabled: bool,
}
