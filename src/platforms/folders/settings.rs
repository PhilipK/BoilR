use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FoldersSettings {
    pub enabled: bool,
    pub folders: Vec<String>,
}

impl Default for FoldersSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            folders: Default::default(),
        }
    }
}

