use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LutrisSettings {
    pub enabled: bool,
    pub executable: String,
    pub flatpak: bool,
    pub flatpak_image: String,
}
