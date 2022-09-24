use copypasta::ClipboardProvider;
use eframe::egui;
use egui::ScrollArea;

#[cfg(target_family = "unix")]
use crate::heroic::HeroicPlatform;

use super::{
    ui_colors::{BACKGROUND_COLOR, EXTRA_BACKGROUND_COLOR},
    MyEguiApp,
};
pub const SECTION_SPACING: f32 = 25.0;
const VERSION: &str = env!("CARGO_PKG_VERSION");

impl MyEguiApp {
    pub(crate) fn render_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");

        let mut scroll_style = ui.style_mut();
        scroll_style.visuals.extreme_bg_color = BACKGROUND_COLOR;
        scroll_style.visuals.widgets.inactive.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.widgets.active.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.selection.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.widgets.hovered.bg_fill = EXTRA_BACKGROUND_COLOR;

        ScrollArea::vertical()
            .stick_to_right(true)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.reset_style();

                self.render_steamgriddb_settings(ui);

                self.render_steam_settings(ui);

                for platform in &mut self.platforms{
                    platform.render_ui(ui);
                    ui.add_space(SECTION_SPACING);
                }

                #[cfg(target_family = "unix")]
                {
                    self.render_heroic_settings(ui);
                }

                self.render_legendary_settings(ui);
                self.render_origin_settings(ui);
                self.render_gog_settings(ui);
                self.render_lutris_settings(ui);
                
                ui.add_space(SECTION_SPACING);
                ui.label(format!("Version: {}", VERSION));
                
              

            });
    }
    
#[cfg(target_family = "unix")]
    fn render_heroic_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Heroic");
        ui.checkbox(&mut self.settings.heroic.enabled, "Import from Heroic");
        ui.checkbox(&mut self.settings.heroic.default_launch_through_heroic, "Always launch games through Heroic");
         let safe_mode_header = match (self.settings.heroic.default_launch_through_heroic,self.settings.heroic.launch_games_through_heroic.len()) {
                        (false,0) => "Force games to launch through Heroic Launcher".to_string(),
                        (false,1) => "One game forced to launch through Heroic Launcher".to_string(),
                        (false,x) => format!("{} games forced to launch through Heroic Launcher", x),
            
                        (true,0) => "Force games to launch directly".to_string(),
                        (true,1) => "One game forced to launch directly".to_string(),
                        (true,x) => format!("{} games forced to launch directly", x),
            
                    };
        egui::CollapsingHeader::new(safe_mode_header)
                .id_source("Heroic_Launcher_safe_launch")
                .show(ui, |ui| {
if 
self.settings.heroic.default_launch_through_heroic{
                                   ui.label("Some games work best when launched directly, select those games below and BoilR will create shortcuts that launch the games directly.");
                    
                }   else{
                                   ui.label("Some games must be started from the Heroic Launcher, select those games below and BoilR will create shortcuts that opens the games through the Heroic Launcher.");
                    
                }
                #[cfg(target_family = "unix")]{
                  let manifests =self.heroic_games.get_or_insert_with(||{
                        let heroic_setting = self.settings.heroic.clone();
        
                        let heroic_platform =HeroicPlatform{
                        settings:heroic_setting
                    };
                        heroic_platform.get_heroic_games()                        
                    });
                                    
                    let safe_open_games = &mut self.settings.heroic.launch_games_through_heroic;
                    for manifest in manifests{
                        let key = manifest.app_name();
                        let display_name = manifest.title();
                        let mut safe_open = safe_open_games.contains(&display_name.to_string()) || safe_open_games.contains(&key.to_string());
                        if ui.checkbox(&mut safe_open, display_name).clicked(){
                            if safe_open{
                                safe_open_games.push(key.to_string());
                            }else{
                                safe_open_games.retain(|m| m!= display_name && m!= key);
                            }
                        }                  
                        }
                    }
                })        ;
        ui.add_space(SECTION_SPACING);
    }

    fn render_lutris_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Lutris");
        ui.checkbox(&mut self.settings.lutris.enabled, "Import from Lutris");
        if self.settings.lutris.enabled {
            ui.checkbox(&mut self.settings.lutris.flatpak, "Flatpak version");
            if !self.settings.lutris.flatpak {
                ui.horizontal(|ui| {
                    let lutris_location = &mut self.settings.lutris.executable;
                    ui.label("Lutris Location: ");
                    ui.text_edit_singleline(lutris_location);
                });
            } else {
                ui.horizontal(|ui| {
                    let flatpak_image = &mut self.settings.lutris.flatpak_image;
                    ui.label("Flatpak image");
                    ui.text_edit_singleline(flatpak_image);
                });
            }
        }
        ui.add_space(SECTION_SPACING);
    }

    fn render_gog_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("GoG Galaxy");
        ui.checkbox(&mut self.settings.gog.enabled, "Import from GoG Galaxy");
        if self.settings.gog.enabled {
            ui.horizontal(|ui| {
                let mut empty_string = "".to_string();
                let itch_location = self
                    .settings
                    .gog
                    .location
                    .as_mut()
                    .unwrap_or(&mut empty_string);
                ui.label("GoG Galaxy Folder: ");
                if ui.text_edit_singleline(itch_location).changed() {
                    self.settings.gog.location = Some(itch_location.to_string());
                }
            });
        }
        ui.add_space(SECTION_SPACING);
    }

    fn render_origin_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Origin");
        ui.checkbox(&mut self.settings.origin.enabled, "Import from Origin");
        ui.add_space(SECTION_SPACING);
    }
    fn render_steam_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Steam");
        ui.horizontal(|ui| {
            let mut empty_string = "".to_string();
            let steam_location = self
                .settings
                .steam
                .location
                .as_mut()
                .unwrap_or(&mut empty_string);
            ui.label("Steam Location: ");
            if ui.text_edit_singleline(steam_location).changed() {
                if steam_location.trim().is_empty(){
                    self.settings.steam.location = None;
                }else{
                    self.settings.steam.location = Some(steam_location.to_string());
                }
                
            }
        });
        ui.checkbox(
            &mut self.settings.steam.create_collections,
            "Create collections",
        )
        .on_hover_text("Tries to create a games collection for each platform");
        ui.checkbox(&mut self.settings.steam.optimize_for_big_picture, "Optimize for big picture").on_hover_text("Set icons to be larger horizontal images, this looks nice in steam big picture mode, but a bit off in desktop mode");
        ui.checkbox(
            &mut self.settings.steam.stop_steam,
            "Stop Steam before import",
        )
        .on_hover_text("Stops Steam if it is running when import starts");
        ui.checkbox(
            &mut self.settings.steam.start_steam,
            "Start Steam after import",
        )
        .on_hover_text("Starts Steam is it is not running after the import");
        ui.add_space(SECTION_SPACING);
    }

    fn render_steamgriddb_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("SteamGridDB");
        ui.checkbox(&mut self.settings.steamgrid_db.enabled, "Download images");
        if self.settings.steamgrid_db.enabled {
            ui.horizontal(|ui| {
                let mut auth_key = self
                    .settings
                    .steamgrid_db
                    .auth_key
                    .clone()
                    .unwrap_or_default();
                ui.label("Authentication key: ");
                if ui.text_edit_singleline(&mut auth_key).changed() {
                    if auth_key.is_empty() {
                        self.settings.steamgrid_db.auth_key = None;
                    } else {
                        self.settings.steamgrid_db.auth_key = Some(auth_key.to_string());
                    }
                }
                if auth_key.is_empty() && ui.button("Paste from clipboard").clicked(){
                    if let Ok(mut clipboard_ctx) = copypasta::ClipboardContext::new() {
                        if let Ok(content) = clipboard_ctx.get_contents() {
                            self.settings.steamgrid_db.auth_key = Some(content);
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label(
                    "To download images you need an API Key from SteamGridDB, you can find yours",
                );
                ui.hyperlink_to(
                    "here",
                    "https://www.steamgriddb.com/profile/preferences/api",
                )
            });
            ui.checkbox(&mut self.settings.steamgrid_db.prefer_animated, "Prefer animated images").on_hover_text("Prefer downloading animated images over static images (this can slow Steam down but looks neat)");
            ui.checkbox(
                &mut self.settings.steamgrid_db.only_download_boilr_images,
                "Only download images for BoilR shortcuts",
            );
        }
        ui.add_space(SECTION_SPACING);
    }

    fn render_legendary_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Legendary & Rare");
        ui.checkbox(
            &mut self.settings.legendary.enabled,
            "Import from Legendary & Rare",
        );
        if self.settings.legendary.enabled {
            ui.horizontal(|ui| {
                let mut empty_string = "".to_string();
                let legendary_location = self
                    .settings
                    .legendary
                    .executable
                    .as_mut()
                    .unwrap_or(&mut empty_string);
                ui.label("Legendary Executable: ")
                    .on_hover_text("The location of the legendary executable to use");
                if ui.text_edit_singleline(legendary_location).changed() {
                    self.settings.legendary.executable = Some(legendary_location.to_string());
                }
            });
        }
        ui.add_space(SECTION_SPACING);
    }    
}
