use std::path::PathBuf;

use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

#[derive(Clone)]
pub struct EAGame {
    pub id: String,
    pub title: String,
    pub launcher_location: PathBuf,
    pub launcher_compat_folder: Option<PathBuf>,
}

fn make_launch_url(id: &str, launcher_location: &std::path::Path) -> String {
    // EA App (EA Desktop) uses link2ea:// — Origin used origin2://
    // Detect which is installed by the exe name so both old and new installs work
    let use_ea_app = launcher_location
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_ascii_lowercase().contains("eadesktop"))
        .unwrap_or(false);

    if use_ea_app {
        // EA App (2022+): link2ea://launchgame/{id}
        format!("link2ea://launchgame/{id}")
    } else {
        // Legacy Origin: origin2://game/launch?offerIds={id}&autoDownload=1&authCode=&cmdParams=
        format!("origin2://game/launch?offerIds={id}&autoDownload=1&authCode=&cmdParams=")
    }
}

impl From<EAGame> for ShortcutOwned {
    fn from(game: EAGame) -> Self {
        let url = make_launch_url(&game.id, &game.launcher_location);
        let launch = match game.launcher_compat_folder {
            Some(compat_folder) => format!(
                "STEAM_COMPAT_DATA_PATH=\"{}\" %command% \"{url}\"",
                compat_folder.to_string_lossy()
            ),
            None => format!("\"{url}\""),
        };
        let launcher_location = format!("\"{}\"", game.launcher_location.to_string_lossy());
        let mut owned_shortcut = Shortcut::new(
            "0",
            game.title.as_str(),
            &launcher_location,
            "",
            "",
            "",
            launch.as_str(),
        )
        .to_owned();
        owned_shortcut.tags.push("Origin/EA Desktop".to_owned());
        owned_shortcut.tags.push("Ready TO Play".to_owned());
        owned_shortcut.tags.push("Installed".to_owned());

        owned_shortcut
    }
}
