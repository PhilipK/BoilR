use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GogSettings {
    pub enabled: bool,
    pub location: Option<String>,
    pub wine_c_drive: Option<String>,
    #[cfg(target_os = "linux")]
    pub create_symlinks: bool,
}
