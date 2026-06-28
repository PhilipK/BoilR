use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CustomSettings {
    pub enabled: bool,
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default, alias = "folder")]
    pub folder: String,
}

impl Default for CustomSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            folders: vec![],
            folder: String::new(),
        }
    }
}

impl CustomSettings {
    pub fn effective_folders(&self) -> Vec<String> {
        let mut result = self
            .folders
            .iter()
            .filter(|folder| !folder.trim().is_empty())
            .cloned()
            .collect::<Vec<_>>();

        if !self.folder.trim().is_empty() && !result.iter().any(|value| value == &self.folder) {
            result.push(self.folder.clone());
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::CustomSettings;

    #[test]
    fn keeps_legacy_single_folder_and_new_list_entries_together() {
        let mut settings = CustomSettings::default();
        settings.folder = "legacy".to_string();
        settings.folders.push("new".to_string());

        let folders = settings.effective_folders();

        assert_eq!(folders, vec!["new", "legacy"]);
    }
}
