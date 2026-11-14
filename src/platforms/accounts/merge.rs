use std::collections::HashSet;

use crate::platforms::ShortcutToImport;
use crate::sync::{identity_ids, identity_key, identity_of};

/// The Steam collection that holds owned games that are not installed.
pub const NOT_INSTALLED_COLLECTION: &str = "Not installed";

/// Drops owned (not installed) entries for games that an installed-games platform also
/// reports: the installed shortcut wins. Also drops repeated owned entries of the same game.
///
/// Identities can name several ids of one game (see [`crate::sync::identity_ids`]); an
/// owned and an installed entry are the same game when any of their ids match. The sync
/// finds the app id Steam knew the owned entry by in the same way, so the installed entry
/// keeps its own identity.
pub fn drop_owned_duplicates<'a, I>(lists: I)
where
    I: IntoIterator<Item = &'a mut Vec<ShortcutToImport>>,
{
    let mut lists: Vec<&mut Vec<ShortcutToImport>> = lists.into_iter().collect();

    // Every id of every installed game.
    let installed: HashSet<String> = lists
        .iter()
        .flat_map(|list| list.iter())
        .filter(|entry| entry.installed)
        .filter_map(|entry| identity_of(&entry.shortcut))
        .flat_map(identity_ids)
        .collect();

    let mut seen_owned: HashSet<String> = HashSet::new();
    for list in lists.iter_mut() {
        list.retain(|entry| {
            let Some(identity) = identity_of(&entry.shortcut) else {
                return true;
            };
            if entry.installed {
                return true;
            }
            if identity_ids(identity)
                .iter()
                .any(|id| installed.contains(id))
            {
                return false;
            }
            seen_owned.insert(identity_key(identity).to_string())
        });
    }
}

/// The "Not installed" collection: every entry that is not installed, by its platform app id.
pub fn not_installed_collection<'a, I>(lists: I) -> Option<(String, Vec<u32>)>
where
    I: IntoIterator<Item = &'a Vec<ShortcutToImport>>,
{
    let ids: Vec<u32> = lists
        .into_iter()
        .flat_map(|list| list.iter())
        .filter(|entry| !entry.installed)
        .map(|entry| entry.shortcut.app_id)
        .collect();
    (!ids.is_empty()).then(|| (NOT_INSTALLED_COLLECTION.to_string(), ids))
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

    fn names(list: &[ShortcutToImport]) -> Vec<&str> {
        list.iter().map(|e| e.shortcut.app_name.as_str()).collect()
    }

    #[test]
    fn installed_copy_replaces_the_owned_entry() {
        let mut installed = vec![entry("Installed", Some("boilr:gog:1"), true)];
        let mut owned = vec![
            entry("Owned copy", Some("boilr:gog:1"), false),
            entry("Only owned", Some("boilr:gog:2"), false),
            entry("Only owned again", Some("boilr:gog:2"), false),
            entry("No identity", None, false),
        ];

        drop_owned_duplicates([&mut installed, &mut owned]);

        assert_eq!(names(&installed), ["Installed"]);
        assert_eq!(names(&owned), ["Only owned", "No identity"]);
    }

    #[test]
    fn installed_copy_found_under_another_id_replaces_the_owned_entry() {
        // The EA manifest lists two offers; the account owns the game under the second.
        let installed_identity = "boilr:ea:Origin.OFR.50.0000001,OFB-EAST:52735";
        let mut installed = vec![entry("Installed", Some(installed_identity), true)];
        let mut owned = vec![
            entry("Owned copy", Some("boilr:ea:OFB-EAST:52735,1002139"), false),
            entry("Other", Some("boilr:ea:OFB-EAST:1"), false),
        ];

        drop_owned_duplicates([&mut owned, &mut installed]);

        assert_eq!(names(&owned), ["Other"]);
        // The installed game keeps its own identity; the sync matches it to the owned
        // shortcut in Steam by the id they share.
        assert_eq!(
            installed
                .first()
                .map(|e| e.shortcut.dev_kit_game_id.as_str()),
            Some(installed_identity)
        );
    }

    #[test]
    fn installed_copy_matches_a_content_id() {
        let mut installed = vec![entry("Installed", Some("boilr:ea:1002139"), true)];
        let mut owned = vec![entry(
            "Owned",
            Some("boilr:ea:OFB-EAST:52735,1002139"),
            false,
        )];

        drop_owned_duplicates([&mut installed, &mut owned]);

        assert!(owned.is_empty());
    }

    #[test]
    fn collection_lists_only_not_installed_games() {
        let installed = vec![entry("Installed", None, true)];
        let owned = vec![entry("Owned", Some("boilr:epic:x"), false)];
        let owned_id = owned.first().map(|e| e.shortcut.app_id).unwrap_or_default();

        let collection = not_installed_collection([&installed, &owned]);

        assert_eq!(
            collection,
            Some((NOT_INSTALLED_COLLECTION.to_string(), vec![owned_id]))
        );
        assert_eq!(not_installed_collection([&installed]), None);
    }
}
