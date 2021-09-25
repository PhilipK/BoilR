use serde::Deserialize;

#[derive(Debug, Deserialize,Clone)]
pub struct EpicGamesLauncherSettings {
    pub enabled: bool,
    pub location: Option<String>,
}

impl Default for EpicGamesLauncherSettings {
    fn default() -> Self {
        
        // On windows
        // Even if no path is given, we can try to guess, so lets enable by default
        #[cfg(target_os = "windows")]
        let enabled = true;


        // On linux
        // We can not guess, so lets not enable by default
        #[cfg(target_os = "linux")]
        let enabled = false;

        EpicGamesLauncherSettings {
            enabled,
            location: None,
        }
    }
}

