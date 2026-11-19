//! How games a store account owns but that are not installed show up in Steam: which of
//! them are added, and what their names say. The store accounts themselves live outside this
//! crate; here a store is only its key (`epic`, `gog`, ...), the first part of a shortcut's
//! identity (see [`crate::sync::identity_of`]).

use serde::{Deserialize, Serialize};

/// What to do with a game that more than one store account owns, while it is not installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStore {
    /// One entry, from the first store in [`OwnedGamesSettings::store_order`].
    #[default]
    Preferred,
    /// One entry per store, each named after its store ("Frostpunk (GOG)").
    LabelStore,
    /// One entry per store, with the same name.
    All,
}

/// What the name of a game that is not installed says besides the title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameLabel {
    /// Only the title.
    #[default]
    None,
    /// "Frostpunk (Not installed)".
    NotInstalled,
    /// "Frostpunk (GOG)".
    Store,
    /// "Frostpunk (GOG, not installed)".
    StoreAndNotInstalled,
}

/// The `[owned_games]` settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OwnedGamesSettings {
    /// Leave out a game that Steam has installed itself (a Steam game of the same name).
    pub skip_games_in_steam: bool,
    /// Leave out a game that is installed from another store or launcher (the same name).
    pub skip_installed_elsewhere: bool,
    pub multi_store: MultiStore,
    /// Store keys, most preferred first; stores not listed come after, in this order:
    /// [`DEFAULT_STORE_ORDER`].
    pub store_order: Vec<String>,
    pub name_label: NameLabel,
    /// Front ends ask before a sync that adds more games that are not installed than this.
    /// 0 never asks.
    pub confirm_above: usize,
}

/// The store order when none is set: DRM-free first, then the stores with the most complete
/// launchers.
pub const DEFAULT_STORE_ORDER: [&str; 7] =
    ["gog", "epic", "amazon", "ea", "battlenet", "humble", "itch"];

impl Default for OwnedGamesSettings {
    fn default() -> Self {
        Self {
            skip_games_in_steam: true,
            skip_installed_elsewhere: true,
            multi_store: MultiStore::Preferred,
            store_order: DEFAULT_STORE_ORDER.iter().map(|s| s.to_string()).collect(),
            name_label: NameLabel::None,
            confirm_above: 100,
        }
    }
}

impl OwnedGamesSettings {
    /// Where a store stands in the preferred order: lower is preferred. Stores missing from
    /// [`Self::store_order`] come after the listed ones, in [`DEFAULT_STORE_ORDER`].
    pub fn store_rank(&self, store_key: &str) -> usize {
        if let Some(rank) = self.store_order.iter().position(|s| s == store_key) {
            return rank;
        }
        let after = self.store_order.len();
        after
            + DEFAULT_STORE_ORDER
                .iter()
                .position(|s| *s == store_key)
                .unwrap_or(DEFAULT_STORE_ORDER.len())
    }

    /// Whether a sync that adds `not_installed` such games should be confirmed first.
    pub fn needs_confirmation(&self, not_installed: usize) -> bool {
        self.confirm_above > 0 && not_installed > self.confirm_above
    }
}

/// The short name of a store in a game's name.
pub fn store_label(store_key: &str) -> &str {
    match store_key {
        "epic" => "Epic",
        "gog" => "GOG",
        "amazon" => "Amazon",
        "ea" => "EA",
        "battlenet" => "Battle.net",
        "humble" => "Humble",
        "itch" => "itch.io",
        other => other,
    }
}

/// What goes in parentheses after the title, if anything.
fn label_text(store_key: &str, show_store: bool, show_not_installed: bool) -> Option<String> {
    match (show_store, show_not_installed) {
        (true, true) => Some(format!("{}, not installed", store_label(store_key))),
        (true, false) => Some(store_label(store_key).to_string()),
        (false, true) => Some("Not installed".to_string()),
        (false, false) => None,
    }
}

/// The name of a game that is not installed: its title, and the label the settings ask for.
pub fn labelled_name(
    title: &str,
    store_key: &str,
    show_store: bool,
    show_not_installed: bool,
) -> String {
    match label_text(store_key, show_store, show_not_installed) {
        Some(label) => format!("{title} ({label})"),
        None => title.to_string(),
    }
}

/// The title without a label [`labelled_name`] may have added, so artwork searches and name
/// comparisons use the game's own title.
pub fn strip_label<'a>(name: &'a str, store_key: &str) -> &'a str {
    for (show_store, show_not_installed) in [(true, true), (true, false), (false, true)] {
        if let Some(label) = label_text(store_key, show_store, show_not_installed) {
            if let Some(title) = name
                .strip_suffix(&format!(" ({label})"))
                .filter(|title| !title.trim().is_empty())
            {
                return title;
            }
        }
    }
    name
}

/// A title reduced to what tells games apart: letters and digits, lower case, without
/// trademark signs. "PREY" and "Prey", "Brothers - A Tale of Two Sons" and "Brothers: A Tale
/// of Two Sons" come out the same.
pub fn normalize_title(title: &str) -> String {
    title
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_go_on_and_come_off() {
        assert_eq!(labelled_name("Frostpunk", "gog", false, false), "Frostpunk");
        assert_eq!(
            labelled_name("Frostpunk", "gog", false, true),
            "Frostpunk (Not installed)"
        );
        assert_eq!(
            labelled_name("Frostpunk", "gog", true, false),
            "Frostpunk (GOG)"
        );
        assert_eq!(
            labelled_name("Frostpunk", "battlenet", true, true),
            "Frostpunk (Battle.net, not installed)"
        );
        for (store, show_store, show_not_installed) in [
            ("gog", false, true),
            ("gog", true, false),
            ("epic", true, true),
        ] {
            let name = labelled_name("Fallout 3 (2008)", store, show_store, show_not_installed);
            assert_eq!(strip_label(&name, store), "Fallout 3 (2008)");
        }
        // Another store's label, or a title that is only a label, stays as it is.
        assert_eq!(strip_label("Frostpunk (GOG)", "epic"), "Frostpunk (GOG)");
        assert_eq!(strip_label(" (Not installed)", "gog"), " (Not installed)");
    }

    #[test]
    fn titles_compare_by_letters_and_digits() {
        assert_eq!(normalize_title("PREY"), normalize_title("Prey"));
        assert_eq!(
            normalize_title("Brothers - A Tale of Two Sons"),
            normalize_title("Brothers: A Tale of Two Sons")
        );
        assert_eq!(
            normalize_title("Alice: Madness Returns™"),
            "alicemadnessreturns"
        );
        assert_ne!(normalize_title("Fallout 3"), normalize_title("Fallout 4"));
    }

    #[test]
    fn store_order_ranks_listed_stores_first() {
        let settings = OwnedGamesSettings {
            store_order: vec!["epic".into(), "gog".into()],
            ..Default::default()
        };
        assert!(settings.store_rank("epic") < settings.store_rank("gog"));
        assert!(settings.store_rank("gog") < settings.store_rank("amazon"));
        assert!(settings.store_rank("amazon") < settings.store_rank("humble"));
        assert!(settings.store_rank("humble") < settings.store_rank("unknown"));
        let defaults = OwnedGamesSettings::default();
        assert!(defaults.store_rank("gog") < defaults.store_rank("epic"));
    }

    #[test]
    fn asks_only_above_the_threshold() {
        let settings = OwnedGamesSettings::default();
        assert!(!settings.needs_confirmation(100));
        assert!(settings.needs_confirmation(101));
        let never = OwnedGamesSettings {
            confirm_above: 0,
            ..Default::default()
        };
        assert!(!never.needs_confirmation(5000));
    }

    #[test]
    fn missing_settings_take_the_defaults() -> Result<(), toml::de::Error> {
        let partial: OwnedGamesSettings = toml::from_str("multi_store = \"label_store\"\n")?;
        assert_eq!(partial.multi_store, MultiStore::LabelStore);
        assert!(partial.skip_games_in_steam);
        assert_eq!(partial.store_order.first().map(String::as_str), Some("gog"));
        Ok(())
    }
}
