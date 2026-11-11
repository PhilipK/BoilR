//! Installed EA games as the current EA app leaves them.
//!
//! The EA app no longer writes Origin's `.mfst` manifests (`%PROGRAMDATA%\Origin\LocalContent`).
//! What it still leaves is the game's own installer data: Windows' list of installed programs
//! holds the game's folder (`InstallLocation`), and that folder has
//! `__Installer\installerdata.xml`, whose `contentIDs` name the game in EA's catalog. Playnite's
//! EaLibrary checks installs against the same installer data.

use std::path::{Path, PathBuf};

/// One game installed through the EA app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledGame {
    pub title: String,
    /// The game's content ids, its own first. The EA app's launch link takes them.
    pub content_ids: Vec<String>,
    pub folder: PathBuf,
}

/// The file that marks a folder as an EA app install.
pub(crate) fn installer_data_file(folder: &Path) -> PathBuf {
    folder.join("__Installer").join("installerdata.xml")
}

/// The content ids an `installerdata.xml` lists, in order, without empty or repeated ones.
pub(crate) fn content_ids(xml: &str) -> Vec<String> {
    let Ok(document) = roxmltree::Document::parse(xml) else {
        return vec![];
    };
    let mut ids: Vec<String> = Vec::new();
    for node in document
        .descendants()
        .filter(|node| node.has_tag_name("contentID"))
    {
        let id = node.text().unwrap_or_default().trim();
        if !id.is_empty() && !ids.iter().any(|known| known == id) {
            ids.push(id.to_string());
        }
    }
    ids
}

/// A game's name as Windows lists it, without trademark signs.
pub(crate) fn clean_title(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '™' | '®' | '©'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The game in `folder`, when it is an EA app install. `display_name` is the name Windows
/// lists it under; the folder name stands in when there is none.
pub(crate) fn installed_game(folder: &Path, display_name: Option<&str>) -> Option<InstalledGame> {
    let xml = std::fs::read(installer_data_file(folder)).ok()?;
    let content_ids = content_ids(&String::from_utf8_lossy(&xml));
    if content_ids.is_empty() {
        return None;
    }
    let title = display_name
        .map(clean_title)
        .filter(|title| !title.is_empty())
        .or_else(|| {
            folder
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
        })?;
    Some(InstalledGame {
        title,
        content_ids,
        folder: folder.to_path_buf(),
    })
}

/// Every game Windows lists as installed whose folder holds EA installer data: the
/// machine-wide lists (64- and 32-bit) and the user's own.
#[cfg(windows)]
pub(crate) fn installed_games() -> Vec<InstalledGame> {
    use winreg::enums::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    };
    use winreg::RegKey;

    const UNINSTALL: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
    let lists = [
        (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_64KEY),
        (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_32KEY),
        (HKEY_CURRENT_USER, KEY_READ),
    ];
    let mut games: Vec<InstalledGame> = Vec::new();
    for (hive, flags) in lists {
        let Ok(uninstall) = RegKey::predef(hive).open_subkey_with_flags(UNINSTALL, flags) else {
            continue;
        };
        for name in uninstall.enum_keys().flatten() {
            let Ok(program) = uninstall.open_subkey_with_flags(&name, flags) else {
                continue;
            };
            let Ok(location) = program.get_value::<String, _>("InstallLocation") else {
                continue;
            };
            let location = location.trim().trim_matches('"');
            if location.is_empty() {
                continue;
            }
            let display_name = program.get_value::<String, _>("DisplayName").ok();
            let Some(game) = installed_game(Path::new(location), display_name.as_deref()) else {
                continue;
            };
            // The 64- and 32-bit lists can both hold a game.
            if !games
                .iter()
                .any(|known| known.content_ids == game.content_ids)
            {
                games.push(game);
            }
        }
    }
    games
}

/// The program a Windows `shell\open\command` runs: `"C:\..\EALauncher.exe" "%1"`, or the
/// same without quotes around the program (the EA app registers `origin2` that way).
pub(crate) fn command_program(command: &str) -> Option<PathBuf> {
    let command = command.trim();
    let program = match command.strip_prefix('"') {
        Some(quoted) => quoted.split('"').next()?,
        None => {
            let end = command.to_ascii_lowercase().find(".exe")? + ".exe".len();
            command.get(..end)?
        }
    };
    let program = program.trim();
    (!program.is_empty()).then(|| PathBuf::from(program))
}

/// The program that opens `origin2://` links, which start EA app games: the one Windows has
/// registered for them, else `EALauncher.exe` next to the EA app, else the EA app itself.
#[cfg(windows)]
pub(crate) fn link_handler(ea_app: &Path) -> PathBuf {
    use winreg::{enums::HKEY_CLASSES_ROOT, RegKey};

    RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey("origin2\\shell\\open\\command")
        .and_then(|key| key.get_value::<String, _>(""))
        .ok()
        .and_then(|command| command_program(&command))
        .into_iter()
        .chain(std::iter::once(ea_app.with_file_name("EALauncher.exe")))
        .find(|program| program.is_file())
        .unwrap_or_else(|| ea_app.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_program_of_a_shell_command() {
        assert_eq!(
            command_program(
                r#"C:\Program Files\Electronic Arts\EA Desktop\EA Desktop\EALauncher.exe "%1""#
            ),
            Some(PathBuf::from(
                r"C:\Program Files\Electronic Arts\EA Desktop\EA Desktop\EALauncher.exe"
            ))
        );
        assert_eq!(
            command_program(r#""C:\EA Desktop\EALauncher.EXE" "%1""#),
            Some(PathBuf::from(r"C:\EA Desktop\EALauncher.EXE"))
        );
        assert_eq!(command_program(""), None);
        assert_eq!(command_program("\"\" \"%1\""), None);
        assert_eq!(command_program("no program here"), None);
    }

    const ALICE: &str = r#"<?xml version='1.0' encoding='utf-8'?>
<DiPManifest version="4.0">
  <buildMetaData><gameVersion version="7.0.0.0" /></buildMetaData>
  <contentIDs>
    <contentID>alice2_dd</contentID>
    <contentID> alice2_dd </contentID>
    <contentID>1002139</contentID>
    <contentID></contentID>
  </contentIDs>
  <gameTitles><gameTitle locale="en_US">Alice: Madness Returns™</gameTitle></gameTitles>
</DiPManifest>"#;

    #[test]
    fn reads_the_content_ids_of_an_install() {
        assert_eq!(content_ids(ALICE), vec!["alice2_dd", "1002139"]);
        assert!(content_ids("<DiPManifest><contentIDs/></DiPManifest>").is_empty());
        assert!(content_ids("not xml").is_empty());
    }

    #[test]
    fn titles_lose_trademark_signs() {
        assert_eq!(
            clean_title("Alice: Madness Returns™"),
            "Alice: Madness Returns"
        );
        assert_eq!(clean_title("  The Sims™  4 "), "The Sims 4");
    }

    #[test]
    fn a_folder_with_installer_data_is_an_install() -> eyre::Result<()> {
        let root = std::env::temp_dir().join(format!("boilr-ea-install-{}", std::process::id()));
        let game = root.join("Alice Madness Returns");
        std::fs::create_dir_all(game.join("__Installer"))?;
        std::fs::write(installer_data_file(&game), ALICE)?;
        let empty = root.join("Not A Game");
        std::fs::create_dir_all(&empty)?;

        let found = installed_game(&game, Some("Alice: Madness Returns™"));
        let unnamed = installed_game(&game, None);
        let not_a_game = installed_game(&empty, Some("Something"));
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(
            found,
            Some(InstalledGame {
                title: "Alice: Madness Returns".to_string(),
                content_ids: vec!["alice2_dd".to_string(), "1002139".to_string()],
                folder: game.clone(),
            })
        );
        assert_eq!(
            unnamed.map(|g| g.title),
            Some("Alice Madness Returns".to_string())
        );
        assert_eq!(not_a_game, None);
        Ok(())
    }

    /// Lists the EA games installed on this computer: `cargo test -p boilr --lib
    /// installer_data -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads this computer's installed programs"]
    fn lists_the_games_installed_here() {
        let ea_app =
            Path::new(r"C:\Program Files\Electronic Arts\EA Desktop\EA Desktop\EADesktop.exe");
        println!("origin2 links open with {}", link_handler(ea_app).display());
        for game in installed_games() {
            println!(
                "{} {:?} {}",
                game.title,
                game.content_ids,
                game.folder.display()
            );
        }
    }
}
