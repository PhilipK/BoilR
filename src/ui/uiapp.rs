use eframe::{egui, epi::{self, IconData},};
use egui::{ScrollArea, TextureHandle,  Stroke, Rounding, Image};
use futures::executor::block_on;
use steam_shortcuts_util::shortcut::ShortcutOwned;
use std::error::Error;
use tokio::runtime::Runtime;

use crate::{settings::Settings, sync::{download_images, self}, sync::run_sync};

use super::{ui_images::{get_import_image, get_logo, get_logo_icon}, ui_colors::{TEXT_COLOR, BACKGROUND_COLOR, BG_STROKE_COLOR,  ORANGE, PURLPLE, LIGHT_ORANGE}};


#[derive(Default)]
struct UiImages{
    import_button: Option<egui::TextureHandle>,
    logo_32: Option<egui::TextureHandle>,
}


struct MyEguiApp {
    selected_menu: Menues,
    settings: Settings,
    rt: Runtime,    
    ui_images: UiImages,
    games_to_sync:Option<Vec<(String, Vec<ShortcutOwned>)>>
}

impl MyEguiApp {
    pub fn new() -> Self {
        let runtime = Runtime::new().unwrap();
        Self {
            selected_menu: Menues::Import,
            settings: Settings::new().expect("We must be able to load our settings"),
            rt: runtime,
            games_to_sync:None,
            ui_images: UiImages::default(),            
        }
    }
    pub fn run_sync(&self) {
        let settings = self.settings.clone();
        self.rt.spawn_blocking(move || {
            
            MyEguiApp::save_settings_to_file(&settings);
            //TODO get status back to ui
            let usersinfo = run_sync(&settings).unwrap();
            let task = download_images(&settings, &usersinfo);
            block_on(task);
        });
    }

    
    fn save_settings_to_file(settings: &Settings) {
        let toml = toml::to_string(&settings).unwrap();
        std::fs::write("config.toml", toml).unwrap();
    }
}

#[derive(PartialEq)]
enum Menues {
    Import, 
    Settings,    
}

impl Default for Menues {
    fn default() -> Menues {
        Menues::Import
    }
}

impl epi::App for MyEguiApp {
    fn name(&self) -> &str {
        "BoilR"
    } 

    fn update(&mut self, ctx: &egui::Context, _frame: &epi::Frame) {
        let mut style: egui::Style = (*ctx.style()).clone();
        create_style(&mut style);
        ctx.set_style(style);

        egui::SidePanel::new(egui::panel::Side::Left, "Side Panel")
            .default_width(40.0)
            .show(ctx, |ui| {
                let texture = self.get_logo_image(ui);
                let size = texture.size_vec2();
                ui.image(texture, size);
                ui.separator();
                ui.selectable_value(&mut self.selected_menu, Menues::Import, "Import Games");
                // ui.selectable_value(&mut self.selected_menu, Menues::Art, "Art");
                ui.selectable_value(&mut self.selected_menu, Menues::Settings, "Settings");
            });
            if  self.games_to_sync.is_some(){
        
            egui::TopBottomPanel::new(egui::panel::TopBottomSide::Bottom, "Bottom Panel")
            .show(ctx,|ui|{
                    let texture = self.get_import_image(ui);
                    let size = texture.size_vec2();
                    let image_button = Image::new(texture, size);
                    if ui.add(image_button).on_hover_text("Import your games into steam").clicked() {
                        self.run_sync();
                    }
            });
        }

        egui::CentralPanel::default()
            .show(ctx, |ui| {
                if let Menues::Import = self.selected_menu{                    
                }else{
                    self.games_to_sync = None;
                }
                match self.selected_menu {
                Menues::Import => {          
                    self.render_import_games(ui);
                   
                },
                Menues::Settings => {
                   self.render_settings(ui);
                },
            };
        
    });
}
}

fn create_style(style: &mut egui::Style) {
    style.spacing.item_spacing = egui::vec2(15.0, 15.0);
    style.visuals.button_frame = false;
    style.visuals.dark_mode = true;
    style.visuals.override_text_color = Some(TEXT_COLOR);
    style.visuals.widgets.noninteractive.rounding = Rounding{
        ne:0.0,
        nw:0.0,
        se:0.0,
        sw:0.0
    };
    style.visuals.faint_bg_color = PURLPLE;
    style.visuals.extreme_bg_color = BACKGROUND_COLOR;
    style.visuals.widgets.active.bg_fill = BACKGROUND_COLOR;
    style.visuals.widgets.active.bg_stroke = Stroke::new(2.0,BG_STROKE_COLOR);
    style.visuals.widgets.active.fg_stroke = Stroke::new(2.0,LIGHT_ORANGE);
    style.visuals.widgets.open.bg_fill = BACKGROUND_COLOR;
    style.visuals.widgets.open.bg_stroke = Stroke::new(2.0,BG_STROKE_COLOR);
    style.visuals.widgets.open.fg_stroke = Stroke::new(2.0,LIGHT_ORANGE);
    style.visuals.widgets.noninteractive.bg_fill = BACKGROUND_COLOR;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(2.0,BG_STROKE_COLOR);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(2.0,ORANGE);
    style.visuals.widgets.inactive.bg_fill = BACKGROUND_COLOR;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(2.0,BG_STROKE_COLOR);
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(2.0,ORANGE);
    style.visuals.widgets.hovered.bg_fill = BACKGROUND_COLOR;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(2.0,BG_STROKE_COLOR);
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(2.0,LIGHT_ORANGE);
}

impl MyEguiApp{

    fn get_import_image(&mut self, ui:&mut egui::Ui) -> &mut TextureHandle {
    self.ui_images.import_button.get_or_insert_with(|| {
        // Load the texture only once.
        ui.ctx().load_texture("import_image", get_import_image())
    })

    }

    fn get_logo_image(&mut self, ui:&mut egui::Ui) -> &mut TextureHandle {
        self.ui_images.logo_32.get_or_insert_with(|| {
            // Load the texture only once.
            ui.ctx().load_texture("logo32", get_logo())
        })
    
        }

    fn render_settings(&mut self, ui: &mut egui::Ui){
        ui.heading("Settings");
        ui.label("Here you can change your settings");

        ScrollArea::vertical().show(ui,|ui| {         
            ui.heading("SteamGridDB");
            ui.checkbox(&mut self.settings.steamgrid_db.enabled, "Download images");
            ui.horizontal(|ui| {
                let mut empty_string ="".to_string();
                let auth_key = self.settings.steamgrid_db.auth_key.as_mut().unwrap_or(&mut empty_string);
                ui.label("Authentication key: ");
                if ui.text_edit_singleline(auth_key).changed(){
                    self.settings.steamgrid_db.auth_key = Some(auth_key.to_string());
                }
            });
            ui.horizontal(|ui| {
                ui.label("To download images you need an API Key from SteamGridDB, you can find yours");
                ui.hyperlink_to("here", "https://www.steamgriddb.com/profile/preferences/api")
            });
            ui.checkbox(&mut self.settings.steamgrid_db.prefer_animated, "Prefer animated images").on_hover_text("Prefer downloading animated images over static images (this can slow Steam down but looks neat)");

            ui.separator();

            ui.heading("Steam");
            ui.horizontal(|ui| {
                let mut empty_string ="".to_string();
                let steam_location = self.settings.steam.location.as_mut().unwrap_or(&mut empty_string);
                ui.label("Steam Location: ");
                if ui.text_edit_singleline(steam_location).changed(){
                    self.settings.steam.location = Some(steam_location.to_string());
                }
            });
            ui.checkbox(&mut self.settings.steam.create_collections, "Create collections").on_hover_text("Tries to create a games collection for each platform");
            ui.checkbox(&mut self.settings.steam.optimize_for_big_picture, "Optimize for big picture").on_hover_text("Set icons to be larger horizontal images, this looks nice in steam big picture mode, but a bit off in desktop mode");

            ui.separator();

            ui.heading("Epic Games");
            ui.checkbox(&mut self.settings.epic_games.enabled, "Import form Epic Games");
            ui.horizontal(|ui| {
                let mut empty_string ="".to_string();
                let epic_location = self.settings.epic_games.location.as_mut().unwrap_or(&mut empty_string);
                ui.label("Epic Manifests Location: ").on_hover_text("The location where Epic stores its manifest files that BoilR needs to read");
                if ui.text_edit_singleline(epic_location).changed(){
                    self.settings.epic_games.location = Some(epic_location.to_string());
                }
            });

            ui.separator();

            #[cfg(target_family = "unix")]
            {
                ui.heading("Heroic");
                ui.checkbox(&mut self.settings.heroic.enabled, "Import form Heroic");
    
                ui.separator();
            }

            ui.heading("Legendary & Rare");
            ui.checkbox(&mut self.settings.legendary.enabled, "Import form Legendary & Rare");
            ui.horizontal(|ui| {
                let mut empty_string ="".to_string();
                let legendary_location = self.settings.legendary.executable.as_mut().unwrap_or(&mut empty_string);
                ui.label("Legendary Executable: ").on_hover_text("The location of the legendary executable to use");
                if ui.text_edit_singleline(legendary_location).changed(){
                    self.settings.legendary.executable = Some(legendary_location.to_string());
                }
            });

            ui.separator();

            ui.heading("Itch.io");
            ui.checkbox(&mut self.settings.itch.enabled, "Import form Itch.io");
            ui.horizontal(|ui| {
                let mut empty_string ="".to_string();
                let itch_location = self.settings.itch.location.as_mut().unwrap_or(&mut empty_string);
                ui.label("Itch.io Folder: ");
                if ui.text_edit_singleline(itch_location).changed(){
                    self.settings.itch.location = Some(itch_location.to_string());
                }
            });

            ui.separator();

            ui.heading("Origin");
            ui.checkbox(&mut self.settings.origin.enabled, "Import from Origin");            

            ui.separator();

            ui.heading("GoG Galaxy");
            ui.checkbox(&mut self.settings.gog.enabled, "Import form GoG Galaxy");
            ui.horizontal(|ui| {
                let mut empty_string ="".to_string();
                let itch_location = self.settings.gog.location.as_mut().unwrap_or(&mut empty_string);
                ui.label("GoG Galaxy Folder: ");
                if ui.text_edit_singleline(itch_location).changed(){
                    self.settings.gog.location = Some(itch_location.to_string());
                }
            });

            ui.separator();

            ui.heading("Uplay");
            ui.checkbox(&mut self.settings.uplay.enabled, "Import form Uplay");

            ui.separator();

            ui.heading("Lutris");
            ui.checkbox(&mut self.settings.lutris.enabled, "Import form Uplay");
            ui.horizontal(|ui| {
                let mut empty_string ="".to_string();
                let lutris_location = self.settings.lutris.executable.as_mut().unwrap_or(&mut empty_string);
                ui.label("Lutris Location: ");
                if ui.text_edit_singleline(lutris_location).changed(){
                    self.settings.lutris.executable = Some(lutris_location.to_string());
                }
            });
         
        });
    }

    fn render_import_games(&mut self, ui: &mut egui::Ui){
        ui.heading("Import Games");
        ui.label("Select the games you want to import into steam");
        ScrollArea::vertical().show(ui,|ui| {         
            match &self.games_to_sync{
                Some(games_to_sync) => {
                    for (platform_name, shortcuts) in games_to_sync{
                        ui.heading(platform_name);
                        for shortcut in shortcuts {
                            let mut import_game = !self.settings.blacklisted_games.contains(&shortcut.app_id);
                            let checkbox = egui::Checkbox::new(&mut import_game,&shortcut.app_name);
                            let response = ui.add(checkbox);                            
                            if response.clicked(){
                                if !self.settings.blacklisted_games.contains(&shortcut.app_id){
                                    self.settings.blacklisted_games.push(shortcut.app_id);
                                }else{
                                    self.settings.blacklisted_games.retain(|id| *id != shortcut.app_id);
                                }
                            }
                        }
                    }
                },
                None => {
                    self.games_to_sync = Some(sync::get_platform_shortcuts(&self.settings));
                },
            };   
        ui.label("Check the settings if BoilR didn't find the game you where looking for");

                        });
                    }
}



pub fn run_ui() -> Result<(), Box<dyn Error>> {
    let app = MyEguiApp::new();

    let mut native_options = eframe::NativeOptions::default();
    native_options.initial_window_size = Some(egui::Vec2{
        x:500.,
        y:500.
    });
    native_options.icon_data = Some(get_logo_icon());
    eframe::run_native(Box::new(app), native_options);
}
