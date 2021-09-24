use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct EpicGamesLauncher {
    enabled: bool,
    location: Option<String>,
}

impl Default for EpicGamesLauncher {
    fn default() -> Self {
        
        // On windows
        // Even if no path is given, we can try to guess, so lets enable by default
        #[cfg(target_os = "windows")]
        let enabled = true;


        // On linux
        // We can notguess, so lets not enable by default
        #[cfg(target_os = "linux")]
        let enabled = false;

        EpicGamesLauncher {
            enabled,
            location: None,
        }
    }
}

