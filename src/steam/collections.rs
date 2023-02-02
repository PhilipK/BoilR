use nom::FindSubstring;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusty_leveldb::{LdbIterator, Options, WriteBatch, DB};

const BOILR_TAG: &str = "boilr";

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
    fn new<B: AsRef<str>>(name: B, ids: &[usize]) -> Self {
        let name = name.as_ref();
        let key = format!("user-collections.{}", name_to_key(name));
        let value = serialize_collection_value(name, ids);
        let start = SystemTime::now();
        let since_the_epoch = start.duration_since(UNIX_EPOCH).unwrap_or_default();
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
            .contains(&format!("user-collections.{BOILR_TAG}"))
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
    fn new<S: AsRef<str>>(name: S, game_ids: &[usize]) -> Self {
        let name = name.as_ref();
        let id = name_to_key(name);

        ValueCollection {
            id,
            name: name.to_string(),
            added: game_ids.to_vec(),
            removed: vec![],
        }
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
    collections_to_add: &[Collection],
) -> Result<(), Box<dyn Error>> {
    let steam_user_id = steam_user_id.as_ref();
    let new_collections: Vec<(String, SteamCollection)> = collections_to_add
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
        save_category(category_key, collections, &mut write_batch)?;

        if let Some(path) = get_vdf_path(steam_user_id) {
            let content = std::fs::read_to_string(&path)
                .ok()
                .and_then(parse_vdf_collection);
            if let Some(mut vdf_collections) = content {
                let boilr_keys: Vec<String> = vdf_collections
                    .keys()
                    .filter(|k| k.contains(BOILR_TAG))
                    .cloned()
                    .collect();
                for key in boilr_keys {
                    vdf_collections.remove(&key);
                }

                let new_vdfs = collections_to_add.iter().map(|collection| {
                    let key = name_to_key(&collection.name);

                    VdfCollection {
                        id: key,
                        added: collection.game_ids.clone(),
                        removed: vec![],
                    }
                });
                for new_vdf in new_vdfs {
                    vdf_collections.insert(new_vdf.id.clone(), new_vdf.clone());
                }

                let new_string = write_vdf_collection_to_string(
                    path.clone().to_string_lossy(),
                    &vdf_collections,
                );
                if let Some(new_string) = new_string {
                    std::fs::write(path, new_string)?;
                }
            }
        }
    }

    db.write(write_batch, true)?;

    Ok(())
}

#[cfg(target_family = "unix")]
fn get_vdf_path<S: AsRef<str>>(steamid: S) -> Option<PathBuf> {
    match std::env::var("HOME") {
        Ok(home) => {
            let path = Path::new(&home)
                .join(".steam")
                .join("steam")
                .join("userdata")
                .join(steamid.as_ref())
                .join("config")
                .join("localconfig.vdf");
            if path.exists() {
                return Some(path);
            }
            None
        }
        Err(_e) => None,
    }
}

#[cfg(target_os = "windows")]
fn get_vdf_path<S: AsRef<str>>(steamid: S) -> Option<PathBuf> {
    match std::env::var("PROGRAMFILES(X86)") {
        Ok(program_files) => {
            let path = Path::new(&program_files)
                .join("Steam")
                .join("userdata")
                .join(steamid.as_ref())
                .join("config")
                .join("localconfig.vdf");
            if path.exists() {
                Some(path)
            } else {
                None
            }
        }
        Err(_e) => None,
    }
}

fn save_category<S: AsRef<str>>(
    category_key: S,
    category: Vec<(String, SteamCollection)>,
    batch: &mut WriteBatch,
) -> Result<(), Box<dyn Error>> {
    let json = serde_json::to_string(&category)?;
    let prefixed = format!("\u{1}{json}");
    batch.put(category_key.as_ref().as_bytes(), prefixed.as_bytes());
    Ok(())
}

type CollectionCategories = HashMap<String, Vec<(String, SteamCollection)>>;

fn get_categories<S: AsRef<str>>(
    steamid: S,
    db: &mut DB,
) -> Result<CollectionCategories, Box<dyn Error>> {
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

fn open_db() -> eyre::Result<DB> {
    use eyre::eyre;
    let location = get_level_db_location().ok_or(eyre!("Collections db not found"))?;
    let options = Options::default();
    let open_res = DB::open(location, options);
    open_res.map_err(|e|{
        use rusty_leveldb::StatusCode::*;
        match e.code{
            LockError => eyre!("Could not lock the steam level database, make sure steam is turned off when running synchronizations"),
            NotFound => eyre!("Could not find the steam level database, try to open and close steam once and synchronize again"),
            _ => eyre!("Failed opening collections file: {}",e.err),
        }
    })
}

fn get_namespace_keys<S: AsRef<str>>(steamid: S, db: &mut DB) -> HashSet<String> {
    let keyprefix = get_steam_user_prefix(steamid);

    let namespaces_key = format!("{keyprefix}s");
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
        Some(got) => String::from_utf8_lossy(got.as_slice())
            .get(1..)
            .and_then(|s| serde_json::from_str(s).ok()),
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
                return Some(path);
            }
            None
        }
        Err(_e) => None,
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
                Some(path)
            } else {
                None
            }
        }
        Err(_e) => None,
    }
}

fn serialize_collection_value<S: AsRef<str>>(name: S, game_ids: &[usize]) -> String {
    let value = ValueCollection::new(name, game_ids);
    serde_json::to_string(&value).unwrap_or_default()
}

fn name_to_key<S: AsRef<str>>(name: S) -> String {
    use base64::{Engine as _, engine::general_purpose};
    let base64 = general_purpose::STANDARD_NO_PAD.encode(name.as_ref());
    let base64_no_end = if base64.ends_with("==") {
        base64.get(..base64.len() - 2).unwrap_or_default()
    } else {
        &base64
    };
    format!("{BOILR_TAG}-{base64_no_end}")
}

fn parse_steam_collections<S: AsRef<str>>(
    input: S,
) -> Result<Vec<(String, SteamCollection)>, Box<dyn Error>> {
    let input = input.as_ref();
    let input = input.strip_prefix('\u{1}').unwrap_or(input);
    let res = serde_json::from_str::<Vec<(String, SteamCollection)>>(input)?;
    Ok(res)
}

fn get_steam_user_prefix<S: AsRef<str>>(steamid: S) -> String {
    let keyprefix = format!(
        "_https://steamloopback.host\u{0000}\u{0001}U{}-cloud-storage-namespace",
        steamid.as_ref()
    );
    keyprefix
}

pub fn parse_vdf_collection<S: AsRef<str>>(input: S) -> Option<HashMap<String, VdfCollection>> {
    let input = input.as_ref();
    serde_json::from_str(input).ok()
}

pub fn write_vdf_collection_to_string<S: AsRef<str>>(
    input: S,
    vdf: &HashMap<String, VdfCollection>,
) -> Option<String> {
    let input = input.as_ref();
    if let Ok(str) = serde_json::to_string(vdf) {
        let encoded_json = format!("\"{}\"", str.replace('\\', "\\\""));
        let key = "\t\"user-collections\"\t\t";
        if let Some(start_index) = input.find_substring(key) {
            let start_index_plus_key = start_index + key.len();
            if let Some(line_index) = input.get(start_index_plus_key..).and_then(|i| i.find('\n')) {
                let end_index_in_full = line_index + start_index_plus_key;
                if let (Some(before), Some(after)) = (
                    input.get(..start_index_plus_key),
                    input.get(end_index_in_full..),
                ) {
                    let result = format!("{before}{encoded_json}{after}");
                    return Some(result);
                }
            }
        }
    }
    None
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VdfCollection {
    id: String,
    added: Vec<usize>,
    removed: Vec<usize>,
}

#[cfg(test)]
mod tests {
    //Allow unwraps in test
    #![allow(clippy::unwrap_in_result)]
    #![allow(clippy::get_unwrap)]
    #![allow(clippy::unwrap_used)]
    use super::*;

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
    fn can_parse_categories() {
        let input = include_str!("../testdata/leveldb/testcollections.json");
        let collection = parse_steam_collections(input).unwrap();
        assert_eq!(28, collection.len())
    }
}
