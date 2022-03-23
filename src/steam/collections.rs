use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::{Path, PathBuf};

use rusty_leveldb::{DBIterator, LdbIterator, Options, DB};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SteamCollection {
    key: String,
    timestamp: usize,
    value: String,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "conflictResolutionMethod"
    )]
    conflict_resolution_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "strMethodId")]
    str_method_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]

pub struct DeletedCollection {
    key: String,
    timestamp: usize,
    is_deleted: bool,
    version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum SteamCollectionType {
    Actual(SteamCollection),
    Deleted(DeletedCollection),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InnerCollection {
    id: String,
    name: String,
    added: Vec<usize>,
    removed: Vec<usize>,
}

fn get_categories_data<S: AsRef<str>>(
    steamid: S,
) -> Result<HashMap<String, String>, Box<dyn Error>> {
    let steamid = steamid.as_ref();

    let keyprefix = format!(
        "_https://steamloopback.host\u{0000}\u{0001}U{}-cloud-storage-namespace",
        steamid
    );

    let location = get_level_db_location();
    if let None = location {
        todo!();
    };

    let mut db = DB::open(location.unwrap(), Options::default())?;
    let namespace_keys = get_namespace_keys(&keyprefix, &mut db);

    let mut db_iter = db.new_iter()?;
    let mut res = HashMap::new();
    while let Some(item) = db_iter.next() {
        let key = String::from_utf8_lossy(&item.0).to_string();
        if namespace_keys.contains(&key) {
            let prefix = format!("{}-", &keyprefix);
            let id = key.replace(prefix.as_str(), "");
            let data = String::from_utf8_lossy(&item.1).to_string();
            res.insert(id, data);
        }
    }
    Ok(res)
}

fn get_namespace_keys<S: AsRef<str>>(keyprefix: &S, db: &mut DB) -> HashSet<String> {
    let namespaces_key = format!("{}s", keyprefix.as_ref());
    let key_bytes = namespaces_key.as_bytes();
    let collections = get_collections(db, key_bytes).unwrap_or_default();
    let namespace_keys = collections
        .iter()
        .map(|c| format!("{}-{}", keyprefix.as_ref(), c.0))
        .collect();
    namespace_keys
}

fn get_collections(db: &mut DB, key_bytes: &[u8]) -> Option<Vec<(i32, String)>> {
    match db.get(key_bytes) {
        Some(got) => {
            let collection_bytes = got.as_slice();
            let collectin_str = String::from_utf8_lossy(&collection_bytes)[1..].to_string();
            let collection = parse_collections(collectin_str);
            Some(collection)
        }
        _ => None,
    }
}

#[cfg(target_family = "unix")]
fn get_level_db_location() -> Option<PathBuf> {
    match std::env::var("HOME") {
        Ok(home) => {
            let path = Path::new(&home)
                .join(".steam")
                .join("Steam")
                .join("htmlcache")
                .join("Local Storage")
                .join("leveldb");
            if path.exists() {
                return Some(path.to_path_buf());
            }
            return None;
        }
        Err(e) => return None,
    }
}

#[cfg(target_os = "windows")]
fn get_level_db_location() -> Option<PathBuf> {
    match std::env::var("LOCALAPPDATA") {
        Ok(localdata) => {
            let path = Path::new(&localdata)
                .join("Steam")
                .join("htmlcache")
                .join("Local Storage")
                .join("leveldb");
            if path.exists() {
                return Some(path.to_path_buf());
            }
            return None;
        }
        Err(e) => return None,
    }
}

fn serialize_collection_value<S:AsRef<str>>(name:S, game_ids:&Vec<u32>) -> String{
    "".to_string()
}

fn parse_collections<S: AsRef<str>>(input: S) -> Vec<(i32, String)> {
    serde_json::from_str(input.as_ref()).unwrap_or_default()
}

fn parse_steam_collections<S: AsRef<str>>(input: S) -> Vec<(String, SteamCollectionType)> {
    let input = input.as_ref();
    let input = if input.starts_with("\u{1}") {
        input[1..].to_string()
    } else {
        input.to_string()
    };
    serde_json::from_str(&input).unwrap()
}

fn serialize_steam_collections(input: Vec<(String, SteamCollectionType)>) -> String {
    let res = serde_json::to_string(&input).unwrap();
    format!("\u{1}{}", res)
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn can_serialize_collection_inner(){
        let games = vec![312200];
        let res = serialize_collection_value("Itch",&games);
        let expected = include_str!("../testdata/leveldb/test_collection_value.json");
        assert_eq!(res,expected);
    }

    #[test]
    fn can_serialize_and_deserialize() {
        let input = include_str!("../testdata/leveldb/testcollections.json");
        let parsed = parse_steam_collections(input);
        let serialized = serialize_steam_collections(parsed);
        assert_eq!(input, &serialized);
    }

    #[test]
    fn can_parse_categories() {
        let input = include_str!("../testdata/leveldb/testcollections.json");
        let collection = parse_steam_collections(input);
        assert_eq!(28, collection.len())
    }

    #[test]
    fn can_parse_collections_empty() {
        let res = parse_collections("");
        let expected = Vec::<(i32, String)>::new();
        assert_eq!(expected, res);
    }

    #[test]
    fn can_parse_collections_empty_list() {
        let res = parse_collections("[]");
        let expected = Vec::<(i32, String)>::new();
        assert_eq!(expected, res);
    }

    #[test]
    fn can_parse_collections_single() {
        let res = parse_collections("[[1,\"772\"]]");
        assert_eq!(vec![(1, "772".to_string())], res);
    }

    #[test]
    fn can_parse_collections_multiple() {
        let res = parse_collections("[[1,\"772\"],[2,\"773\"]]");
        assert_eq!(vec![(1, "772".to_string()), (2, "773".to_string())], res);
    }
}
