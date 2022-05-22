use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AmazonSettings {
    pub enabled: bool,
    pub launcher_location: Option<String>
}
