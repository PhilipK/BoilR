//! User-chosen names for imported games, keyed by Steam app id.
use std::{collections::HashMap, error::Error};

use crate::config::get_renames_file;

pub fn load_rename_map() -> HashMap<u32, String> {
    try_load_rename_map().unwrap_or_default()
}

pub fn try_load_rename_map() -> Result<HashMap<u32, String>, Box<dyn Error>> {
    let rename_map = get_renames_file();
    let file_content = std::fs::read_to_string(rename_map)?;
    let deserialized = serde_json::from_str(&file_content)?;
    Ok(deserialized)
}

/// Sets the name BoilR gives a game in Steam. An empty name, or the launcher's own name, removes
/// the rename. Keyed by the app id of the launcher's name, like egui's rename map.
pub fn set_rename(app_id: u32, original: &str, name: &str) -> Result<(), Box<dyn Error>> {
    let mut map = load_rename_map();
    apply_rename(&mut map, app_id, original, name);
    let path = get_renames_file();
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, serde_json::to_string(&map)?)?;
    Ok(())
}

/// Records `name` as the rename for `app_id` in `map`.
///
/// This is the only rule for what `renames.json` holds: an empty name, or the
/// launcher's own name, removes the entry rather than storing it. Front ends
/// must go through this so a rename that changes nothing leaves no record —
/// the map is also what Steam is told to call a game, and a no-op entry would
/// pin the launcher's own name there forever.
pub fn apply_rename(map: &mut HashMap<u32, String>, app_id: u32, original: &str, name: &str) {
    let name = name.trim();
    if name.is_empty() || name == original {
        map.remove(&app_id);
    } else {
        map.insert(app_id, name.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_and_resets() {
        let mut map = HashMap::new();
        apply_rename(&mut map, 7, "Hades", "  Hades II  ");
        assert_eq!(map.get(&7).map(String::as_str), Some("Hades II"));
        apply_rename(&mut map, 7, "Hades", "Hades");
        assert!(map.is_empty());
        apply_rename(&mut map, 7, "Hades", "Other");
        apply_rename(&mut map, 7, "Hades", "   ");
        assert!(map.is_empty());
    }

    /// Opening the rename editor seeds the map with the launcher's own name so
    /// the text field has something to edit. Confirming without typing anything
    /// must drop that seed instead of writing it to renames.json (#561).
    #[test]
    fn a_seeded_no_op_rename_records_nothing() {
        let mut map = HashMap::new();
        map.insert(4249995176, "Ryujinx (Ryubing)".to_string());
        map.insert(4143722054, "Motrix".to_string());

        apply_rename(
            &mut map,
            4249995176,
            "Ryujinx (Ryubing)",
            "Ryujinx (Ryubing)",
        );

        assert_eq!(map.get(&4143722054).map(String::as_str), Some("Motrix"));
        assert!(
            !map.contains_key(&4249995176),
            "a rename to the launcher's own name must not be recorded"
        );
    }

    /// Renaming back to the launcher's own name is how a rename is cleared, and
    /// it has to clear it even when the entry is surrounded by others.
    #[test]
    fn renaming_to_the_original_name_clears_only_that_entry() {
        let mut map = HashMap::new();
        map.insert(1, "Hades".to_string());
        map.insert(2, "Hades II".to_string());
        map.insert(3, "Other".to_string());

        apply_rename(&mut map, 1, "Hades", "Hades");

        assert!(!map.contains_key(&1));
        assert_eq!(map.len(), 2);
    }
}
