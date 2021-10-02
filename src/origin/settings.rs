use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]

pub struct OriginSettings {
    pub enabled: bool,
    pub path: Option<String>,
}
