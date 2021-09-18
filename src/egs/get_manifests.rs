use std::env::{self, VarError};
use std::fs::{DirEntry, File};
use std::io::BufReader;
use std::path::Path;
use std::error::Error;
use super::ManifestItem;

pub fn get_egs_manifests()  -> Result<Vec<ManifestItem>, Box<dyn Error>> {
    let manifest_dir_path = get_manifest_dir_path()?;
    let manifest_dir_result = std::fs::read_dir(&manifest_dir_path);
    if let Err(err) = manifest_dir_result {
        println!("Could not find manifest directory: {}", manifest_dir_path);
        return Result::Err(Box::new(err));
    }
    let manifest_dir = manifest_dir_result?;
    let manifests = manifest_dir        
        .filter_map(|dir| dir.ok())
        .filter_map(get_manifest_item)
        .filter(is_game_installed);
    Ok(manifests.collect())
}

fn get_manifest_dir_path() -> Result<String, VarError> {
    let key = "SYSTEMDRIVE";
    let system_drive = env::var(key)?;
    Ok(format!(
        "{system_drive}//ProgramData//Epic//EpicGamesLauncher//Data//Manifests",
        system_drive = system_drive
    ))
}

 fn is_game_installed(manifest:&ManifestItem) -> bool{
    Path::new(manifest.manifest_location.as_str()).exists()        
}

 fn get_manifest_item(dir_entry: DirEntry) -> Option<ManifestItem> {
    if let Some(extension) = dir_entry.path().extension() {
        if extension.eq("item") {
            if let Ok(file) = File::open(dir_entry.path()) {
                let reader = BufReader::new(file);
                return serde_json::from_reader(reader).ok();
            }
        }
    }
    None
}
