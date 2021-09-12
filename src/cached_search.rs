use std::{collections::HashMap, fs::File, io::Write, path::Path};

type SearchMap = HashMap<u32, usize>;

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

    pub async fn search(
        &mut self,
        app_id: u32,
        query: &str,
    ) -> Result<Option<usize>, Box<dyn std::error::Error>> {
        
        let cached_result = self.search_map.get(&app_id);
        if let Some(result) = cached_result {
            return Ok(Some(*result));
        }

        let search = self.client.search(query).await?;
        if search.is_empty() {
            return Ok(None);
        }
        let first_item = &search[0];
        let assumed_id = first_item.id;
        self.search_map.insert(app_id, assumed_id);

        Ok(Some(assumed_id))
    }
}

fn get_search_map() -> SearchMap {
    let path = Path::new("cache.json");
    if path.exists() {
        let string = std::fs::read_to_string(path).unwrap();
        let search_map =
            serde_json::from_str::<SearchMap>(&string).expect("Failed to parse cache.json");
        search_map
    } else {
        SearchMap::new()
    }
}

fn save_search_map(search_map: &SearchMap) {
    let string = serde_json::to_string(search_map).unwrap();
    let mut file = File::create("cache.json").unwrap();
    file.write_all(string.as_bytes()).unwrap();
}
