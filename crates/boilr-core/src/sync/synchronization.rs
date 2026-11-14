use steam_shortcuts_util::{
    app_id_generator::calculate_app_id, shortcut::ShortcutOwned, shortcuts_to_bytes,
};
use tokio::sync::watch::Sender;

use crate::{
    settings::Settings,
    steam::{
        get_shortcuts_for_user, get_shortcuts_paths, is_flatpak_steam, run_flatpak_on_host,
        write_collections, Collection, ShortcutInfo, SteamUsersInfo,
    },
    steamgriddb::{download_images_for_users, ImageType},
};

use std::{
    collections::{HashMap, HashSet},
    error::Error,
};

use std::{fs::File, io::Write, path::Path};

pub const BOILR_TAG: &str = "boilr";

#[derive(Clone, Debug)]
pub enum SyncProgress {
    NotStarted,
    Starting,
    FoundGames {
        games_found: usize,
    },
    FindingImages,
    DownloadingImages {
        to_download: usize,
    },
    Done,
    /// Error occurred during sync - contains user-friendly error message
    Error {
        message: String,
    },
}

pub fn disconnect_shortcut(settings: &Settings, app_id: u32) -> Result<(), String> {
    let mut userinfo_shortcuts = get_shortcuts_paths(&settings.steam)
        .map_err(|e| format!("Getting shortcut paths failed: {e}"))?;

    for user in userinfo_shortcuts.iter_mut() {
        let shortcut_info = get_shortcuts_for_user(user);
        if let Ok(mut shortcut_info) = shortcut_info {
            for shortcut in shortcut_info.shortcuts.iter_mut() {
                if shortcut.app_id == app_id {
                    shortcut.dev_kit_game_id = "".to_string();
                    shortcut.tags.retain(|s| s != BOILR_TAG);
                }
            }
            save_shortcuts(&shortcut_info.shortcuts, Path::new(&shortcut_info.path))?
        }
    }

    Ok(())
}

/// Prefix of a shortcut identity stored in `DevkitGameID`: `boilr:<store>:<store id>`.
/// Shortcuts without an identity carry the bare [`BOILR_TAG`].
pub const IDENTITY_PREFIX: &str = "boilr:";

/// The identity of a game in a store, for `DevkitGameID`. The same game gets the same
/// identity whether it is installed or only owned, so a sync can keep its Steam app id
/// (and with it artwork, playtime and collections) when the shortcut's target changes.
pub fn shortcut_identity(store: &str, id: &str) -> String {
    format!("{IDENTITY_PREFIX}{store}:{id}")
}

/// The identity of a shortcut (`<store>:<id>`), when it has one.
pub fn identity_of(shortcut: &ShortcutOwned) -> Option<&str> {
    shortcut
        .dev_kit_game_id
        .strip_prefix(IDENTITY_PREFIX)
        .filter(|rest| !rest.is_empty())
}

/// An identity can name several ids of one game, comma-separated (`ea:<offer>,<content id>`,
/// or every offer an installed EA game's manifest lists). The first id is the game's own;
/// the others are ids the same game may be known by elsewhere. This is the identity with
/// only the first id, which keeps the app id stable when the other ids change.
pub fn identity_key(identity: &str) -> &str {
    let Some((store, ids)) = identity.split_once(':') else {
        return identity;
    };
    match ids.find(',') {
        Some(comma) => identity.get(..store.len() + 1 + comma).unwrap_or(identity),
        None => identity,
    }
}

/// Every `<store>:<id>` an identity names. Two identities are the same game when these
/// overlap.
pub fn identity_ids(identity: &str) -> Vec<String> {
    let Some((store, ids)) = identity.split_once(':') else {
        return vec![identity.to_string()];
    };
    ids.split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| format!("{store}:{id}"))
        .collect()
}

/// What [`owned_app_id`] hashes next to the identity, in place of an exe.
const OWNED_APP_ID_SEED: &str = "boilr-owned";

/// The app id of a shortcut for a game that is not installed: a hash of its identity alone
/// (of its first id, see [`identity_key`]). These shortcuts share their exe (`xdg-open`, a
/// store launcher) and often their name (one game owned in two stores), so the usual
/// `exe + name` hash would give two games one app id. And their exe changes when the user
/// installs a launcher or picks another way to open them, while the id must stay: BoilR's
/// hidden games and renames are keyed by it. Steam reads the app id from `shortcuts.vdf`.
pub fn owned_app_id(identity: &str) -> u32 {
    calculate_app_id(OWNED_APP_ID_SEED, identity_key(identity))
}

/// Whether the shortcut's app id was made by [`owned_app_id`]. Such an id stays as it is
/// when the shortcut's exe or name changes.
pub fn has_owned_app_id(shortcut: &ShortcutOwned) -> bool {
    identity_of(shortcut).is_some_and(|identity| shortcut.app_id == owned_app_id(identity))
}

/// The app id a shortcut has once its exe or name changed: the same for a game that is not
/// installed, the `exe + name` hash for every other.
pub fn app_id_after_change(shortcut: &ShortcutOwned) -> u32 {
    if has_owned_app_id(shortcut) {
        shortcut.app_id
    } else {
        calculate_app_id(&shortcut.exe, &shortcut.app_name)
    }
}

/// The user's own name for a game. Renames are keyed by the app id the platform reports. A
/// game with an identity is also found under the id it had while it was not installed, so
/// a rename made then still applies once the game is installed.
pub fn rename_for<'a>(
    shortcut: &ShortcutOwned,
    renames: &'a HashMap<u32, String>,
) -> Option<&'a String> {
    renames
        .get(&shortcut.app_id)
        .or_else(|| identity_of(shortcut).and_then(|identity| renames.get(&owned_app_id(identity))))
}

pub fn sync_shortcuts(
    settings: &Settings,
    platform_shortcuts: &[(String, Vec<ShortcutOwned>)],
    sender: &mut Option<Sender<SyncProgress>>,
    renames: &HashMap<u32, String>,
) -> eyre::Result<Vec<SteamUsersInfo>> {
    sync_shortcuts_with_collections(settings, platform_shortcuts, &[], sender, renames)
}

/// Like [`sync_shortcuts`], with extra Steam collections (for example "Not installed").
/// `extra_collections` lists app ids as the platforms reported them; they are mapped to
/// the ids the shortcuts end up with, like the per-platform collections.
pub fn sync_shortcuts_with_collections(
    settings: &Settings,
    platform_shortcuts: &[(String, Vec<ShortcutOwned>)],
    extra_collections: &[(String, Vec<u32>)],
    sender: &mut Option<Sender<SyncProgress>>,
    renames: &HashMap<u32, String>,
) -> eyre::Result<Vec<SteamUsersInfo>> {
    let userinfo_shortcuts = get_shortcuts_paths(&settings.steam)?;
    let flatpak_steam = is_flatpak_steam(&settings.steam);
    if flatpak_steam {
        println!(
            "Steam is a Flatpak; launching Flatpak apps through flatpak-spawn --host. \
             Steam needs: flatpak override --user --talk-name=org.freedesktop.Flatpak com.valvesoftware.Steam"
        );
    }
    let mut all_shortcuts: Vec<ShortcutOwned> = platform_shortcuts
        .iter()
        .flat_map(|s| s.1.clone())
        .filter(|s| !settings.blacklisted_games.contains(&s.app_id))
        .collect();
    for shortcut in &mut all_shortcuts {
        if identity_of(shortcut).is_none() {
            shortcut.dev_kit_game_id = BOILR_TAG.to_string();
        }
    }
    if let Some(sender) = &sender {
        let _ = sender.send(SyncProgress::FoundGames {
            games_found: all_shortcuts.len(),
        });
    }
    // The app id each platform reported, in the same order as `all_shortcuts`. Collections
    // are built from these and mapped to the final ids per user.
    let platform_ids: Vec<u32> = all_shortcuts.iter().map(|s| s.app_id).collect();
    let mut previous_ids = HashMap::new();
    for shortcut in &mut all_shortcuts {
        // Renames are keyed by the platform's app id, so look up before the Flatpak rewrite.
        let rename = rename_for(shortcut, renames);
        // Whether this is a game that is not installed, whose id stays, is told from the id
        // the platform gave it: decide before anything changes.
        let owned = has_owned_app_id(shortcut);
        if flatpak_steam {
            let id_before = shortcut.app_id;
            run_flatpak_on_host(shortcut);
            if !owned {
                shortcut.app_id = calculate_app_id(&shortcut.exe, &shortcut.app_name);
            }
            if shortcut.app_id != id_before {
                previous_ids.insert(shortcut.app_id, id_before);
            }
        }
        if let Some(rename) = rename {
            let original_id = shortcut.app_id;
            shortcut.app_name = rename.clone();
            if !owned {
                shortcut.app_id = calculate_app_id(&shortcut.exe, &shortcut.app_name);
            }
            if shortcut.app_id != original_id {
                previous_ids.insert(shortcut.app_id, original_id);
            }
        }
    }
    let mut collections: Vec<(String, Vec<u32>)> = platform_shortcuts
        .iter()
        .map(|(name, shortcuts)| (name.clone(), shortcuts.iter().map(|s| s.app_id).collect()))
        .collect();
    collections.extend(extra_collections.iter().cloned());
    println!("Found {} user(s)", userinfo_shortcuts.len());
    // Read every user's shortcuts before writing any. A file that can't be read is not
    // skipped: the user would see an import that "worked" but changed nothing (#558).
    let mut users_shortcuts = vec![];
    for user in userinfo_shortcuts.iter() {
        let shortcut_info = get_shortcuts_for_user(user).map_err(|err| {
            eyre::format_err!("{err}. Nothing was imported and the file was left as it is.")
        })?;
        users_shortcuts.push((user, shortcut_info));
    }
    for (user, mut shortcut_info) in users_shortcuts {
        let start_time = std::time::Instant::now();
        println!(
            "Found {} shortcuts for user: {}",
            shortcut_info.shortcuts.len(),
            user.user_id
        );

        let mut user_shortcuts = all_shortcuts.clone();
        keep_identity_app_ids(&shortcut_info.shortcuts, &mut user_shortcuts);
        for shortcut in &user_shortcuts {
            println!("Appid: {} name: {}", shortcut.app_id, shortcut.app_name);
        }
        keep_steam_fields(&shortcut_info.shortcuts, &mut user_shortcuts, &previous_ids);

        remove_old_shortcuts(&mut shortcut_info);
        remove_shortcuts_with_same_appid(&mut shortcut_info, &user_shortcuts);

        let final_ids: HashMap<u32, u32> = platform_ids
            .iter()
            .copied()
            .zip(user_shortcuts.iter().map(|s| s.app_id))
            .collect();

        shortcut_info.shortcuts.extend(user_shortcuts);

        if let Err(e) = save_shortcuts(&shortcut_info.shortcuts, Path::new(&shortcut_info.path)) {
            eprintln!("Failed to save shortcuts for user {}: {}", user.user_id, e);
            if let Some(sender) = sender {
                let _ = sender.send(SyncProgress::Error { message: e });
            }
            // Continue with other users even if one fails
        }

        if settings.steam.create_collections {
            let user_collections = map_collection_ids(&collections, &final_ids);
            match write_shortcut_collections(
                &user.user_id,
                Path::new(&user.steam_user_data_folder),
                &user_collections,
            ) {
                Ok(_) => (),
                Err(_e) => eprintln!("Could not write collections, make sure steam is shut down"),
            }
        }

        let duration = start_time.elapsed();
        println!("Finished synchronizing games in: {duration:?}");
    }
    Ok(userinfo_shortcuts)
}

/// A game with an identity keeps the app id Steam already knows it by, even when its
/// target changed (an owned game that got installed launches the game instead of the
/// store's installer, so exe and with it the computed id change). Steam reads the app id
/// from `shortcuts.vdf`; artwork in `grid/` and playtime are keyed by it.
///
/// Two identities are the same game when any of their ids match ([`identity_ids`]): an
/// installed EA game lists every offer in its manifest, and the account may own it under
/// any of them. A match on a shortcut's first id is preferred.
fn keep_identity_app_ids(existing: &[ShortcutOwned], new_shortcuts: &mut [ShortcutOwned]) {
    let mut existing_by_id: HashMap<String, u32> = HashMap::new();
    // First ids go in before the other ids, so a game's own id wins over an id another
    // game merely lists.
    for first_ids_only in [true, false] {
        for old in existing {
            let Some(identity) = identity_of(old) else {
                continue;
            };
            let mut ids = identity_ids(identity);
            if first_ids_only {
                ids.truncate(1);
            }
            for id in ids {
                existing_by_id.entry(id).or_insert(old.app_id);
            }
        }
    }
    let mut taken: HashSet<u32> = new_shortcuts.iter().map(|s| s.app_id).collect();
    for shortcut in new_shortcuts.iter_mut() {
        let Some(identity) = identity_of(shortcut) else {
            continue;
        };
        let Some(kept_id) = identity_ids(identity)
            .iter()
            .find_map(|id| existing_by_id.get(id).copied())
        else {
            continue;
        };
        if kept_id == shortcut.app_id || taken.contains(&kept_id) {
            continue;
        }
        taken.insert(kept_id);
        shortcut.app_id = kept_id;
    }
}

fn map_collection_ids(
    collections: &[(String, Vec<u32>)],
    final_ids: &HashMap<u32, u32>,
) -> Vec<(String, Vec<u32>)> {
    collections
        .iter()
        .map(|(name, ids)| {
            let mut seen = HashSet::new();
            let mapped: Vec<u32> = ids
                .iter()
                .filter_map(|id| final_ids.get(id).copied())
                .filter(|id| seen.insert(*id))
                .collect();
            (name.clone(), mapped)
        })
        .collect()
}

pub async fn download_images(
    settings: &Settings,
    userinfo_shortcuts: &[SteamUsersInfo],
    sender: &mut Option<Sender<SyncProgress>>,
) {
    if settings.steamgrid_db.enabled {
        download_images_for_users(settings, userinfo_shortcuts, sender).await;
        if settings.steamgrid_db.prefer_animated {
            let mut set = settings.clone();
            set.steamgrid_db.prefer_animated = false;
            download_images_for_users(&set, userinfo_shortcuts, sender).await;
        }
    }
}

pub trait IsBoilRShortcut {
    fn is_boilr_shortcut(&self) -> bool;
}

impl IsBoilRShortcut for ShortcutOwned {
    fn is_boilr_shortcut(&self) -> bool {
        let boilr_tag = BOILR_TAG.to_string();
        self.tags.contains(&boilr_tag) || self.dev_kit_game_id.starts_with(&boilr_tag)
    }
}

/// Steam stores some per-game state in `shortcuts.vdf` itself: the "Include in VR Library"
/// flag (#379), the last time the game was played (#389), and the overlay, desktop
/// configuration and hidden switches from the game's properties. A sync replaces each
/// shortcut with a freshly built one, so copy that state over from the shortcut Steam
/// already has under the same app id. `previous_ids` maps a renamed game's new app id to
/// its old one, so the first sync after a rename still finds Steam's entry.
fn keep_steam_fields(
    existing: &[ShortcutOwned],
    new_shortcuts: &mut [ShortcutOwned],
    previous_ids: &HashMap<u32, u32>,
) {
    let mut existing_by_id: HashMap<u32, &ShortcutOwned> = HashMap::new();
    for old in existing {
        // With duplicates, the one Steam last updated is the one the user sees.
        let entry = existing_by_id.entry(old.app_id).or_insert(old);
        if old.last_play_time > entry.last_play_time {
            *entry = old;
        }
    }
    for shortcut in new_shortcuts {
        let old = existing_by_id.get(&shortcut.app_id).or_else(|| {
            previous_ids
                .get(&shortcut.app_id)
                .and_then(|id| existing_by_id.get(id))
        });
        if let Some(old) = old {
            shortcut.open_vr = old.open_vr;
            shortcut.last_play_time = old.last_play_time;
            shortcut.allow_overlay = old.allow_overlay;
            shortcut.allow_desktop_config = old.allow_desktop_config;
            shortcut.is_hidden = old.is_hidden;
        }
    }
}

fn remove_shortcuts_with_same_appid(
    shortcut_info: &mut ShortcutInfo,
    new_shortcuts: &[ShortcutOwned],
) {
    let app_ids: HashSet<u32> = new_shortcuts.iter().map(|s| s.app_id).collect();
    shortcut_info
        .shortcuts
        .retain(|shortcut| !app_ids.contains(&shortcut.app_id));
}

fn remove_old_shortcuts(shortcut_info: &mut ShortcutInfo) {
    shortcut_info
        .shortcuts
        .retain(|shortcut| !shortcut.is_boilr_shortcut());
}

pub fn fix_all_shortcut_icons(settings: &Settings) -> eyre::Result<()> {
    let mut userinfo_shortcuts = get_shortcuts_paths(&settings.steam)
        .map_err(|e| eyre::format_err!("Could not find steam shortcuts; {e}"))?;
    for user in userinfo_shortcuts.iter_mut() {
        let shortcut_info = get_shortcuts_for_user(user);
        if let Ok(mut shortcut_info) = shortcut_info {
            let changes = fix_shortcut_icons(
                user,
                &mut shortcut_info.shortcuts,
                settings.steam.optimize_for_big_picture,
            );
            if changes {
                if let Err(e) =
                    save_shortcuts(&shortcut_info.shortcuts, Path::new(&shortcut_info.path))
                {
                    eprintln!(
                        "Failed to save shortcut icons for user {}: {}",
                        user.user_id, e
                    );
                    // Continue with other users
                }
            }
        }
    }
    Ok(())
}

fn fix_shortcut_icons(
    user: &SteamUsersInfo,
    shortcuts: &mut Vec<ShortcutOwned>,
    big_picture_mode: bool,
) -> bool {
    let image_folder = Path::new(&user.steam_user_data_folder)
        .join("config")
        .join("grid");
    let image_type = if big_picture_mode {
        ImageType::BigPicture
    } else {
        ImageType::Icon
    };

    let mut has_changes = false;
    for shortcut in shortcuts {
        let app_id = shortcut.app_id;
        let icon_exsists = Path::new(&shortcut.icon).exists() && !shortcut.icon.is_empty();
        for ext in ["ico", "png", "jpg", "webp"] {
            let path = image_folder.join(image_type.file_name(app_id, ext));
            if !icon_exsists && path.exists() {
                shortcut.icon = path.to_string_lossy().to_string();
                has_changes = true;
                break;
            }
        }
    }
    has_changes
}

fn write_shortcut_collections<S: AsRef<str>>(
    steam_id: S,
    user_data_folder: &Path,
    platform_results: &[(String, Vec<u32>)],
) -> Result<(), Box<dyn Error>> {
    let mut collections = vec![];

    for (name, ids) in platform_results {
        if ids.is_empty() {
            continue;
        }
        let game_ids = ids.iter().map(|id| *id as usize).collect();
        collections.push(Collection {
            name: name.clone(),
            game_ids,
        });
    }
    println!("Writing {} collections ", collections.len());
    write_collections(steam_id.as_ref(), user_data_folder, &collections)?;
    Ok(())
}

/// Writes to a temporary file next to `shortcuts.vdf` and renames it over the original, so
/// a crash or a full disk never leaves Steam with half a file. A `shortcuts.vdf` that is a
/// symlink keeps being one: the file it points to is the one replaced.
fn save_shortcuts(shortcuts: &[ShortcutOwned], path: &Path) -> Result<(), String> {
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path = target.as_path();
    let mut shortcuts_refs = vec![];
    for shortcut in shortcuts {
        shortcuts_refs.push(shortcut.borrow());
    }
    let new_content = shortcuts_to_bytes(&shortcuts_refs);
    let advice =
        "Check that Steam is not running and you have write permissions to the Steam folder.";
    let tmp_path = path.with_extension("vdf.boilr-tmp");
    let write = || -> std::io::Result<()> {
        let mut file = File::create(&tmp_path)?;
        file.write_all(new_content.as_slice())?;
        file.sync_all()?;
        std::fs::rename(&tmp_path, path)
    };
    match write() {
        Ok(()) => {
            println!("Saved {} shortcuts", shortcuts.len());
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(format!(
                "Failed to write shortcuts to {}: {}. {advice}",
                path.display(),
                e
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use steam_shortcuts_util::Shortcut;

    fn shortcut(name: &str, exe: &str) -> ShortcutOwned {
        Shortcut::new("0", name, exe, "", "", "", "").to_owned()
    }

    fn owned(name: &str, exe: &str, store: &str, id: &str) -> ShortcutOwned {
        let mut shortcut = shortcut(name, exe);
        shortcut.dev_kit_game_id = shortcut_identity(store, id);
        shortcut.app_id = owned_app_id(&format!("{store}:{id}"));
        shortcut
    }

    #[test]
    fn identity_keys_and_ids() {
        assert_eq!(identity_key("ea:OFB-EAST:1,1002139"), "ea:OFB-EAST:1");
        assert_eq!(identity_key("gog:1207658924"), "gog:1207658924");
        assert_eq!(
            identity_ids("ea:OFB-EAST:1, 1002139"),
            vec!["ea:OFB-EAST:1".to_string(), "ea:1002139".to_string()]
        );
    }

    #[test]
    fn one_title_owned_in_two_stores_gets_two_app_ids() {
        let gog = owned("Fallout 3", "xdg-open", "gog", "1454315831");
        let epic = owned(
            "Fallout 3",
            "xdg-open",
            "epic",
            "adeae8bbfc94427db57c7dfecce3f1d4",
        );
        assert_ne!(gog.app_id, epic.app_id);
        assert!(has_owned_app_id(&gog));
        assert!(!has_owned_app_id(&shortcut("Fallout 3", "xdg-open")));
        // Other ids of the same game don't move the app id.
        assert_eq!(
            owned_app_id("ea:OFB-EAST:1"),
            owned_app_id("ea:OFB-EAST:1,1002139")
        );
    }

    #[test]
    fn a_game_that_is_not_installed_keeps_its_app_id_whatever_changes() {
        let mut game = owned("Fallout 3", "xdg-open", "gog", "1");
        let id = game.app_id;
        // A rename, another launcher: the id is a hash of the identity alone.
        game.app_name = "Fallout III".to_string();
        game.exe = "\"C:\\GOG Galaxy\\GalaxyClient.exe\"".to_string();
        assert_eq!(app_id_after_change(&game), id);
        assert!(has_owned_app_id(&game));
        assert_ne!(id, calculate_app_id("xdg-open", "Fallout 3"));

        // An installed game is still hashed from exe and name.
        let mut installed = shortcut("Game", "/games/game");
        installed.dev_kit_game_id = shortcut_identity("gog", "1");
        installed.app_name = "Renamed".to_string();
        assert_eq!(
            app_id_after_change(&installed),
            calculate_app_id("/games/game", "Renamed")
        );
    }

    #[test]
    fn a_rename_made_before_a_game_was_installed_still_applies() {
        let owned_game = owned("Game", "xdg-open", "gog", "1");
        let mut installed = shortcut("Game", "/games/game");
        installed.dev_kit_game_id = shortcut_identity("gog", "1");
        let other = shortcut("Game", "/games/other");

        let renames = HashMap::from([(owned_game.app_id, "My Game".to_string())]);
        assert_eq!(
            rename_for(&owned_game, &renames).map(String::as_str),
            Some("My Game")
        );
        assert_eq!(
            rename_for(&installed, &renames).map(String::as_str),
            Some("My Game")
        );
        assert_eq!(rename_for(&other, &renames), None);
        // A rename of the installed game itself comes first.
        let renames = HashMap::from([
            (owned_game.app_id, "Old".to_string()),
            (installed.app_id, "New".to_string()),
        ]);
        assert_eq!(
            rename_for(&installed, &renames).map(String::as_str),
            Some("New")
        );
    }

    #[test]
    fn an_installed_game_found_under_another_of_its_ids_keeps_the_app_id() {
        // Steam has the game as the account lists it; the installed copy's manifest names
        // that offer second.
        let mut old = shortcut("Game", "/usr/bin/xdg-open");
        old.dev_kit_game_id = shortcut_identity("ea", "OFB-EAST:2,1002139");
        let old_id = old.app_id;
        let mut other = shortcut("Other", "/usr/bin/xdg-open");
        other.dev_kit_game_id = shortcut_identity("ea", "OFB-EAST:1");
        let other_id = other.app_id;
        let mut installed = shortcut("Game", "/games/game.exe");
        installed.dev_kit_game_id = shortcut_identity("ea", "OFB-EAST:9,OFB-EAST:2");
        // A game whose own id another game merely lists keeps its own.
        let mut listed_first = shortcut("Other", "/games/other.exe");
        listed_first.dev_kit_game_id = shortcut_identity("ea", "OFB-EAST:1,OFB-EAST:2");
        let mut new_shortcuts = vec![installed, listed_first];

        keep_identity_app_ids(&[old, other], &mut new_shortcuts);

        let ids: Vec<u32> = new_shortcuts.iter().map(|s| s.app_id).collect();
        assert_eq!(ids, vec![old_id, other_id]);
    }

    #[test]
    fn keeps_vr_flag_and_last_play_time_of_existing_shortcut() {
        let mut old = shortcut("Game", "/usr/bin/game");
        old.open_vr = 1;
        old.last_play_time = 1_700_000_000;
        let mut new_shortcuts = vec![shortcut("Game", "/usr/bin/game")];

        keep_steam_fields(&[old], &mut new_shortcuts, &HashMap::new());

        let kept = new_shortcuts.first();
        assert_eq!(kept.map(|s| s.open_vr), Some(1));
        assert_eq!(kept.map(|s| s.last_play_time), Some(1_700_000_000));
    }

    #[test]
    fn keeps_properties_set_in_steam() {
        let mut old = shortcut("Game", "/usr/bin/game");
        old.allow_overlay = false;
        old.allow_desktop_config = false;
        old.is_hidden = true;
        let mut new_shortcuts = vec![shortcut("Game", "/usr/bin/game")];

        keep_steam_fields(&[old], &mut new_shortcuts, &HashMap::new());

        let kept = new_shortcuts.first();
        assert_eq!(kept.map(|s| s.allow_overlay), Some(false));
        assert_eq!(kept.map(|s| s.allow_desktop_config), Some(false));
        assert_eq!(kept.map(|s| s.is_hidden), Some(true));
    }

    #[test]
    fn renamed_game_keeps_state_from_its_old_app_id() {
        let mut old = shortcut("Old Name", "/usr/bin/game");
        old.last_play_time = 1_700_000_000;
        let old_id = old.app_id;
        let mut new_shortcuts = vec![shortcut("New Name", "/usr/bin/game")];
        let new_id = new_shortcuts.first().map(|s| s.app_id).unwrap_or_default();
        let previous_ids = HashMap::from([(new_id, old_id)]);

        keep_steam_fields(&[old], &mut new_shortcuts, &previous_ids);

        assert_eq!(
            new_shortcuts.first().map(|s| s.last_play_time),
            Some(1_700_000_000)
        );
    }

    #[test]
    fn duplicate_entries_use_the_most_recently_played() {
        let mut played = shortcut("Game", "/usr/bin/game");
        played.last_play_time = 1_700_000_000;
        let stale = shortcut("Game", "/usr/bin/game");
        let mut new_shortcuts = vec![shortcut("Game", "/usr/bin/game")];

        keep_steam_fields(&[played, stale], &mut new_shortcuts, &HashMap::new());

        assert_eq!(
            new_shortcuts.first().map(|s| s.last_play_time),
            Some(1_700_000_000)
        );
    }

    #[test]
    fn installed_game_keeps_the_app_id_of_its_owned_shortcut() {
        let mut owned = shortcut("Game", "/usr/bin/xdg-open");
        owned.dev_kit_game_id = shortcut_identity("gog", "1207658924");
        let owned_id = owned.app_id;
        let mut installed = shortcut("Game", "/games/game/start.sh");
        installed.dev_kit_game_id = shortcut_identity("gog", "1207658924");
        let mut other = shortcut("Other", "/games/other");
        other.dev_kit_game_id = shortcut_identity("gog", "1");
        let other_id = other.app_id;
        let mut new_shortcuts = vec![installed, other];

        keep_identity_app_ids(&[owned], &mut new_shortcuts);

        let ids: Vec<u32> = new_shortcuts.iter().map(|s| s.app_id).collect();
        assert_eq!(ids, vec![owned_id, other_id]);
    }

    #[test]
    fn shortcuts_without_identity_keep_their_computed_id() {
        let mut old = shortcut("Game", "/old");
        old.dev_kit_game_id = BOILR_TAG.to_string();
        let mut new_shortcuts = vec![shortcut("Game", "/new")];
        let computed = new_shortcuts.first().map(|s| s.app_id);

        keep_identity_app_ids(&[old], &mut new_shortcuts);

        assert_eq!(new_shortcuts.first().map(|s| s.app_id), computed);
    }

    #[test]
    fn identity_with_other_ids_keeps_the_app_id() {
        let mut old = shortcut("Game", "/usr/bin/xdg-open");
        old.dev_kit_game_id = shortcut_identity("ea", "OFB-EAST:1,1002139");
        let old_id = old.app_id;
        let mut installed = shortcut("Game", "/games/game.exe");
        installed.dev_kit_game_id = shortcut_identity("ea", "OFB-EAST:1");
        let mut new_shortcuts = vec![installed];

        keep_identity_app_ids(&[old], &mut new_shortcuts);

        assert_eq!(new_shortcuts.first().map(|s| s.app_id), Some(old_id));
    }

    #[test]
    fn kept_id_is_not_reused_when_another_shortcut_has_it() {
        let mut old = shortcut("Game", "/old");
        old.dev_kit_game_id = shortcut_identity("epic", "Fortnite");
        let mut moved = shortcut("Game", "/new");
        moved.dev_kit_game_id = shortcut_identity("epic", "Fortnite");
        let moved_id = moved.app_id;
        // A different game that computes to the old id.
        let clash = shortcut("Game", "/old");
        let mut new_shortcuts = vec![moved, clash];

        keep_identity_app_ids(&[old], &mut new_shortcuts);

        assert_eq!(new_shortcuts.first().map(|s| s.app_id), Some(moved_id));
    }

    #[test]
    fn collections_follow_the_final_app_ids() {
        // Two ids that end up the same are listed once, wherever they stand.
        let collections = vec![("Not installed".to_string(), vec![1, 2, 3, 4])];
        let final_ids = HashMap::from([(1, 10), (2, 2), (4, 10)]);

        let mapped = map_collection_ids(&collections, &final_ids);

        assert_eq!(mapped, vec![("Not installed".to_string(), vec![10, 2])]);
    }

    #[test]
    fn identity_is_read_back() {
        let mut game = shortcut("Game", "/game");
        assert_eq!(identity_of(&game), None);
        game.dev_kit_game_id = shortcut_identity("itch", "42");
        assert_eq!(identity_of(&game), Some("itch:42"));
        assert!(game.is_boilr_shortcut());
    }

    #[test]
    fn new_games_start_without_steam_state() {
        let mut old = shortcut("Other", "/usr/bin/other");
        old.open_vr = 1;
        old.last_play_time = 1_700_000_000;
        let mut new_shortcuts = vec![shortcut("Game", "/usr/bin/game")];

        keep_steam_fields(&[old], &mut new_shortcuts, &HashMap::new());

        let fresh = new_shortcuts.first();
        assert_eq!(fresh.map(|s| s.open_vr), Some(0));
        assert_eq!(fresh.map(|s| s.last_play_time), Some(0));
    }

    #[test]
    fn updates_shortcut_icon_when_the_old_image_was_replaced() -> eyre::Result<()> {
        let temp_dir = std::env::temp_dir().join(format!(
            "boilr-icon-fix-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let grid_dir = temp_dir.join("config").join("grid");
        std::fs::create_dir_all(&grid_dir)?;

        let mut game = shortcut("Game", "/usr/bin/game");
        game.icon = grid_dir
            .join("removed-icon.png")
            .to_string_lossy()
            .to_string();
        let replacement_icon = grid_dir.join(ImageType::Icon.file_name(game.app_id, "png"));
        std::fs::write(&replacement_icon, b"icon")?;
        let replacement_icon = replacement_icon.to_string_lossy().into_owned();
        let user = SteamUsersInfo {
            steam_user_data_folder: temp_dir.to_string_lossy().to_string(),
            shortcut_path: None,
            user_id: "1".to_string(),
        };
        let mut shortcuts = vec![game];

        assert!(fix_shortcut_icons(&user, &mut shortcuts, false));
        assert_eq!(
            shortcuts.first().map(|shortcut| &shortcut.icon),
            Some(&replacement_icon)
        );

        std::fs::remove_dir_all(temp_dir)?;
        Ok(())
    }
}
