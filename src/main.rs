use crate::{
    itch::ItchPlatform,
    origin::OriginPlatform,
    steamgriddb::{download_images, CachedSearch},
};
#[cfg(feature = "ui")]
use std::{cell::RefCell, rc::Rc};
use std::{fs::File, io::Write, path::Path};
mod egs;
mod itch;
mod legendary;
mod origin;
mod platform;
mod settings;
mod steam;
mod steamgriddb;
#[cfg(feature = "ui")]
use egs::get_default_location;
#[cfg(feature = "ui")]
use fltk::{app, prelude::*};
#[cfg(feature = "ui")]
mod mainview;

#[cfg(feature = "ui")]
use futures::executor::block_on;

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
    #[cfg(feature = "ui")]
    {
        {
            let app = app::App::default().with_scheme(app::Scheme::Gtk);

            let settings = Rc::new(RefCell::new(Settings::new()?));
            let mut ui = mainview::UserInterface::make_window();
            update_ui_with_settings(&mut ui, &settings.clone().borrow());
            {
                let save_ui = ui.clone();
                let mut save_button = ui.save_settings_button.clone();
                let save_settings = settings.clone();
                save_button.set_callback(move |cb| {
                    update_settings_with_ui_values(&mut save_settings.borrow_mut(), &save_ui);
                    save_settings_to_file(&save_settings.borrow());
                    cb.set_label("Saved");
                });
            }
            {
                let mut synch_bytton = ui.synchronize_button.clone();
                let synch_ui = ui.clone();
                let sync_settings = settings.clone();
                synch_bytton.set_callback(move |cb| {
                    update_settings_with_ui_values(&mut sync_settings.borrow_mut(), &synch_ui);
                    match block_on(run_sync(&sync_settings.borrow())) {
                        Ok(_) => cb.set_label("Synched"),
                        Err(e) => println!("{}", e),
                    }
                });
            }

            app.run().unwrap();
        }
        Ok(())
    }
    #[cfg(not(feature = "ui"))]
    {
        let settings = Settings::new()?;
        run_sync(&settings).await
    }
}

#[cfg(feature = "ui")]
fn empty_or_whitespace(input: String) -> Option<String> {
    if input.trim().is_empty() {
        None
    } else {
        Some(input)
    }
}
#[cfg(feature = "ui")]
fn update_settings_with_ui_values(settings: &mut Settings, ui: &mainview::UserInterface) {
    // Steam location
    settings.steam.location = empty_or_whitespace(ui.steam_location_input.value());

    // Steamgrid db
    settings.steamgrid_db.enabled = ui.enable_steamgrid_db_checkbox.value();
    settings.steamgrid_db.auth_key = empty_or_whitespace(ui.steamgrid_db_auth_key_input.value());

    // Legendary
    settings.legendary.enabled = ui.enable_legendary_checkbox.value();
    settings.legendary.executable = empty_or_whitespace(ui.legendary_executable_input.value());

    // Origin
    settings.origin.enabled = ui.enable_origin_checkbox.value();
    settings.origin.path = empty_or_whitespace(ui.origin_folder_input.value());

    // Epic
    settings.epic_games.enabled = ui.enable_egs_checkbox.value();
    settings.epic_games.location = empty_or_whitespace(ui.epic_location_input.value());

    // Itch
    settings.itch.enabled = ui.enable_itch_checkbox.value();
    settings.itch.location = empty_or_whitespace(ui.itch_locatoin_input.value());
}

#[cfg(feature = "ui")]

fn save_settings_to_file(settings: &Settings) {
    let toml = toml::to_string(&settings).unwrap();
    std::fs::write("config.toml", toml).unwrap();
}

#[cfg(feature = "ui")]
fn update_ui_with_settings(ui: &mut mainview::UserInterface, settings: &Settings) {
    ui.steam_location_input.set_value(
        &settings
            .steam
            .location
            .clone()
            .unwrap_or(steam::get_default_location().unwrap_or(String::from(""))),
    );

    ui.enable_steamgrid_db_checkbox
        .set_value(settings.steamgrid_db.enabled);
    ui.steamgrid_db_auth_key_input.set_value(
        &settings
            .steamgrid_db
            .auth_key
            .clone()
            .unwrap_or(String::from("")),
    );
    ui.enable_legendary_checkbox
        .set_value(settings.legendary.enabled);
    ui.legendary_executable_input.set_value(
        settings
            .legendary
            .executable
            .clone()
            .unwrap_or(String::from("legendary"))
            .as_str(),
    );
    ui.enable_egs_checkbox
        .set_value(settings.epic_games.enabled);
    let default_egs = get_default_location();
    let default_egs = default_egs
        .map(|p| p.to_str().unwrap().to_owned())
        .unwrap_or(String::from(""));
    let egs_location = settings
        .epic_games
        .location
        .clone()
        .unwrap_or(String::from(default_egs));
    ui.epic_location_input.set_value(&egs_location);
    ui.enable_egs_checkbox
        .set_value(settings.epic_games.enabled);
    ui.enable_itch_checkbox.set_value(settings.itch.enabled);
    let mut default_itch_location = itch::get_default_location();
    if !Path::new(&default_itch_location).exists() {
        default_itch_location = String::from("");
    }
    ui.enable_itch_checkbox.set_value(settings.itch.enabled);
    ui.itch_locatoin_input.set_value(
        &settings
            .itch
            .location
            .clone()
            .unwrap_or(default_itch_location),
    );
    ui.enable_origin_checkbox.set_value(settings.origin.enabled);
    let mut default_origin_location = origin::get_default_location();
    if !Path::new(&default_origin_location).exists() {
        default_origin_location = String::from("");
    }
    ui.origin_folder_input.set_value(
        &settings
            .origin
            .path
            .clone()
            .unwrap_or(default_origin_location),
    );
}

async fn run_sync(settings: &Settings) -> Result<(), Box<dyn Error>> {
    let userinfo_shortcuts = get_shortcuts_paths(&settings.steam)?;
    println!("Found {} user(s)", userinfo_shortcuts.len());
    for user in userinfo_shortcuts.iter() {
        let start_time = std::time::Instant::now();

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

        update_platform_shortcuts(
            &ItchPlatform::new(settings.itch.clone()),
            &mut new_user_shortcuts,
        );

        update_platform_shortcuts(
            &OriginPlatform {
                settings: settings.origin.clone(),
            },
            &mut new_user_shortcuts,
        );

        let shortcuts = new_user_shortcuts.iter().map(|f| f.borrow()).collect();

        save_shortcuts(&shortcuts, Path::new(&shortcut_info.path));

        let duration = start_time.elapsed();

        println!("Finished synchronizing games in: {:?}", duration);

        if settings.steamgrid_db.enabled {
            let auth_key = &settings.steamgrid_db.auth_key;
            if let Some(auth_key) = auth_key {
                let start_time = std::time::Instant::now();
                println!("Checking for game images");
                let client = steamgriddb_api::Client::new(auth_key);
                let mut search = CachedSearch::new(&client);
                let known_images = get_users_images(user).unwrap();
                download_images(
                    known_images,
                    user.steam_user_data_folder.as_str(),
                    shortcuts,
                    &mut search,
                    &client,
                )
                .await?;
                search.save();
                let duration = start_time.elapsed();
                println!("Finished getting images in: {:?}", duration);
            } else {
                println!("Steamgrid DB Auth Key not found, please add one as described here:  https://github.com/PhilipK/steam_shortcuts_sync#configuration");
            }
        }
    }
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
                    "Found {} game(s) for platform {}",
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
