use std::{collections::HashMap, error::Error};

use boilr_core::config::get_renames_file;

pub fn load_rename_map() -> HashMap<u32, String> {
    try_load_rename_map().unwrap_or_default()
}

pub fn try_load_rename_map() -> Result<HashMap<u32, String>, Box<dyn Error>> {
    let rename_map = get_renames_file();
    let file_content = std::fs::read_to_string(rename_map)?;
    let deserialized = serde_json::from_str(&file_content)?;
    Ok(deserialized)
}
