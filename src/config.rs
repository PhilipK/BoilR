use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
};

fn get_config_folder() -> PathBuf {
    let config_home = std::env::var("XDG_CONFIG_HOME");
    let home = std::env::var("HOME");
    match (config_home, home) {
        (Ok(p), _) => Path::new(&p).to_path_buf(),
        (Err(_), Ok(home)) => Path::new(&home).join(".config").join("boilr").to_path_buf(),
        _ => Path::new("").to_path_buf(),
    }
}

pub fn get_thumbnails_folder() -> PathBuf {
    let thumbnails_path = get_config_folder().join("thumbnails");
    let _ = create_dir_all(&thumbnails_path);
    thumbnails_path.to_path_buf()
}

pub fn get_config_file() -> PathBuf {
    get_config_folder().join("config.toml").to_path_buf()
}

pub fn get_cache_file() -> PathBuf {
    get_config_folder().join("cache.json").to_path_buf()
}

pub fn get_boilr_links_path() -> PathBuf {
    get_config_folder().join("links").to_path_buf()
}
