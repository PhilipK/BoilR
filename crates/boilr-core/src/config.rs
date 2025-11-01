use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
};

#[cfg(target_family = "unix")]
pub fn get_config_folder() -> PathBuf {
    let config_home = std::env::var("XDG_CONFIG_HOME");
    let home = std::env::var("HOME");
    match (config_home, home) {
        (Ok(p), _) => Path::new(&p).join("boilr"),
        (Err(_), Ok(home)) => Path::new(&home).join(".config").join("boilr"),
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

pub fn get_renames_file() -> PathBuf {
    get_config_folder().join("renames.json")
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
    get_config_folder().join("links")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::get_config_folder;

    #[test]
    #[cfg(target_family = "unix")]
    fn check_return_xdg_config_path() {
        // Parameters for set environment 'XDG_CONFIG_HOME'
        std::env::set_var(
            "XDG_CONFIG_HOME",
            std::env::var("HOME").unwrap_or_default() + "/.config",
        );

        let xdg_config_home = std::env::var("XDG_CONFIG_HOME").unwrap_or_default() + "/boilr";
        let config_path = get_config_folder();

        assert_eq!(config_path, PathBuf::from(xdg_config_home));
    }

    #[test]
    #[cfg(target_family = "unix")]
    fn check_return_config_path() {
        std::env::set_var(
            "XDG_CONFIG_HOME",
            std::env::var("HOME").unwrap_or_default() + "/.config",
        );

        let config_path = get_config_folder();
        let current_path = std::env::var("HOME").unwrap_or_default() + "/.config/boilr";

        assert_eq!(config_path, PathBuf::from(current_path));
    }
}
