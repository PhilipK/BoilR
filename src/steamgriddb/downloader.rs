use std::fs::File;
use std::io::Write;
use std::{collections::HashMap, path::Path};

use std::error::Error;
use steam_shortcuts_util::shortcut::ShortcutOwned;
use steamgriddb_api::Client;

use crate::steamgriddb::ImageType;

use super::CachedSearch;

pub async fn download_images<'b>(
    known_images: Vec<String>,
    user_data_folder: &str,
    shortcuts: &Vec<ShortcutOwned>,
    search: &mut CachedSearch<'b>,
    client: &Client,
) -> Result<(), Box<dyn Error>> {
    let shortcuts_to_search_for = shortcuts.iter().filter(|s| {
        let images = vec![
            format!("{}_hero.png", s.app_id),
            format!("{}p.png", s.app_id),
            format!("{}_logo.png", s.app_id),
        ];
        // if we are missing any of the images we need to search for them
        images.iter().any(|image| !known_images.contains(&image)) && "" != s.app_name
    });
    if shortcuts_to_search_for.clone().count() == 0 {
        return Ok(());
    }
    let mut search_results = HashMap::new();
    for s in shortcuts_to_search_for {
        let search = search.search(s.app_id, &s.app_name).await?;
        if let Some(search) = search {
            search_results.insert(s.app_id, search);
        }
    }
    let types = vec![ImageType::Logo, ImageType::Hero, ImageType::Grid];
    Ok(for image_type in types {
        let mut images_needed = shortcuts
            .iter()
            .filter(|s| search_results.contains_key(&s.app_id))
            .filter(|s| !known_images.contains(&image_type.file_name(s.app_id)));
        let image_ids: Vec<usize> = images_needed
            .clone()
            .filter_map(|s| search_results.get(&s.app_id))
            .map(|search| *search)
            .collect();

        let query_type = match image_type {
            ImageType::Hero => steamgriddb_api::query_parameters::QueryType::Hero(None),
            ImageType::Grid => steamgriddb_api::query_parameters::QueryType::Grid(None),
            ImageType::Logo => steamgriddb_api::query_parameters::QueryType::Logo(None),
        };

        match client
            .get_images_for_ids(image_ids.as_slice(), &query_type)
            .await
        {
            Ok(images) => {
                for image in images {
                    if let Some(shortcut) = images_needed.next() {
                        if let Ok(image) = image {
                            let grid_folder = Path::new(user_data_folder).join("config/grid");
                            let path = grid_folder.join(image_type.file_name(shortcut.app_id));
                            println!(
                                "Downloading {} to {}",
                                image.url,
                                path.as_path().to_str().unwrap()
                            );
                            let mut file = File::create(path).unwrap();
                            let response = reqwest::get(image.url).await?;
                            let content = response.bytes().await?;
                            file.write_all(&content).unwrap();
                        }
                    }
                }
            }
            Err(err) => println!("Error getting images: {}", err),
        }
    })
}
