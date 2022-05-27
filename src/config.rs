use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
};

#[cfg(target_family = "unix")]
pub fn get_config_folder() -> PathBuf {
    let config_home = std::env::var("XDG_CONFIG_HOME");
    let home = std::env::var("HOME");
    match (config_home, home) {
        (Ok(p), _) => Path::new(&p).to_path_buf(),
        (Err(_), Ok(home)) => Path::new(&home).join(".config").join("boilr").to_path_buf(),
        _ => Path::new("").to_path_buf(),
    }
}

#[cfg(windows)]
pub fn get_config_folder() -> PathBuf {
    let config_home = std::env::var("APPDATA");
    match config_home {
        Ok(p) => Path::new(&p).join("boilr"),
        Err(_) => Path::new("").to_path_buf(),
    }
}

pub fn get_thumbnails_folder() -> PathBuf {
    let thumbnails_path = get_config_folder().join("thumbnails");
    let _ = create_dir_all(&thumbnails_path);
    thumbnails_path
}

pub fn get_config_file() -> PathBuf {
    get_config_folder().join("config.toml")
}

pub fn get_cache_file() -> PathBuf {
    get_config_folder().join("cache.json")
}

pub fn get_backups_flder() -> PathBuf {
    let backups_path = get_config_folder().join("backup");
    let _ = create_dir_all(&backups_path);
    backups_path
}

#[cfg(target_family = "unix")]
pub fn get_boilr_links_path() -> PathBuf {
    get_config_folder().join("links").to_path_buf()
}
