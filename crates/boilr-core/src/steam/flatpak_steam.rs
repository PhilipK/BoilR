use std::path::{Component, Path};

use steam_shortcuts_util::{app_id_generator::calculate_app_id, shortcut::ShortcutOwned};

use super::{get_steam_path, SteamSettings};

const STEAM_FLATPAK_ID: &str = "com.valvesoftware.Steam";

/// True when the configured Steam is the Flatpak one (`~/.var/app/com.valvesoftware.Steam`).
/// Symlinks are resolved first, so a `~/.steam` link into the Flatpak folder counts (#324).
pub fn is_flatpak_steam(settings: &SteamSettings) -> bool {
    let Ok(steam_path) = get_steam_path(settings) else {
        return false;
    };
    let path = Path::new(&steam_path);
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    is_flatpak_steam_path(&resolved)
}

fn is_flatpak_steam_path(path: &Path) -> bool {
    let names: Vec<&std::ffi::OsStr> = path
        .components()
        .filter_map(|c| match c {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    names
        .windows(3)
        .any(|w| matches!(w, [a, b, c] if *a == ".var" && *b == "app" && *c == STEAM_FLATPAK_ID))
}

/// A Flatpak Steam runs shortcuts inside its own sandbox, where `flatpak` can't start
/// other apps. Rewrites `flatpak run ...` shortcuts to `flatpak-spawn --host flatpak run ...`
/// and recalculates the app id, since Steam derives it from the exe.
pub fn run_flatpak_on_host(shortcut: &mut ShortcutOwned) {
    if shortcut.exe.trim_matches('"') != "flatpak" {
        return;
    }
    shortcut.exe = "flatpak-spawn".to_string();
    shortcut.launch_options = format!("--host flatpak {}", shortcut.launch_options)
        .trim_end()
        .to_string();
    shortcut.app_id = calculate_app_id(&shortcut.exe, &shortcut.app_name);
}

#[cfg(test)]
mod tests {
    use super::*;
    use steam_shortcuts_util::Shortcut;

    #[test]
    fn detects_flatpak_steam_folder() {
        assert!(is_flatpak_steam_path(Path::new(
            "/home/deck/.var/app/com.valvesoftware.Steam/.steam/steam"
        )));
        assert!(!is_flatpak_steam_path(Path::new("/home/deck/.steam/steam")));
        assert!(!is_flatpak_steam_path(Path::new(
            "/home/deck/.var/app/io.github.philipk.boilr/data"
        )));
    }

    #[test]
    fn rewrites_flatpak_shortcut() {
        let mut shortcut = Shortcut::new(
            "0",
            "Game",
            "flatpak",
            "",
            "",
            "",
            "run net.lutris.Lutris lutris:rungame/game",
        )
        .to_owned();
        let old_id = shortcut.app_id;
        run_flatpak_on_host(&mut shortcut);
        assert_eq!(shortcut.exe, "flatpak-spawn");
        assert_eq!(
            shortcut.launch_options,
            "--host flatpak run net.lutris.Lutris lutris:rungame/game"
        );
        assert_ne!(shortcut.app_id, old_id);
        assert_eq!(shortcut.app_id, calculate_app_id("flatpak-spawn", "Game"));
    }

    #[test]
    fn leaves_other_shortcuts_alone() {
        let mut shortcut = Shortcut::new(
            "0",
            "Game",
            "\"/usr/bin/lutris\"",
            "",
            "",
            "",
            "lutris:game",
        )
        .to_owned();
        let before = shortcut.clone();
        run_flatpak_on_host(&mut shortcut);
        assert_eq!(shortcut.exe, before.exe);
        assert_eq!(shortcut.launch_options, before.launch_options);
        assert_eq!(shortcut.app_id, before.app_id);
    }
}
