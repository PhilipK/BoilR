use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusty_leveldb::{ LdbIterator, Options, DB, WriteBatch};

const BOILR_TAG: &'static str = "boilr";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
enum SteamCollection {
    Actual(ActualSteamCollection),
    Deleted(DeletedCollection),
}

impl SteamCollection {
    pub fn is_boilr_collection(&self) -> bool {
        match self {
            SteamCollection::Actual(actual) => actual.is_boilr_collection(),
            SteamCollection::Deleted(_) => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ActualSteamCollection {
    key: String,
    timestamp: u64,
    value: String, //For custom collections, this value is a ValueCollection json serialized
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

impl ActualSteamCollection {
    fn new<B: AsRef<str>>(name: B, ids: &Vec<usize>) -> Self {
        let name = name.as_ref();
        let key = format!("user-collections.{}", name_to_key(name));
        let value = serialize_collection_value(name, ids);
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        let timestamp = since_the_epoch.as_secs();

        ActualSteamCollection {
            key,
            timestamp,
            value,
            conflict_resolution_method: Some("custom".to_string()),
            str_method_id: Some("union-collections".to_string()),
            version: None,
        }
    }

    pub fn is_boilr_collection(&self) -> bool {
        self.key
            .starts_with(&format!("user-collections.{}", BOILR_TAG))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ValueCollection {
    id: String,
    name: String,
    added: Vec<usize>,
    removed: Vec<usize>,
}

impl ValueCollection {
    fn new<S: AsRef<str>>(name: S, game_ids: &Vec<usize>) -> Self {
        let name = name.as_ref();
        let id = name_to_key(name);
        let value = ValueCollection {
            id,
            name: name.to_string(),
            added: game_ids.clone(),
            removed: vec![],
        };
        value
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]

struct DeletedCollection {
    key: String,
    timestamp: usize,
    is_deleted: bool,
    version: String,
}

pub struct Collection {
    pub name: String,
    pub game_ids: Vec<usize>,
}

pub fn write_collections<S: AsRef<str>>(
    steam_user_id: S,
    collections: &Vec<Collection>,
) -> Result<(), Box<dyn Error>> {
    let steam_user_id = steam_user_id.as_ref();
    let new_collections: Vec<(String, SteamCollection)> = collections
        .iter()
        .map(|c| {
            let collection = ActualSteamCollection::new(&c.name, &c.game_ids);
            (collection.key.clone(), SteamCollection::Actual(collection))
        })
        .collect();

    let mut db = open_db()?;

    let current_categories = get_categories(steam_user_id, &mut db)?;
    //this is a collection of collections, known as a category
    let mut write_batch = WriteBatch::new();

    for (category_key, mut collections) in current_categories {
        collections.retain(|(_key, collection)| !collection.is_boilr_collection());
        collections.extend(new_collections.clone());
        save_category(category_key, collections, &mut write_batch)?
    }
    db.write(write_batch,true)?;

    Ok(())
}

fn save_category<S: AsRef<str>>(
    category_key: S,
    category: Vec<(String, SteamCollection)>,
    batch : &mut WriteBatch,
) -> Result<(), Box<dyn Error>> {
    let json = serde_json::to_string(&category)?;
    let prefixed = format!("\u{00}{}",json);
    batch.put(category_key.as_ref().as_bytes(),prefixed.as_bytes());
    Ok(())
}

fn get_categories<S: AsRef<str>>(
    steamid: S,
    db: &mut DB,
) -> Result<HashMap<String, Vec<(String, SteamCollection)>>, Box<dyn Error>> {
    let namespace_keys = get_namespace_keys(steamid, db);
    let mut db_iter = db.new_iter()?;
    let mut res = HashMap::new();
    while let Some((key_bytes, data_bytes)) = db_iter.next() {
        let key = String::from_utf8_lossy(&key_bytes).to_string();
        //make sure that what we are looking at is a collection
        //there are other things in this db as well
        if namespace_keys.contains(&key) {                      
            let data = String::from_utf8_lossy(&data_bytes);
            let collections = parse_steam_collections(&data)?;
            res.insert(key, collections);
        }
    }
    Ok(res)
}

fn open_db() -> Result<DB, Box<dyn Error>> {
    let location = get_level_db_location();
    if let None = location {
        todo!()
    };
    let options = Options::default();
    Ok(DB::open(location.unwrap(),options )?)
}

fn get_namespace_keys<S: AsRef<str>>(steamid: S, db: &mut DB) -> HashSet<String> {
    let keyprefix = get_steam_user_prefix(steamid);

    let namespaces_key = format!("{}s", keyprefix);
    let key_bytes = namespaces_key.as_bytes();
    let namespaces = get_namespaces(db, key_bytes).unwrap_or_default();
    let namespace_keys = namespaces
        .iter()
        .map(|c| format!("{}-{}", keyprefix, c.0))
        .collect();
    namespace_keys
}

fn get_namespaces(db: &mut DB, key_bytes: &[u8]) -> Option<Vec<(i32, String)>> {
    match db.get(key_bytes) {
        Some(got) => {
            let collection_bytes = got.as_slice();
            let collectin_str = String::from_utf8_lossy(&collection_bytes)[1..].to_string();
            let collection = serde_json::from_str(&collectin_str).unwrap_or_default();
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
                .join("steam")
                .join("config")
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

fn serialize_collection_value<S: AsRef<str>>(name: S, game_ids: &Vec<usize>) -> String {
    let value = ValueCollection::new(name, game_ids);
    let value_json = serde_json::to_string(&value).expect("Should be able to serialize known type");
    value_json
}

fn name_to_key<S: AsRef<str>>(name: S) -> String {
    let config = base64::Config::new(base64::CharacterSet::Standard, false);
    let base64 = base64::encode_config(name.as_ref(), config);
    let key = format!("{}-{}", BOILR_TAG, base64);
    key
}

fn parse_steam_collections<S: AsRef<str>>(
    input: S,
) -> Result<Vec<(String, SteamCollection)>, Box<dyn Error>> {
    let input = input.as_ref();
    let input = if input.starts_with("\u{1}") {
        input[1..].to_string()
    } else {
        input.to_string()
    };
    let res = serde_json::from_str::<Vec<(String, SteamCollection)>>(&input)?;
    Ok(res)
}

fn serialize_steam_collections(input: Vec<(String, SteamCollection)>) -> String {
    let res = serde_json::to_string(&input).unwrap();
    format!("\u{1}{}", res)
}

fn local_key_to_global<A: AsRef<str>, B: AsRef<str>>(steamid: A, local_key: B) -> String {
    let user_prefix = get_steam_user_prefix(steamid);
    format!("{}-{}", user_prefix, local_key.as_ref())
}

fn get_steam_user_prefix<S: AsRef<str>>(steamid: S) -> String {
    let keyprefix = format!(
        "_https://steamloopback.host\u{0000}\u{0001}U{}-cloud-storage-namespace",
        steamid.as_ref()
    );
    keyprefix
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn write_collections_test() {
    //     let steamid = "10342635";
    //     let colllections = vec![Collection {
    //         name: "Test Collection".to_string(),
    //         game_ids: vec![265930, 751780, 433340, 361420, 337340, 1055540],
    //     }];
    //     write_collections(
    //         steamid,
    //         &colllections,
    //     ).unwrap();
    // }

    #[test]
    fn can_serialize_collection() {
        let games = vec![312200];
        let mut collection = ActualSteamCollection::new("Itch", &games);
        collection.timestamp = 1647763452;
        let json = serde_json::to_string(&collection).unwrap();
        let expected = include_str!("../testdata/leveldb/test_collection.json");
        assert_eq!(json, expected);
    }

    #[test]
    fn can_serialize_collection_inner() {
        let games = vec![312200];
        let res = serialize_collection_value("Itch", &games);
        let expected = include_str!("../testdata/leveldb/test_collection_value.json");
        assert_eq!(res, expected);
    }

    #[test]
    fn can_serialize_and_deserialize() {
        let input = include_str!("../testdata/leveldb/testcollections.json");
        let parsed = parse_steam_collections(input).unwrap();
        let serialized = serialize_steam_collections(parsed);
        assert_eq!(input, &serialized);
    }

    #[test]
    fn can_parse_categories() {
        let input = include_str!("../testdata/leveldb/testcollections.json");
        let collection = parse_steam_collections(input).unwrap();
        assert_eq!(28, collection.len())
    }
}
