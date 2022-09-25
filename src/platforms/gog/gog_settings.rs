use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct GogSettings {
    pub enabled: bool,
    pub location: Option<String>,
    pub wine_c_drive: Option<String>,
    #[cfg(target_family = "unix")]
    pub create_symlinks: bool,
}
