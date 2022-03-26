use nom::FindSubstring;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusty_leveldb::{LdbIterator, Options, WriteBatch, DB};

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
            .contains(&format!("user-collections.{}", BOILR_TAG))
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
    collections_to_add: &Vec<Collection>,
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
            let content = std::fs::read_to_string(&path).expect("Should be able to read this file");
            if let Some(mut vdf_collections) = parse_vdf_collection(content) {
                let boilr_keys: Vec<String> = vdf_collections
                    .keys()
                    .filter(|k| k.contains(BOILR_TAG))
                    .map(|k| k.clone())
                    .collect();
                for key in boilr_keys {
                    vdf_collections.remove(&key);
                }

                let new_vdfs = collections_to_add.iter().map(|collection| {
                    let key = name_to_key(&collection.name);
                    let vd_collection = VdfCollection {
                        id: key,
                        added: collection.game_ids.clone(),
                        removed: vec![],
                    };
                    vd_collection
                });
                for new_vdf in new_vdfs {
                    vdf_collections.insert(new_vdf.id.clone(), new_vdf.clone());
                }

                let new_string = write_vdf_collection_to_string(
                    &path.clone().to_string_lossy(),
                    &vdf_collections,
                );
                if let Some(new_string) = new_string {
                    std::fs::write(path, new_string).unwrap();
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
                return Some(path.to_path_buf());
            }
            return None;
        }
        Err(e) => return None,
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
                Some(path.to_path_buf())
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
    let prefixed = format!("\u{1}{}", json);
    batch.put(category_key.as_ref().as_bytes(), prefixed.as_bytes());
    Ok(())
}

fn category_to_bytes(category: Vec<(String, SteamCollection)>) -> Result<Vec<u8>, Box<dyn Error>> {
    let json = serde_json::to_string(&category)?;
    let prefixed = format!("\u{1}{}", json);
    Ok(prefixed.as_bytes().to_vec())
}

fn get_categories_data<S: AsRef<str>>(
    steamid: S,
    db: &mut DB,
) -> Result<HashMap<String, Vec<u8>>, Box<dyn Error>> {
    let namespace_keys = get_namespace_keys(steamid, db);
    let mut db_iter = db.new_iter()?;
    let mut res = HashMap::new();
    while let Some((key_bytes, data_bytes)) = db_iter.next() {
        let key = String::from_utf8_lossy(&key_bytes).to_string();
        //make sure that what we are looking at is a collection
        //there are other things in this db as well
        if namespace_keys.contains(&key) {
            res.insert(key, data_bytes.to_vec());
        }
    }
    Ok(res)
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
    Ok(DB::open(location.unwrap(), options)?)
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

fn get_steam_user_prefix<S: AsRef<str>>(steamid: S) -> String {
    let keyprefix = format!(
        "_https://steamloopback.host\u{0000}\u{0001}U{}-cloud-storage-namespace",
        steamid.as_ref()
    );
    keyprefix
}

fn get_collection_part<S: AsRef<str>>(input: S) -> Option<String> {
    let input = input.as_ref();
    let key = "\t\"user-collections\"\t\t";
    if let Some(start_index) = input.find_substring(key) {
        let start_index = start_index + key.len();
        if let Some(line_index) = input[start_index..].find("\n") {
            let encoded_json = input[start_index..][..line_index].to_string();
            let json = encoded_json.replace("\\\"", "\"");
            return Some(json.trim_matches('"').to_string());
        }
    }
    None
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
    let str = serde_json::to_string(vdf).expect("Should be able to serialize known type");
    let encoded_json = format!("\"{}\"", str.replace("\"", "\\\""));
    let key = "\t\"user-collections\"\t\t";
    if let Some(start_index) = input.find_substring(key) {
        let start_index_plus_key = start_index + key.len();
        if let Some(line_index) = input[start_index_plus_key..].find("\n") {
            let end_index_in_full = line_index + start_index_plus_key;
            let result = format!(
                "{}{}{}",
                input[..start_index_plus_key].to_string(),
                encoded_json,
                input[end_index_in_full..].to_string()
            );
            return Some(result);
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
    fn get_collection_part_test() {
        let vdf_text = std::fs::read_to_string(
            "/home/philip/.steam/steam/userdata/10342635/config/localconfig.vdf",
        )
        .unwrap();
        let collection_part = get_collection_part(&vdf_text).unwrap();
        let collection = parse_vdf_collection(collection_part).unwrap();
        let new_string = write_vdf_collection_to_string(&vdf_text, &collection).unwrap();
        assert_eq!(vdf_text, new_string);
    }

    #[test]
    fn same_bytes() {
        let mut db = open_db().unwrap();
        let categores_data = get_categories_data("10342635", &mut db).unwrap();
        for (_key, data_bytes) in categores_data {
            let data_string = String::from_utf8_lossy(&data_bytes);
            let collections = parse_steam_collections(&data_string).unwrap();
            let new_bytes = category_to_bytes(collections).unwrap();
            assert_eq!(data_bytes.to_vec(), new_bytes);
        }
    }

    // #[test]
    // fn can_parse_vdf() {
    //     use keyvalues_parser::Vdf;
    //     let vdf_text = std::fs::read_to_string(
    //         "/home/philip/.steam/steam/userdata/10342635/config/localconfig.vdf",
    //     )
    //     .unwrap();
    //     let mut vdf = Vdf::parse(&vdf_text).unwrap();
    //     vdf.
    //     match vdf.value {
    //         keyvalues_parser::Value::Str(str) => todo!(),
    //         keyvalues_parser::Value::Obj(obj) => {
    //             let localstore = obj.get("UserLocalConfigStore").unwrap();
    //             let first = localstore.iter().find(|v| match v {
    //                 keyvalues_parser::Value::Str(str) => false,
    //                 keyvalues_parser::Value::Obj(obj) => obj.get("WebStorage").is_some(),
    //             });

    //             let first = first.unwrap().unwrap_obj();
    //         }
    //     };
    //     // vdf.keys().first(|k| k == "UserLocalConfigStore").unwrap();
    //     // localConfig.UserLocalConfigStore.WebStorage
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
