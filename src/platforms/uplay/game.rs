use std::path::{Path, PathBuf};

use boilr_core::launch_and_wait::{LaunchAndWait, SettingLine};
use steam_shortcuts_util::shortcut::{Shortcut, ShortcutOwned};

#[derive(Clone)]
pub(crate) struct UbisoftGame {
    pub(crate) name: String,
    pub(crate) icon: String,
    pub(crate) id: String,
    pub(crate) launcher: PathBuf,
    pub(crate) launcher_compat_folder: Option<PathBuf>,
    pub(crate) launch_id: usize,
    /// The game's install folder (the registry's `InstallDir`), when known.
    pub(crate) game_folder: Option<PathBuf>,
    /// The BoilR the shortcut runs to wait for the game, see
    /// [`boilr_core::launch_and_wait`]. Ubisoft Connect hands the game over and keeps
    /// running, so Steam would otherwise count the game as running until Ubisoft Connect
    /// closes, or (when it was already open) as stopped at once.
    pub(crate) helper: Option<PathBuf>,
    /// Close an open Ubisoft Connect before the game too, so Steam starts it and its overlay
    /// reaches the game ([`super::settings::UbisoftSettings::steam_overlay`]).
    pub(crate) restart_launcher: bool,
    /// Ubisoft Connect's `settings.yaml`, whose `closebehavior` BoilR changes while it has
    /// Ubisoft Connect running, so that a close request quits it instead of hiding it.
    pub(crate) settings_file: Option<PathBuf>,
}

/// The title of Ubisoft Connect's window: a close request to it quits Ubisoft Connect while
/// `closebehavior` is [`CLOSE_QUITS`].
const UBISOFT_CONNECT_WINDOW: &str = "Ubisoft Connect";
/// Ubisoft Connect's default: its window's close button hides it in the tray.
const CLOSE_HIDES: &str = "closebehavior: CloseBehavior_Minimize";
/// The setting under which closing its window quits Ubisoft Connect.
const CLOSE_QUITS: &str = "closebehavior: CloseBehavior_Close";

impl From<UbisoftGame> for ShortcutOwned {
    fn from(game: UbisoftGame) -> Self {
        let link = format!("uplay://launch/{}/{}", game.id, game.launch_id);
        if let (Some(helper), Some(game_folder), None) = (
            &game.helper,
            &game.game_folder,
            &game.launcher_compat_folder,
        ) {
            // A Ubisoft Connect this launch started is asked to quit after the game: a close
            // request quits it while `closebehavior` says so, which BoilR sets while Ubisoft
            // Connect is closed and sets back once it has exited. It logs no end of its cloud
            // sync, so BoilR waits a fixed time first.
            let request = LaunchAndWait {
                launcher: game.launcher.clone(),
                link,
                game_folder: game_folder.clone(),
                restart_launcher: game.restart_launcher,
                close_window: Some(UBISOFT_CONNECT_WINDOW.to_string()),
                setting: game.settings_file.as_ref().map(|file| SettingLine {
                    file: file.clone(),
                    from: CLOSE_HIDES.to_string(),
                    to: CLOSE_QUITS.to_string(),
                }),
                ..LaunchAndWait::default()
            };
            let target = request.shortcut_target(helper);
            return Shortcut::new(
                "0",
                &game.name,
                &target.exe,
                &target.start_dir,
                &game.icon,
                "",
                &target.launch_options,
            )
            .to_owned();
        }
        let launch = match game.launcher_compat_folder {
            Some(compat_folder) => format!(
                "STEAM_COMPAT_DATA_PATH=\"{}\" %command% \"{link}\"",
                compat_folder.to_string_lossy(),
            ),
            None => format!("\"{link}\""),
        };
        let start_dir = game
            .launcher
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_string_lossy();
        let exe = format!("\"{}\"", game.launcher.to_string_lossy());
        Shortcut::new("0", &game.name, &exe, &start_dir, &game.icon, "", &launch).to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn far_cry_3() -> UbisoftGame {
        UbisoftGame {
            name: "Far Cry 3".to_string(),
            icon: String::new(),
            id: "46".to_string(),
            launcher: PathBuf::from(r"C:\Ubisoft Game Launcher\upc.exe"),
            launcher_compat_folder: None,
            launch_id: 0,
            game_folder: Some(PathBuf::from(r"D:\UplayLibrary\Far Cry 3\")),
            helper: Some(PathBuf::from(r"C:\BoilR\BoilR.exe")),
            restart_launcher: true,
            settings_file: Some(PathBuf::from(
                r"C:\Users\me\Ubisoft Game Launcher\settings.yaml",
            )),
        }
    }

    #[test]
    fn runs_boilr_to_wait_for_the_game() {
        let shortcut: ShortcutOwned = far_cry_3().into();
        assert_eq!(shortcut.exe, r#""C:\BoilR\BoilR.exe""#);
        assert_eq!(
            shortcut.launch_options,
            r#"--launch-and-wait --game-folder "D:\UplayLibrary\Far Cry 3" --launcher "C:\Ubisoft Game Launcher\upc.exe" --link "uplay://launch/46/0" --close-window "Ubisoft Connect" --setting-file "C:\Users\me\Ubisoft Game Launcher\settings.yaml" --setting-from "closebehavior: CloseBehavior_Minimize" --setting-to "closebehavior: CloseBehavior_Close" --restart-launcher"#
        );
    }

    #[test]
    fn without_a_folder_opens_the_link_in_ubisoft_connect() {
        let game = UbisoftGame {
            game_folder: None,
            ..far_cry_3()
        };
        let shortcut: ShortcutOwned = game.into();
        assert_eq!(shortcut.exe, r#""C:\Ubisoft Game Launcher\upc.exe""#);
        assert_eq!(shortcut.launch_options, r#""uplay://launch/46/0""#);
    }
}
