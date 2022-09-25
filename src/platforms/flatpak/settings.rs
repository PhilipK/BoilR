use serde::{Deserialize, Serialize};

#[derive(Debug, Default,Deserialize, Serialize, Clone)]
pub struct FlatpakSettings {
    pub enabled: bool,
}
