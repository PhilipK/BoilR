use std::path::Path;

use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

use crate::config::get_boilr_links_path;

pub fn create_sym_links(shortcut: &ShortcutOwned) -> ShortcutOwned {
    let links_folder = get_boilr_links_path();

    let target_link = links_folder.join(format!("t{}", shortcut.app_id));
    let workdir_link = links_folder.join(format!("w{}", shortcut.app_id));

    let target_original = Path::new(&shortcut.exe);
    let workdir_original = Path::new(&shortcut.start_dir);

    use std::os::unix::fs::symlink;
    // If the links exsists, then they must point towards what is needed, otherwise they would have a different app id
    let target_ok = target_link.exists() || symlink(target_original, &target_link).is_ok();
    let workdir_ok = workdir_link.exists() || symlink(workdir_original, &workdir_link).is_ok();
    match (target_ok, workdir_ok) {
        (true, true) => {
            let exe = target_link.to_string_lossy().to_string();
            let start_dir = workdir_link.to_string_lossy().to_string();

            let icon = if shortcut.icon.eq(&shortcut.exe) {
                &exe
            } else {
                &shortcut.icon
            };

            let new_shortcut = Shortcut::new(
                "0",
                shortcut.app_name.as_str(),
                exe.as_str(),
                start_dir.as_str(),
                icon.as_str(),
                shortcut.shortcut_path.as_str(),
                shortcut.launch_options.as_str(),
            );
            let mut new_shortcut = new_shortcut.to_owned();
            new_shortcut.tags = shortcut.tags.clone();
            new_shortcut.dev_kit_game_id = shortcut.dev_kit_game_id.clone();
            new_shortcut
        }
        _ => {
            println!("Could not create symlinks for game: {}", shortcut.app_name);
            shortcut.clone()
        }
    }
}

pub fn ensure_links_folder_created(name: &str) {
    let boilr_links_path = get_boilr_links_path();
    if !boilr_links_path.exists() {
        if let Err(e) = std::fs::create_dir_all(&boilr_links_path) {
            println!(
                "Could not create links folder for symlinks at path: {boilr_links_path:?} , error: {e:?} , you can try to disable creating symlinks for platform {name}"
            );
        }
    }
}
