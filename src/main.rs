use crate::steamgriddb::{download_images, CachedSearch};
use std::{fs::File, io::Write, path::Path};
mod egs;
mod legendary;
mod platform;
mod settings;
mod steam;
mod steamgriddb;
use fltk::{app, prelude::*, window::Window};


use crate::{
    egs::EpicPlatform,
    legendary::LegendaryPlatform,
    platform::Platform,
    settings::Settings,
    steam::{get_shortcuts_for_user, get_shortcuts_paths, get_users_images},
};
use std::error::Error;
use steam_shortcuts_util::{shortcut::ShortcutOwned, shortcuts_to_bytes, Shortcut};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app = app::App::default();
    let mut wind = Window::new(100, 100, 400, 300, "Hello from rust");
    wind.end();
    wind.show();
    app.run().unwrap();


    let settings = Settings::new()?;

    let auth_key = settings.steamgrid_db.auth_key;
    if settings.steamgrid_db.enabled && auth_key.is_none() {
        println!("auth_key not found, please add it to the steamgrid_db settings ");
        return Ok(());
    }

    let auth_key = auth_key.unwrap();

    let client = steamgriddb_api::Client::new(auth_key);
    let mut search = CachedSearch::new(&client);

    let userinfo_shortcuts = get_shortcuts_paths(&settings.steam)?;
    println!("Found {} user(s)", userinfo_shortcuts.len());

    for user in userinfo_shortcuts.iter() {
        let shortcut_info = get_shortcuts_for_user(user);

        let mut new_user_shortcuts: Vec<ShortcutOwned> = shortcut_info.shortcuts;

        update_platform_shortcuts(
            &EpicPlatform::new(settings.epic_games.clone()),
            &mut new_user_shortcuts,
        );

        update_platform_shortcuts(
            &LegendaryPlatform::new(settings.legendary.clone()),
            &mut new_user_shortcuts,
        );

        let shortcuts = new_user_shortcuts.iter().map(|f| f.borrow()).collect();

        save_shortcuts(&shortcuts, Path::new(&shortcut_info.path));

        let known_images = get_users_images(user).unwrap();
        download_images(
            known_images,
            user.steam_user_data_folder.as_str(),
            shortcuts,
            &mut search,
            &client,
        )
        .await?;
    }

    search.save();

    Ok(())
}

fn save_shortcuts(shortcuts: &Vec<Shortcut>, path: &Path) {
    let new_content = shortcuts_to_bytes(shortcuts);
    let mut file = File::create(path).unwrap();
    file.write_all(new_content.as_slice()).unwrap();
}

fn update_platform_shortcuts<P, T, E>(platform: &P, current_shortcuts: &mut Vec<ShortcutOwned>)
where
    P: Platform<T, E>,
    E: std::fmt::Debug + std::fmt::Display,
    T: Into<ShortcutOwned>,
{
    if platform.enabled() {
        let shortcuts_to_add_result = platform.get_shortcuts();

        match shortcuts_to_add_result {
            Ok(shortcuts_to_add) => {
                println!(
                    "Found {} games for platform {}",
                    shortcuts_to_add.len(),
                    platform.name()
                );
                current_shortcuts.retain(|f| !f.tags.contains(&platform.name().to_owned()));
                for shortcut in shortcuts_to_add {
                    let shortcut_owned: ShortcutOwned = shortcut.into();
                    current_shortcuts.push(shortcut_owned);
                }
            }
            Err(err) => {
                eprintln!("Error getting shortcuts from platform: {}", platform.name());
                eprintln!("{}", err);
            }
        }
    }
}
