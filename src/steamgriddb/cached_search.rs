use dashmap::DashMap;
use std::{fs::File, io::Write};

use crate::config::get_cache_file;

type SearchMap = DashMap<u32, (String, usize)>;

pub struct CachedSearch<'a> {
    search_map: SearchMap,
    client: &'a steamgriddb_api::Client,
}

impl<'a> CachedSearch<'a> {
    pub fn new(client: &'a steamgriddb_api::Client) -> CachedSearch<'a> {
        CachedSearch {
            search_map: get_search_map(),
            client,
        }
    }

    pub fn save(&self) {
        if let Err(err) = save_search_map(&self.search_map) {
            eprintln!("Failed saving searchmap : {err:?}");
        }
    }

    pub fn set_cache<S>(&mut self, app_id: u32, name: S, new_grid_id: usize)
    where
        S: Into<String>,
    {
        self.search_map.insert(app_id, (name.into(), new_grid_id));
        self.save();
    }

    pub async fn search<S>(
        &self,
        app_id: u32,
        query: S,
    ) -> Result<Option<usize>, Box<dyn std::error::Error>>
    where
        S: AsRef<str> + Into<String>,
    {
        let cached_result = self.search_map.get(&app_id);
        if let Some(result) = cached_result {
            return Ok(Some(result.1));
        }
        println!("Searching for {}", query.as_ref());
        let search = self.client.search(query.as_ref()).await?;
        let first_id = search.first().map(|f| f.id);
        match first_id {
            Some(assumed_id) => {
                self.search_map.insert(app_id, (query.into(), assumed_id));
                Ok(Some(assumed_id))
            }
            None => Ok(None),
        }
    }
}

fn get_search_map() -> SearchMap {
    let path = get_cache_file();
    if path.exists() {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|string| serde_json::from_str::<SearchMap>(&string).ok())
            .unwrap_or_default()
    } else {
        SearchMap::new()
    }
}

fn save_search_map(search_map: &SearchMap) -> eyre::Result<()> {
    let string = serde_json::to_string(search_map)?;
    let path = get_cache_file();
    let mut file = File::create(path)?;
    file.write_all(string.as_bytes())?;
    Ok(())
}
