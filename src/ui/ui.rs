use crate::sync::run_sync;
use crate::ui::UserInterface;
use crate::{egs::get_default_location, settings::Settings};
use fltk::{app, prelude::*};
use futures::executor::block_on;
use std::error::Error;
use std::{cell::RefCell, rc::Rc};

pub async fn run_ui() -> Result<(), Box<dyn Error>> {
    let app = app::App::default().with_scheme(app::Scheme::Gtk);

    let settings = Rc::new(RefCell::new(Settings::new()?));
    let mut ui = UserInterface::make_window();
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
    Ok(())
}

fn empty_or_whitespace(input: String) -> Option<String> {
    if input.trim().is_empty() {
        None
    } else {
        Some(input)
    }
}
fn update_settings_with_ui_values(settings: &mut Settings, ui: &UserInterface) {
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

    // Gog
    settings.gog.enabled = ui.enable_gog_checkbox.value();
    settings.gog.location = empty_or_whitespace(ui.gog_folder_input.value());
    settings.gog.wine_c_drive = empty_or_whitespace(ui.gog_winedrive_input.value());


    // Uplay
    settings.uplay.enabled = ui.enable_uplay_checkbox.value();
}

fn save_settings_to_file(settings: &Settings) {
    let toml = toml::to_string(&settings).unwrap();
    std::fs::write("config.toml", toml).unwrap();
}

fn update_ui_with_settings(ui: &mut UserInterface, settings: &Settings) {
    use std::path::Path;

    use crate::{gog, itch, origin, steam};

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

    ui.enable_gog_checkbox.set_value(settings.gog.enabled);
    let default_gog_location = gog::default_location();
    let default_gog_location = if !default_gog_location.exists() {
        String::from("")
    } else {
        default_gog_location.to_string_lossy().to_string()
    };
    ui.gog_folder_input.set_value(
        &settings
            .gog
            .location
            .clone()
            .unwrap_or(default_gog_location),
    );

    #[cfg(target_os = "linux")]
    {
        ui.gog_winedrive_input
            .set_value(&settings.gog.wine_c_drive.clone().unwrap_or("".to_string()));
    }
    #[cfg(not(target_os = "linux"))]
    {
        ui.gog_winedrive_input.deactivate();
    }
    
    ui.enable_uplay_checkbox.set_value(settings.uplay.enabled);
}
