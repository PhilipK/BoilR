use copypasta::ClipboardProvider;
use eframe::egui;
use egui::ScrollArea;

use crate::egs::EpicPlatform;

use super::{
    ui_colors::{BACKGROUND_COLOR, EXTRA_BACKGROUND_COLOR},
    MyEguiApp,
};
const SECTION_SPACING: f32 = 25.0;
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
            .stick_to_right()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.reset_style();

                self.render_steamgriddb_settings(ui);

                self.render_steam_settings(ui);

                self.render_epic_settings(ui);

                #[cfg(target_family = "unix")]
                {
                    ui.heading("Heroic");
                    ui.checkbox(&mut self.settings.heroic.enabled, "Import from Heroic");

                    ui.add_space(SECTION_SPACING);
                }

                self.render_legendary_settings(ui);
                self.render_itch_settings(ui);
                self.render_origin_settings(ui);
                self.render_gog_settings(ui);
                self.render_uplay_settings(ui);
                self.render_lutris_settings(ui);
                #[cfg(windows)]
                {
                    self.render_amazon_settings(ui);
                }
                ui.add_space(SECTION_SPACING);
                ui.label(format!("Version: {}", VERSION));
            });
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

    #[cfg(windows)]
    fn render_amazon_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Amazon");
        ui.checkbox(&mut self.settings.amazon.enabled, "Import from Amazon");
        ui.add_space(SECTION_SPACING);
    }

    fn render_uplay_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Uplay");
        ui.checkbox(&mut self.settings.uplay.enabled, "Import from Uplay");
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

    fn render_itch_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Itch.io");
        ui.checkbox(&mut self.settings.itch.enabled, "Import from Itch.io");
        if self.settings.itch.enabled {
            ui.horizontal(|ui| {
                let mut empty_string = "".to_string();
                let itch_location = self
                    .settings
                    .itch
                    .location
                    .as_mut()
                    .unwrap_or(&mut empty_string);
                ui.label("Itch.io Folder: ");
                if ui.text_edit_singleline(itch_location).changed() {
                    self.settings.itch.location = Some(itch_location.to_string());
                }
            });
        }
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
                self.settings.steam.location = Some(steam_location.to_string());
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
                let mut empty_string = "".to_string();
                let auth_key = self
                    .settings
                    .steamgrid_db
                    .auth_key
                    .as_mut()
                    .unwrap_or(&mut empty_string);
                ui.label("Authentication key: ");
                if ui.text_edit_singleline(auth_key).changed() {
                    *auth_key = auth_key.to_string();
                }
                if auth_key.is_empty() {
                    if ui.button("Paste from clipboard").clicked() {
                        if let Ok(mut clipboard_ctx) = copypasta::ClipboardContext::new() {
                            if let Ok(content) = clipboard_ctx.get_contents() {
                                self.settings.steamgrid_db.auth_key = Some(content.clone());
                            }
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

    fn render_epic_settings(&mut self, ui: &mut egui::Ui) {
        let epic_settings = &mut self.settings.epic_games;
        ui.heading("Epic Games");
        ui.checkbox(&mut epic_settings.enabled, "Import from Epic Games");
        if epic_settings.enabled {
            ui.horizontal(|ui| {
                let mut empty_string = "".to_string();
                let epic_location = epic_settings.location.as_mut().unwrap_or(&mut empty_string);
                ui.label("Epic Manifests Location: ").on_hover_text(
                    "The location where Epic stores its manifest files that BoilR needs to read",
                );
                if ui.text_edit_singleline(epic_location).changed() {
                    epic_settings.location = Some(epic_location.to_string());
                }
            });

            let safe_mode_header = match epic_settings.safe_launch.len() {
                0 => "Force games to launch through Epic Launcher".to_string(),
                1 => "One game forced to launch through Epic Launcher".to_string(),
                x => format!("{} games forced to launch through Epic Launcher", x),
            };

            egui::CollapsingHeader::new(safe_mode_header)
        .id_source("Epic_Launcher_safe_launch")
        .show(ui, |ui| {
            ui.label("Some games must be started from the Epic Launcher, select those games below and BoilR will create shortcuts that opens the games through the Epic Launcher.");
            let manifests =self.epic_manifests.get_or_insert_with(||{
                let epic_platform = EpicPlatform::new(epic_settings);
                let manifests = crate::platform::Platform::get_shortcuts(&epic_platform);
                manifests.unwrap_or_default()
            });
            let mut safe_open_games = epic_settings.safe_launch.clone();
            for manifest in manifests{
                let key = manifest.get_key();
                let display_name = &manifest.display_name;
                let mut safe_open = safe_open_games.contains(display_name) || safe_open_games.contains(&key);
                if ui.checkbox(&mut safe_open, display_name).clicked(){
                    if safe_open{
                        safe_open_games.push(key);
                    }else{
                        safe_open_games.retain(|m| m!= display_name && m!= &key);
                    }
                }
            }
            epic_settings.safe_launch = safe_open_games;
        })        ;
            ui.add_space(SECTION_SPACING);
        }
    }
}
