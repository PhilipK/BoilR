use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ItchSettings {
    pub enabled: bool,
    pub location: Option<String>,
    #[cfg(target_family = "unix")]
    pub create_symlinks: bool,
}

impl Default for ItchSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            location: Default::default(),
            #[cfg(target_family = "unix")]
            create_symlinks: true,
        }
    }
}
