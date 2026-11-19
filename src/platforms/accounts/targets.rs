//! What a shortcut for an owned, not installed game runs: the store's own launcher with its
//! install link when that launcher is installed, otherwise the store page in the browser.
//! On Linux, where most stores have no launcher, Heroic is used for Epic, GOG and Amazon
//! games when it happens to be installed (never required).

use std::path::{Path, PathBuf};

use boilr_stores::{OwnedGame, StoreId};
use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

use super::InstallWith;

/// Launchers found on this machine.
#[derive(Clone, Debug, Default)]
pub(crate) struct Launchers {
    pub epic: Option<PathBuf>,
    pub galaxy: Option<PathBuf>,
    pub amazon: Option<PathBuf>,
    pub ea: Option<PathBuf>,
    pub battlenet: Option<PathBuf>,
    pub itch: Option<PathBuf>,
    pub heroic: Option<Heroic>,
    /// Stores Heroic is signed in to; Heroic can only install games of those.
    pub heroic_stores: Vec<StoreId>,
    /// `explorer.exe` on Windows, used to open store links.
    pub opener: Opener,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Heroic {
    Flatpak,
    Native(PathBuf),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Opener {
    /// `explorer.exe <link>`.
    Explorer(PathBuf),
    /// `xdg-open <link>`.
    XdgOpen,
}

impl Default for Opener {
    fn default() -> Self {
        if cfg!(windows) {
            let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
            Opener::Explorer(Path::new(&root).join("explorer.exe"))
        } else {
            Opener::XdgOpen
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    pub exe: String,
    pub start_dir: String,
    pub launch_options: String,
}

impl Launchers {
    pub fn detect() -> Self {
        Launchers {
            epic: detect_epic(),
            galaxy: detect_galaxy(),
            amazon: detect_amazon(),
            ea: detect_ea(),
            battlenet: detect_battlenet(),
            itch: detect_itch(),
            heroic: detect_heroic(),
            heroic_stores: detect_heroic_stores(),
            opener: Opener::default(),
        }
    }
}

pub(crate) fn owned_shortcut(
    game: &OwnedGame,
    install_with: InstallWith,
    launchers: &Launchers,
) -> ShortcutOwned {
    let target = target(game, install_with, launchers);
    let mut shortcut = Shortcut::new(
        "0",
        game.title.as_str(),
        &target.exe,
        &target.start_dir,
        "",
        "",
        &target.launch_options,
    )
    .to_owned();
    let identity = owned_identity(game);
    shortcut.app_id = crate::sync::owned_app_id(&identity);
    shortcut.dev_kit_game_id = format!("{}{identity}", crate::sync::IDENTITY_PREFIX);
    shortcut.tags.push(store_tag(game.store).to_owned());
    shortcut.tags.push("Not installed".to_owned());
    shortcut
}

/// The tag the store's installed-games platform gives its games, so a game keeps its tag
/// when it is installed or uninstalled.
fn store_tag(store: StoreId) -> &'static str {
    match store {
        StoreId::Epic => "EGS",
        StoreId::Gog => "Gog",
        StoreId::Itch => "Itch",
        StoreId::Ea => "Origin/EA Desktop",
        StoreId::Amazon | StoreId::BattleNet | StoreId::Humble => store.display_name(),
    }
}

/// `<store>:<id>`, plus the other ids an installed copy may report instead (an EA game's
/// manifest can name its content id rather than its offer id).
fn owned_identity(game: &OwnedGame) -> String {
    let mut ids = vec![game.id.as_str()];
    if game.store == StoreId::Ea {
        ids.extend(game.extra("content_id").filter(|c| !c.contains(',')));
    }
    format!("{}:{}", game.store.key(), ids.join(","))
}

pub(crate) fn target(game: &OwnedGame, install_with: InstallWith, launchers: &Launchers) -> Target {
    match install_with {
        InstallWith::Browser => browser(launchers, &store_page(game)),
        InstallWith::Heroic => heroic_target(game, launchers)
            .or_else(|| launcher_target(game, launchers))
            .unwrap_or_else(|| browser(launchers, &store_page(game))),
        InstallWith::Auto => launcher_target(game, launchers)
            .or_else(|| heroic_target(game, launchers))
            .unwrap_or_else(|| browser(launchers, &store_page(game))),
    }
}

/// The store's own launcher, opened on the game's install page or dialog.
fn launcher_target(game: &OwnedGame, launchers: &Launchers) -> Option<Target> {
    match game.store {
        StoreId::Epic => {
            let launcher = launchers.epic.as_ref()?;
            Some(Target {
                exe: quoted(launcher),
                start_dir: parent_dir(launcher),
                launch_options: epic_install_link(game),
            })
        }
        StoreId::Gog => {
            launchers.galaxy.as_ref()?;
            Some(open_link(
                launchers,
                &format!("goggalaxy://openGameView/{}", game.id),
            ))
        }
        StoreId::Amazon => {
            launchers.amazon.as_ref()?;
            Some(open_link(
                launchers,
                &format!("amazon-games://install/{}", game.id),
            ))
        }
        StoreId::Ea => {
            launchers.ea.as_ref()?;
            Some(open_link(launchers, &ea_install_link(game)))
        }
        StoreId::BattleNet => {
            let launcher = launchers.battlenet.as_ref()?;
            // Games the launcher cannot install (classic CD-key games, titles BoilR does
            // not know yet) have no internal id.
            let internal_id = game.extra("internal_id")?;
            Some(Target {
                exe: quoted(launcher),
                start_dir: parent_dir(launcher),
                launch_options: format!("--game={internal_id}"),
            })
        }
        StoreId::Itch => {
            launchers.itch.as_ref()?;
            Some(open_link(
                launchers,
                &format!("itch://install?game_id={}&launch", game.id),
            ))
        }
        StoreId::Humble => None,
    }
}

/// Playnite's launch link (EpicLibrary `EpicLauncher.cs`), the one BoilR writes for
/// installed games too. Playnite avoids `?action=install`: Epic shows an old, buggy install
/// dialog for it. Without catalog ids, the library page is the safe fallback.
fn epic_install_link(game: &OwnedGame) -> String {
    match (game.extra("namespace"), game.extra("catalog_item_id")) {
        (Some(namespace), Some(catalog_item_id)) => format!(
            "com.epicgames.launcher://apps/{namespace}%3A{catalog_item_id}%3A{}?action=launch&silent=true",
            game.id
        ),
        _ => "com.epicgames.launcher://store/library".to_string(),
    }
}

/// The EA app (and Origin before it) opens its install dialog for this link when the game
/// is not installed (Jeshibu's EaLibrary, `EaControllerHelper.cs` and `EaInstallController.cs`).
fn ea_install_link(game: &OwnedGame) -> String {
    let id = game.extra("content_id").unwrap_or(&game.id);
    format!("origin2://game/launch/?offerIds={id}")
}

/// Heroic handles Epic (Legendary), GOG and Amazon (Nile) games; `heroic://launch` asks to
/// install a game that is not installed.
fn heroic_target(game: &OwnedGame, launchers: &Launchers) -> Option<Target> {
    let runner = match game.store {
        StoreId::Epic => "legendary",
        StoreId::Gog => "gog",
        StoreId::Amazon => "nile",
        _ => return None,
    };
    if !launchers.heroic_stores.contains(&game.store) {
        return None;
    }
    // With the runner, unlike the link of an installed Heroic game
    // (`heroic://launch/<app>`): Heroic has to know which store to install from.
    let link = format!("heroic://launch/{runner}/{}", game.id);
    match launchers.heroic.as_ref()? {
        Heroic::Flatpak => Some(Target {
            exe: "flatpak".to_string(),
            start_dir: String::new(),
            launch_options: format!("run com.heroicgameslauncher.hgl {link} --no-gui --no-sandbox"),
        }),
        Heroic::Native(path) => Some(Target {
            exe: if path.is_absolute() {
                quoted(path)
            } else {
                path.to_string_lossy().to_string()
            },
            start_dir: String::new(),
            launch_options: link,
        }),
    }
}

/// What the shortcut of a game that is not installed does, for display ("Opens GOG Galaxy
/// to install it"), from the shortcut's target as [`target`] writes it.
pub fn describe_install_target(exe: &str, launch_options: &str) -> String {
    let link = launch_options.trim().trim_matches('"');
    if launch_options.contains("heroic://") {
        return "Opens Heroic to install it".to_string();
    }
    if let Some(rest) = link.strip_prefix("https://") {
        let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
        return format!("Opens its store page on {host}");
    }
    let launcher = match link.split_once("://").map(|(scheme, _)| scheme) {
        Some("com.epicgames.launcher") => "the Epic Games Launcher".to_string(),
        Some("goggalaxy") => "GOG Galaxy".to_string(),
        Some("amazon-games") => "Amazon Games".to_string(),
        Some("origin2") => "the EA app".to_string(),
        Some("itch") => "the itch app".to_string(),
        Some(_) => "the store's app".to_string(),
        None => Some(launcher_name(exe))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "the store's app".to_string()),
    };
    format!("Opens {launcher} to install it")
}

/// The launcher's name from its path: the file name without extension and " Launcher".
/// Splits on both `/` and `\`, because `Path` only knows the separator of the system it
/// runs on, and the path can be a Windows one read on Linux (or the other way round).
fn launcher_name(exe: &str) -> String {
    let file_name = exe
        .trim()
        .trim_matches('"')
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default();
    let stem = match file_name.rsplit_once('.') {
        Some((stem, _extension)) if !stem.is_empty() => stem,
        _ => file_name,
    };
    stem.trim_end_matches(" Launcher").to_string()
}

fn browser(launchers: &Launchers, url: &str) -> Target {
    open_link(launchers, url)
}

fn open_link(launchers: &Launchers, link: &str) -> Target {
    match &launchers.opener {
        // Steam on Windows fails to start a shortcut with an empty StartDir (OS error 123).
        Opener::Explorer(explorer) => Target {
            exe: quoted(explorer),
            start_dir: parent_dir(explorer),
            launch_options: format!("\"{link}\""),
        },
        Opener::XdgOpen => Target {
            exe: "xdg-open".to_string(),
            start_dir: String::new(),
            launch_options: format!("\"{link}\""),
        },
    }
}

/// A page where the user can get the game: the store module's `url` when it has one.
pub(crate) fn store_page(game: &OwnedGame) -> String {
    if let Some(url) = game.extra("url").filter(|url| url.starts_with("https://")) {
        return url.to_string();
    }
    let slug = game.extra("slug").filter(|slug| !slug.is_empty());
    match game.store {
        StoreId::Epic => match game.extra("product_slug").or(slug) {
            Some(slug) => format!("https://store.epicgames.com/p/{slug}"),
            None => format!(
                "https://store.epicgames.com/browse?q={}",
                url::form_urlencoded::byte_serialize(game.title.as_bytes()).collect::<String>()
            ),
        },
        StoreId::Gog => match slug {
            Some(slug) => format!("https://www.gog.com/game/{slug}"),
            None => "https://www.gog.com/account".to_string(),
        },
        StoreId::Amazon => "https://gaming.amazon.com/home".to_string(),
        StoreId::Ea => "https://www.ea.com/ea-app".to_string(),
        StoreId::BattleNet => "https://download.battle.net/".to_string(),
        StoreId::Humble => "https://www.humblebundle.com/home/library".to_string(),
        StoreId::Itch => format!("https://itch.io/game/{}", game.id),
    }
}

fn quoted(path: &Path) -> String {
    format!("\"{}\"", path.to_string_lossy())
}

fn parent_dir(path: &Path) -> String {
    path.parent().map(quoted).unwrap_or_default()
}

fn first_existing(paths: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    paths.into_iter().find(|path| path.is_file())
}

fn env_path(var: &str, rest: &[&str]) -> Option<PathBuf> {
    let base = std::env::var_os(var)?;
    let mut path = PathBuf::from(base);
    for part in rest {
        path.push(part);
    }
    Some(path)
}

fn detect_epic() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let mut candidates = vec![];
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        for arch in ["Win64", "Win32"] {
            candidates.extend(env_path(
                var,
                &[
                    "Epic Games",
                    "Launcher",
                    "Portal",
                    "Binaries",
                    arch,
                    "EpicGamesLauncher.exe",
                ],
            ));
        }
    }
    first_existing(candidates)
}

fn detect_galaxy() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    first_existing(
        ["ProgramFiles(x86)", "ProgramFiles"]
            .into_iter()
            .filter_map(|var| env_path(var, &["GOG Galaxy", "GalaxyClient.exe"])),
    )
}

fn detect_amazon() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    first_existing(env_path(
        "LOCALAPPDATA",
        &["Amazon Games", "App", "Amazon Games.exe"],
    ))
}

fn detect_ea() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let mut candidates = vec![];
    for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
        candidates.extend(env_path(
            var,
            &[
                "Electronic Arts",
                "EA Desktop",
                "EA Desktop",
                "EADesktop.exe",
            ],
        ));
        candidates.extend(env_path(var, &["Origin", "Origin.exe"]));
    }
    first_existing(candidates)
}

fn detect_battlenet() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    first_existing(
        ["ProgramFiles(x86)", "ProgramFiles"]
            .into_iter()
            .filter_map(|var| env_path(var, &["Battle.net", "Battle.net.exe"])),
    )
}

fn detect_itch() -> Option<PathBuf> {
    if cfg!(windows) {
        first_existing(env_path("LOCALAPPDATA", &["itch", "itch.exe"]))
    } else {
        let home = std::env::var_os("HOME")?;
        let home = Path::new(&home);
        first_existing([
            home.join(".itch").join("itch"),
            home.join(".local")
                .join("share")
                .join("flatpak")
                .join("exports")
                .join("bin")
                .join("io.itch.itch"),
            PathBuf::from("/var/lib/flatpak/exports/bin/io.itch.itch"),
        ])
    }
}

fn detect_heroic() -> Option<Heroic> {
    if cfg!(windows) {
        return None;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let installed_natively =
        find_in_path("heroic").or_else(|| first_existing([PathBuf::from("/opt/Heroic/heroic")]));
    heroic_install(installed_natively, |path| path.exists(), home.as_deref())
}

/// Which Heroic is there. `exists` tells whether a path is there, so tests need no files.
///
/// Inside BoilR's own Flatpak neither the host's `PATH` nor `/var/lib/flatpak` (where a
/// system-wide Heroic Flatpak lives, the usual place on SteamOS) can be seen. What can be
/// seen is the folder a Flatpak app keeps its data in, `~/.var/app/<id>`: it is there once
/// the Heroic Flatpak has run, however it was installed.
fn heroic_install(
    native: Option<PathBuf>,
    exists: impl Fn(&Path) -> bool,
    home: Option<&Path>,
) -> Option<Heroic> {
    if let Some(path) = native {
        return Some(Heroic::Native(path));
    }
    const FLATPAK_ID: &str = "com.heroicgameslauncher.hgl";
    let mut flatpak_signs = vec![PathBuf::from("/var/lib/flatpak/app").join(FLATPAK_ID)];
    if let Some(home) = home {
        flatpak_signs.push(home.join(".local/share/flatpak/app").join(FLATPAK_ID));
        flatpak_signs.push(home.join(".var/app").join(FLATPAK_ID));
    }
    if flatpak_signs.iter().any(|path| exists(path)) {
        return Some(Heroic::Flatpak);
    }
    // A native Heroic that BoilR's Flatpak cannot find in the host's `PATH` still shows by
    // its config folder; Steam runs the shortcut on the host, where `heroic` is found.
    let native_config = home.map(|home| home.join(".config/heroic"));
    native_config
        .filter(|path| exists(path))
        .map(|_| Heroic::Native(PathBuf::from("heroic")))
}

/// Heroic keeps a sign-in file per store (Legendary, GOG, Nile), natively or in its Flatpak.
fn detect_heroic_stores() -> Vec<StoreId> {
    if cfg!(windows) {
        return vec![];
    }
    let Some(home) = std::env::var_os("HOME") else {
        return vec![];
    };
    let home = Path::new(&home);
    let config_dirs = [
        home.join(".config").join("heroic"),
        home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic"),
    ];
    let sign_in_files = [
        (StoreId::Epic, "legendaryConfig/legendary/user.json"),
        (StoreId::Gog, "gog_store/auth.json"),
        (StoreId::Amazon, "nile_config/nile/user.json"),
    ];
    sign_in_files
        .into_iter()
        .filter(|(_, file)| config_dirs.iter().any(|dir| dir.join(file).is_file()))
        .map(|(store, _)| store)
        .collect()
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(store: StoreId, id: &str) -> OwnedGame {
        OwnedGame::new(store, id, "Some Game")
    }

    fn linux() -> Launchers {
        Launchers {
            opener: Opener::XdgOpen,
            ..Default::default()
        }
    }

    fn windows() -> Launchers {
        Launchers {
            opener: Opener::Explorer(PathBuf::from("C:\\Windows\\explorer.exe")),
            ..Default::default()
        }
    }

    #[test]
    fn without_launchers_opens_the_store_page() {
        let gog = game(StoreId::Gog, "1207658924").with_extra("slug", "unreal_gold");
        let target = target(&gog, InstallWith::Auto, &linux());
        assert_eq!(target.exe, "xdg-open");
        assert_eq!(
            target.launch_options,
            "\"https://www.gog.com/game/unreal_gold\""
        );
    }

    #[test]
    fn one_title_in_two_stores_gets_two_app_ids() {
        let gog = OwnedGame::new(StoreId::Gog, "1454315831", "Fallout 3");
        let epic = OwnedGame::new(
            StoreId::Epic,
            "adeae8bbfc94427db57c7dfecce3f1d4",
            "Fallout 3",
        );
        let gog = owned_shortcut(&gog, InstallWith::Auto, &linux());
        let epic = owned_shortcut(&epic, InstallWith::Auto, &linux());
        assert_eq!(gog.exe, epic.exe);
        assert_ne!(gog.app_id, epic.app_id);
    }

    #[test]
    fn ea_identity_names_the_content_id_too() {
        let ea = OwnedGame::new(StoreId::Ea, "OFB-EAST:52735", "Zuma's Revenge")
            .with_extra("content_id", "1002139");
        let shortcut = owned_shortcut(&ea, InstallWith::Auto, &linux());
        assert_eq!(shortcut.dev_kit_game_id, "boilr:ea:OFB-EAST:52735,1002139");
    }

    #[test]
    fn epic_launcher_gets_the_install_link() {
        let epic = game(StoreId::Epic, "Fortnite")
            .with_extra("namespace", "fn")
            .with_extra("catalog_item_id", "4fe75bbc5a674f4f9b356b5c90567da5");
        let launchers = Launchers {
            epic: Some(PathBuf::from("C:\\Epic\\EpicGamesLauncher.exe")),
            ..windows()
        };
        let target = target(&epic, InstallWith::Auto, &launchers);
        assert_eq!(target.exe, "\"C:\\Epic\\EpicGamesLauncher.exe\"");
        assert_eq!(
            target.launch_options,
            "com.epicgames.launcher://apps/fn%3A4fe75bbc5a674f4f9b356b5c90567da5%3AFortnite?action=launch&silent=true"
        );
    }

    #[test]
    fn heroic_is_used_on_linux_when_present() {
        let launchers = Launchers {
            heroic: Some(Heroic::Flatpak),
            heroic_stores: vec![StoreId::Amazon],
            ..linux()
        };
        // Heroic is not signed in to GOG, so GOG games open the store page.
        assert_eq!(
            target(&game(StoreId::Gog, "1"), InstallWith::Heroic, &launchers).exe,
            "xdg-open"
        );
        let target = target(
            &game(StoreId::Amazon, "amzn1.adg.product.x"),
            InstallWith::Auto,
            &launchers,
        );
        assert_eq!(target.exe, "flatpak");
        assert!(target
            .launch_options
            .contains("heroic://launch/nile/amzn1.adg.product.x"));
        // Stores Heroic doesn't handle fall back to the browser.
        let humble = game(StoreId::Humble, "game_name")
            .with_extra("url", "https://www.humblebundle.com/downloads?key=abc");
        let target = super::target(&humble, InstallWith::Heroic, &launchers);
        assert_eq!(
            target.launch_options,
            "\"https://www.humblebundle.com/downloads?key=abc\""
        );
    }

    #[test]
    fn heroic_is_found_from_inside_a_flatpak() {
        let home = Path::new("/home/deck");
        let only = |there: &'static str| move |path: &Path| path == Path::new(there);
        // A system-wide Heroic Flatpak, seen from BoilR's own Flatpak: only its data folder.
        assert_eq!(
            heroic_install(
                None,
                only("/home/deck/.var/app/com.heroicgameslauncher.hgl"),
                Some(home)
            ),
            Some(Heroic::Flatpak)
        );
        // Seen from the host.
        assert_eq!(
            heroic_install(
                None,
                only("/var/lib/flatpak/app/com.heroicgameslauncher.hgl"),
                Some(home)
            ),
            Some(Heroic::Flatpak)
        );
        // A native Heroic whose binary cannot be seen runs by name on the host.
        assert_eq!(
            heroic_install(None, only("/home/deck/.config/heroic"), Some(home)),
            Some(Heroic::Native(PathBuf::from("heroic")))
        );
        assert_eq!(
            heroic_install(
                Some(PathBuf::from("/usr/bin/heroic")),
                |_: &Path| true,
                Some(home)
            ),
            Some(Heroic::Native(PathBuf::from("/usr/bin/heroic")))
        );
        assert_eq!(heroic_install(None, |_: &Path| false, Some(home)), None);

        let launchers = Launchers {
            heroic: Some(Heroic::Native(PathBuf::from("heroic"))),
            heroic_stores: vec![StoreId::Gog],
            ..linux()
        };
        assert_eq!(
            target(&game(StoreId::Gog, "1"), InstallWith::Auto, &launchers).exe,
            "heroic"
        );
    }

    #[test]
    fn browser_setting_skips_launchers() {
        let launchers = Launchers {
            galaxy: Some(PathBuf::from("C:\\GOG Galaxy\\GalaxyClient.exe")),
            ..windows()
        };
        let gog = game(StoreId::Gog, "1");
        assert!(target(&gog, InstallWith::Auto, &launchers)
            .launch_options
            .contains("goggalaxy://openGameView/1"));
        let in_browser = target(&gog, InstallWith::Browser, &launchers);
        assert_eq!(in_browser.launch_options, "\"https://www.gog.com/account\"");
        // Only Windows reads `C:\` paths as folders.
        #[cfg(windows)]
        assert_eq!(in_browser.start_dir, "\"C:\\Windows\"");
    }

    #[test]
    fn ea_and_battlenet_use_their_launchers() {
        let launchers = Launchers {
            ea: Some(PathBuf::from("C:\\EA\\EADesktop.exe")),
            battlenet: Some(PathBuf::from("C:\\Battle.net\\Battle.net.exe")),
            ..windows()
        };
        let ea = game(StoreId::Ea, "Origin.OFR.50.0002694").with_extra("content_id", "1234");
        assert_eq!(
            target(&ea, InstallWith::Auto, &launchers).launch_options,
            "\"origin2://game/launch/?offerIds=1234\""
        );
        let overwatch = game(StoreId::BattleNet, "Pro").with_extra("internal_id", "prometheus");
        assert_eq!(
            target(&overwatch, InstallWith::Auto, &launchers).launch_options,
            "--game=prometheus"
        );
        let classic = game(StoreId::BattleNet, "W3");
        assert_eq!(
            target(&classic, InstallWith::Auto, &launchers).launch_options,
            "\"https://download.battle.net/\""
        );
    }

    #[test]
    fn launcher_names_come_from_windows_and_unix_paths_alike() {
        assert_eq!(
            launcher_name(r#""C:\Battle.net\Battle.net Launcher.exe""#),
            "Battle.net"
        );
        assert_eq!(launcher_name("/usr/bin/heroic"), "heroic");
        assert_eq!(launcher_name("C:/Games/Epic.exe"), "Epic");
        assert_eq!(launcher_name(""), "");
    }

    #[test]
    fn describes_what_the_shortcut_does() {
        let launchers = Launchers {
            epic: Some(PathBuf::from("C:\\Epic\\EpicGamesLauncher.exe")),
            galaxy: Some(PathBuf::from("C:\\GOG Galaxy\\GalaxyClient.exe")),
            battlenet: Some(PathBuf::from("C:\\Battle.net\\Battle.net Launcher.exe")),
            ..windows()
        };
        let describe = |game: &OwnedGame, launchers: &Launchers| {
            let target = target(game, InstallWith::Auto, launchers);
            describe_install_target(&target.exe, &target.launch_options)
        };
        assert_eq!(
            describe(&game(StoreId::Epic, "Fortnite"), &launchers),
            "Opens the Epic Games Launcher to install it"
        );
        assert_eq!(
            describe(&game(StoreId::Gog, "1"), &launchers),
            "Opens GOG Galaxy to install it"
        );
        assert_eq!(
            describe(
                &game(StoreId::BattleNet, "Pro").with_extra("internal_id", "prometheus"),
                &launchers
            ),
            "Opens Battle.net to install it"
        );
        assert_eq!(
            describe(&game(StoreId::Itch, "42"), &linux()),
            "Opens its store page on itch.io"
        );
        let heroic = Launchers {
            heroic: Some(Heroic::Flatpak),
            heroic_stores: vec![StoreId::Gog],
            ..linux()
        };
        assert_eq!(
            describe(&game(StoreId::Gog, "1"), &heroic),
            "Opens Heroic to install it"
        );
    }

    #[test]
    fn shortcut_carries_identity_and_tags() {
        let shortcut = owned_shortcut(&game(StoreId::Itch, "42"), InstallWith::Auto, &linux());
        assert_eq!(shortcut.dev_kit_game_id, "boilr:itch:42");
        assert_eq!(shortcut.tags, vec!["Itch", "Not installed"]);
        assert_eq!(shortcut.app_name, "Some Game");
    }
}
