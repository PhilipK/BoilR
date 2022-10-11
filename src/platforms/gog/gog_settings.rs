use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GogSettings {
    pub enabled: bool,
    pub location: Option<String>,
    pub wine_c_drive: Option<String>,
    #[cfg(target_family = "unix")]
    pub create_symlinks: bool,
}

impl Default for GogSettings {
    fn default() -> Self {
        Self {
            #[cfg(target_family = "unix")]
            enabled: false,
            #[cfg(not(target_family = "unix"))]
            enabled: true,
            location: None,
            wine_c_drive: None,
            #[cfg(target_family = "unix")]
            create_symlinks: true,
        }
    }
}
