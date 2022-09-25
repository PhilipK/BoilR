use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, Clone)]

pub struct OriginSettings {
    pub enabled: bool,
}
