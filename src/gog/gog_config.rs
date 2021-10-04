use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct GogConfig {
    #[serde(alias = "installationPaths")]
    pub installation_paths: Vec<String>,
}
