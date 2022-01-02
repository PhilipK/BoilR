use dashmap::DashMap;
use std::{fs::File, io::Write, path::Path};

type SearchMap = DashMap<u32, (String, usize)>;

pub struct CachedSearch<'a> {
    search_map: SearchMap,
    client: &'a steamgriddb_api::Client,
}

impl<'a> CachedSearch<'a> {
    pub fn new(client: &steamgriddb_api::Client) -> CachedSearch {
        CachedSearch {
            search_map: get_search_map(),
            client,
        }
    }

    pub fn save(&self) {
        save_search_map(&self.search_map);
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
        if search.is_empty() {
            return Ok(None);
        }
        let first_item = &search[0];
        let assumed_id = first_item.id;
        self.search_map.insert(app_id, (query.into(), assumed_id));

        Ok(Some(assumed_id))
    }
}

fn get_search_map() -> SearchMap {
    let path = Path::new("cache.json");
    if path.exists() {
        let string = std::fs::read_to_string(path).unwrap();
        serde_json::from_str::<SearchMap>(&string).expect("Failed to parse cache.json")
    } else {
        SearchMap::new()
    }
}

fn save_search_map(search_map: &SearchMap) {
    let string = serde_json::to_string(search_map).unwrap();
    let mut file = File::create("cache.json").unwrap();
    file.write_all(string.as_bytes()).unwrap();
}
