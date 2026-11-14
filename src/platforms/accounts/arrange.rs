//! Which games that are not installed go into Steam, and under what name: the
//! `[owned_games]` settings ([`OwnedGamesSettings`]) applied to the game lists of a sync.

use std::collections::{HashMap, HashSet};

use boilr_core::owned::{
    labelled_name, normalize_title, MultiStore, NameLabel, OwnedGamesSettings,
};

use super::merge::drop_owned_duplicates;
use crate::platforms::ShortcutToImport;
use crate::sync::identity_of;

/// What [`arrange_owned_games`] left out.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Arranged {
    /// Installed through the store that owns it: the installed entry stays.
    pub installed: usize,
    /// Steam has the game installed itself.
    pub in_steam: usize,
    /// Installed from another store or launcher.
    pub installed_elsewhere: usize,
    /// Owned in a store further down the preferred order.
    pub other_store: usize,
}

/// The store an owned, not installed entry comes from (the first part of its identity).
fn owned_store(entry: &ShortcutToImport) -> Option<String> {
    if entry.installed {
        return None;
    }
    let identity = identity_of(&entry.shortcut)?;
    identity.split_once(':').map(|(store, _)| store.to_string())
}

/// Applies the `[owned_games]` settings to the game lists of a sync, in place:
///
/// 1. An owned entry whose game is installed through the same store goes (always).
/// 2. With `skip_installed_elsewhere`, one whose title an installed entry has goes.
/// 3. With `skip_games_in_steam`, one whose title is in `steam_titles` (normalized titles of
///    the games Steam has installed) goes.
/// 4. A title several stores own keeps one entry ([`MultiStore::Preferred`]), or each entry
///    is named after its store ([`MultiStore::LabelStore`]).
/// 5. The remaining owned entries are named as `name_label` asks.
///
/// Names are compared by [`normalize_title`]. Labels do not change app ids: an owned
/// shortcut's app id comes from its identity.
pub fn arrange_owned_games<'a, I>(
    lists: I,
    settings: &OwnedGamesSettings,
    steam_titles: &HashSet<String>,
) -> Arranged
where
    I: IntoIterator<Item = &'a mut Vec<ShortcutToImport>>,
{
    let mut lists: Vec<&mut Vec<ShortcutToImport>> = lists.into_iter().collect();
    let mut arranged = Arranged::default();

    let owned_before = count_owned(&lists);
    drop_owned_duplicates(lists.iter_mut().map(|list| &mut **list));
    arranged.installed = owned_before - count_owned(&lists);

    let installed_titles: HashSet<String> = lists
        .iter()
        .flat_map(|list| list.iter())
        .filter(|entry| entry.installed)
        .map(|entry| normalize_title(&entry.shortcut.app_name))
        .collect();
    for list in lists.iter_mut() {
        list.retain(|entry| {
            if owned_store(entry).is_none() {
                return true;
            }
            let title = normalize_title(&entry.shortcut.app_name);
            if settings.skip_installed_elsewhere && installed_titles.contains(&title) {
                arranged.installed_elsewhere += 1;
                return false;
            }
            if settings.skip_games_in_steam && steam_titles.contains(&title) {
                arranged.in_steam += 1;
                return false;
            }
            true
        });
    }

    // Every store that owns a title, best first.
    let mut stores_of: HashMap<String, Vec<String>> = HashMap::new();
    for entry in lists.iter().flat_map(|list| list.iter()) {
        if let Some(store) = owned_store(entry) {
            let stores = stores_of
                .entry(normalize_title(&entry.shortcut.app_name))
                .or_default();
            if !stores.contains(&store) {
                stores.push(store);
            }
        }
    }
    for stores in stores_of.values_mut() {
        stores.sort_by_key(|store| settings.store_rank(store));
    }

    if settings.multi_store == MultiStore::Preferred {
        // The preferred store's first entry of a title stays.
        let mut kept: HashSet<String> = HashSet::new();
        for list in lists.iter_mut() {
            list.retain(|entry| {
                let Some(store) = owned_store(entry) else {
                    return true;
                };
                let title = normalize_title(&entry.shortcut.app_name);
                let preferred = stores_of
                    .get(&title)
                    .and_then(|stores| stores.first())
                    .is_none_or(|best| *best == store);
                if preferred && kept.insert(title) {
                    true
                } else {
                    arranged.other_store += 1;
                    false
                }
            });
        }
    }

    let show_not_installed = matches!(
        settings.name_label,
        NameLabel::NotInstalled | NameLabel::StoreAndNotInstalled
    );
    for entry in lists.iter_mut().flat_map(|list| list.iter_mut()) {
        let Some(store) = owned_store(entry) else {
            continue;
        };
        let in_several_stores = stores_of
            .get(&normalize_title(&entry.shortcut.app_name))
            .is_some_and(|stores| stores.len() > 1);
        let show_store = matches!(
            settings.name_label,
            NameLabel::Store | NameLabel::StoreAndNotInstalled
        ) || (settings.multi_store == MultiStore::LabelStore && in_several_stores);
        entry.shortcut.app_name = labelled_name(
            &entry.shortcut.app_name,
            &store,
            show_store,
            show_not_installed,
        );
    }
    arranged
}

fn count_owned(lists: &[&mut Vec<ShortcutToImport>]) -> usize {
    lists
        .iter()
        .flat_map(|list| list.iter())
        .filter(|entry| owned_store(entry).is_some())
        .count()
}

/// The normalized titles of the games Steam has installed itself.
pub fn steam_titles(steam: &crate::steam::SteamSettings) -> HashSet<String> {
    crate::steam::get_installed_games(steam)
        .iter()
        .map(|game| normalize_title(&game.name))
        .collect()
}

/// [`arrange_owned_games`] with the settings of `settings`, reading Steam's games when the
/// settings need them.
pub fn arrange_owned<'a, I>(lists: I, settings: &crate::settings::Settings) -> Arranged
where
    I: IntoIterator<Item = &'a mut Vec<ShortcutToImport>>,
{
    let owned = &settings.owned_games;
    let steam = if owned.skip_games_in_steam {
        steam_titles(&settings.steam)
    } else {
        HashSet::new()
    };
    arrange_owned_games(lists, owned, &steam)
}

#[cfg(test)]
mod tests {
    use super::*;
    use steam_shortcuts_util::Shortcut;

    fn entry(name: &str, identity: Option<&str>, installed: bool) -> ShortcutToImport {
        let mut shortcut = Shortcut::new("0", name, name, "", "", "", "").to_owned();
        if let Some(identity) = identity {
            shortcut.dev_kit_game_id = identity.to_string();
        }
        ShortcutToImport {
            shortcut,
            needs_proton: false,
            needs_symlinks: false,
            installed,
        }
    }

    fn names(lists: &[Vec<ShortcutToImport>]) -> Vec<String> {
        let mut names: Vec<String> = lists
            .iter()
            .flatten()
            .map(|e| e.shortcut.app_name.clone())
            .collect();
        names.sort();
        names
    }

    fn sample() -> Vec<Vec<ShortcutToImport>> {
        vec![
            // Installed from Epic.
            vec![entry("PREY", Some("boilr:epic:Prey"), true)],
            // A launcher without store identities.
            vec![entry("Rayman Legends", None, true)],
            vec![
                entry("Prey", Some("boilr:gog:1"), false),
                entry("Frostpunk", Some("boilr:gog:2"), false),
                entry("Rocket League", Some("boilr:gog:3"), false),
            ],
            vec![
                entry("Frostpunk", Some("boilr:epic:F"), false),
                entry("Rayman Legends", Some("boilr:epic:R"), false),
                entry("Alan Wake", Some("boilr:epic:A"), false),
            ],
        ]
    }

    fn run(settings: &OwnedGamesSettings, steam: &[&str]) -> (Vec<String>, Arranged) {
        let mut lists = sample();
        let steam: HashSet<String> = steam.iter().map(|t| normalize_title(t)).collect();
        let arranged = arrange_owned_games(lists.iter_mut(), settings, &steam);
        (names(&lists), arranged)
    }

    #[test]
    fn the_defaults_leave_one_entry_per_game() {
        let (names, arranged) = run(&OwnedGamesSettings::default(), &["Rocket League"]);
        assert_eq!(
            names,
            vec!["Alan Wake", "Frostpunk", "PREY", "Rayman Legends"]
        );
        assert_eq!(
            arranged,
            Arranged {
                installed: 0,
                in_steam: 1,
                installed_elsewhere: 2,
                other_store: 1,
            }
        );
    }

    #[test]
    fn the_preferred_store_keeps_the_game() {
        let mut lists = sample();
        arrange_owned_games(
            lists.iter_mut(),
            &OwnedGamesSettings::default(),
            &HashSet::new(),
        );
        let frostpunk: Vec<&str> = lists
            .iter()
            .flatten()
            .filter(|e| e.shortcut.app_name == "Frostpunk")
            .map(|e| e.shortcut.dev_kit_game_id.as_str())
            .collect();
        assert_eq!(frostpunk, vec!["boilr:gog:2"]);

        let epic_first = OwnedGamesSettings {
            store_order: vec!["epic".into()],
            ..Default::default()
        };
        let mut lists = sample();
        arrange_owned_games(lists.iter_mut(), &epic_first, &HashSet::new());
        let frostpunk: Vec<&str> = lists
            .iter()
            .flatten()
            .filter(|e| e.shortcut.app_name == "Frostpunk")
            .map(|e| e.shortcut.dev_kit_game_id.as_str())
            .collect();
        assert_eq!(frostpunk, vec!["boilr:epic:F"]);
    }

    #[test]
    fn everything_can_be_kept() {
        let keep_all = OwnedGamesSettings {
            skip_games_in_steam: false,
            skip_installed_elsewhere: false,
            multi_store: MultiStore::All,
            ..Default::default()
        };
        let (names, arranged) = run(&keep_all, &["Rocket League"]);
        assert_eq!(names.len(), 8);
        assert_eq!(arranged, Arranged::default());
    }

    #[test]
    fn copies_in_several_stores_can_carry_their_store() {
        let label = OwnedGamesSettings {
            skip_installed_elsewhere: false,
            multi_store: MultiStore::LabelStore,
            ..Default::default()
        };
        let (names, _) = run(&label, &[]);
        assert_eq!(
            names,
            vec![
                "Alan Wake",
                "Frostpunk (Epic)",
                "Frostpunk (GOG)",
                "PREY",
                "Prey",
                "Rayman Legends",
                "Rayman Legends",
                "Rocket League",
            ]
        );
    }

    #[test]
    fn names_can_say_not_installed_and_the_store() {
        let not_installed = OwnedGamesSettings {
            name_label: NameLabel::NotInstalled,
            ..Default::default()
        };
        let (names, _) = run(&not_installed, &[]);
        assert!(names.contains(&"Alan Wake (Not installed)".to_string()));
        assert!(
            names.contains(&"PREY".to_string()),
            "installed games keep their name"
        );

        let both = OwnedGamesSettings {
            name_label: NameLabel::StoreAndNotInstalled,
            ..Default::default()
        };
        let (names, _) = run(&both, &[]);
        assert!(names.contains(&"Frostpunk (GOG, not installed)".to_string()));
        assert!(names.contains(&"Alan Wake (Epic, not installed)".to_string()));
    }

    #[test]
    fn an_installed_copy_from_the_same_store_replaces_the_owned_one() {
        let mut lists = vec![
            vec![entry("Game", Some("boilr:epic:G"), true)],
            vec![entry("Game", Some("boilr:epic:G"), false)],
        ];
        let arranged = arrange_owned_games(
            lists.iter_mut(),
            &OwnedGamesSettings::default(),
            &HashSet::new(),
        );
        assert_eq!(names(&lists), vec!["Game"]);
        assert_eq!(arranged.installed, 1);
    }
}
