use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ItchSettings {
    pub enabled: bool,
    pub location: Option<String>,
    #[cfg(target_family = "unix")]
    pub create_symlinks: bool,
}
