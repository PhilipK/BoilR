use eframe::{egui, epi::{self},};
use egui::{ScrollArea, TextureHandle,  Stroke, Rounding, ImageButton};
use futures::executor::block_on;
use steam_shortcuts_util::shortcut::ShortcutOwned;
use std::{error::Error};
use tokio::{runtime::Runtime, sync::watch::{Receiver, self}};

use crate::{settings::Settings, sync::{download_images, self, SyncProgress}, egs::{EpicPlatform, ManifestItem}};

use super::{ui_images::{get_import_image, get_logo, get_logo_icon}, ui_colors::{TEXT_COLOR, BACKGROUND_COLOR, BG_STROKE_COLOR,  ORANGE, PURLPLE, LIGHT_ORANGE, EXTRA_BACKGROUND_COLOR}};

const SECTION_SPACING : f32 = 25.0;

#[derive(Default)]
struct UiImages{
    import_button: Option<egui::TextureHandle>,
    logo_32: Option<egui::TextureHandle>,
}


enum FetchGameStatus{
    NeedsFetched,
    Fetching,
    Fetched(Vec<(String, Vec<ShortcutOwned>)>)
}

impl FetchGameStatus{
    pub fn is_some(&self) -> bool{
        match self{
            FetchGameStatus::NeedsFetched => false,
            FetchGameStatus::Fetching => false,
            FetchGameStatus::Fetched(_) => true,
        }
    }

    pub fn needs_fetching(&self) -> bool{
        match self{
            FetchGameStatus::NeedsFetched => true,
            FetchGameStatus::Fetching => false,
            FetchGameStatus::Fetched(_) => false,
        }
    }
}

struct MyEguiApp {
    selected_menu: Menues,
    settings: Settings,
    rt: Runtime,    
    ui_images: UiImages,    
    games_to_sync: Receiver<FetchGameStatus>,    
    status_reciever: Receiver<SyncProgress>,
    epic_manifests: Option<Vec<ManifestItem>>,
}



impl MyEguiApp {
    pub fn new() -> Self {
        let runtime = Runtime::new().unwrap();
        Self {
            selected_menu: Menues::Import,
            settings: Settings::new().expect("We must be able to load our settings"),
            rt: runtime,
            games_to_sync:watch::channel(FetchGameStatus::NeedsFetched).1,
            ui_images: UiImages::default(),            
            status_reciever: watch::channel(SyncProgress::NotStarted).1,
            epic_manifests : None,
        }
    }
    pub fn run_sync(&mut self) {
        let (sender,reciever ) = watch::channel(SyncProgress::NotStarted);        
        let settings = self.settings.clone();        
        if settings.steam.stop_steam{
            crate::steam::ensure_steam_stopped();
        }

        self.status_reciever   = reciever;
        self.rt.spawn_blocking(move || {                        

            MyEguiApp::save_settings_to_file(&settings);
            let mut some_sender =Some(sender);
            let usersinfo = sync::run_sync(&settings,&mut some_sender).unwrap();                        
            let task = download_images(&settings, &usersinfo,&mut some_sender);
            block_on(task);
            if let Some(sender) = some_sender{
                let _ = sender.send(SyncProgress::Done);
            }
            if settings.steam.start_steam{
                crate::steam::ensure_steam_started(&settings.steam);
            }
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

    fn setup(
        &mut self, 
        ctx: &egui::Context, 
        _frame: &epi::Frame, 
        _storage: Option<&dyn epi::Storage>
    ) { 
        ctx.set_pixels_per_point(1.0);
        let mut style: egui::Style = (*ctx.style()).clone();
        create_style(&mut style);
        ctx.set_style(style);
     }

    fn update(&mut self, ctx: &egui::Context, _frame: &epi::Frame) {
        let frame = egui::Frame::default().stroke(Stroke::new(0., BACKGROUND_COLOR)).fill(BACKGROUND_COLOR);
        egui::SidePanel::new(egui::panel::Side::Left, "Side Panel")
            .default_width(40.0)
            .frame(frame)
            .show(ctx, |ui| {
                let texture = self.get_logo_image(ui);
                let size = texture.size_vec2();
                ui.image(texture, size);
                ui.add_space(SECTION_SPACING);                
                
                let changed = ui.selectable_value(&mut self.selected_menu, Menues::Import, "Import Games").changed();
                let changed = changed || ui.selectable_value(&mut self.selected_menu, Menues::Settings, "Settings").changed();
                if changed{
                    self.games_to_sync =watch::channel(FetchGameStatus::NeedsFetched).1;
                }
                
            });
            if  self.games_to_sync.borrow().is_some(){
        
            egui::TopBottomPanel::new(egui::panel::TopBottomSide::Bottom, "Bottom Panel")
            .frame(frame)
            .show(ctx,|ui|{
                let (status_string,syncing) =  match &*self.status_reciever.borrow(){
                        SyncProgress::NotStarted => {
                            ("".to_string(),false)
                        },
                        SyncProgress::Starting => {
                           ("Starting Import".to_string(),true)
                        },
                        SyncProgress::FoundGames { games_found } => {
                            (format!("Found {} games to  import",games_found),true)
                        },
                        SyncProgress::FindingImages => {
                            (format!("Searching for images"),true)
                        },
                        SyncProgress::DownloadingImages {  to_download } => {
                            (format!("Downloading {} images ",to_download),true)
                        },
                        SyncProgress::Done => {
                            (format!("Done importing games"),false)
                        },
                };
                if status_string != "" {
                    ui.label(status_string);
                }
                
                let texture = self.get_import_image(ui);
                let size = texture.size_vec2();
                let image_button = ImageButton::new(texture, size * 0.5);
                if ui.add(image_button).on_hover_text("Import your games into steam")                                
                    .clicked() && !syncing{                    
                        self.run_sync();
                }
            });
        }

        egui::CentralPanel::default()
            .show(ctx, |ui| {
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
    style.visuals.extreme_bg_color = EXTRA_BACKGROUND_COLOR;
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
    style.visuals.selection.bg_fill = PURLPLE;
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
                
        let mut scroll_style = ui.style_mut();
        scroll_style.visuals.extreme_bg_color = BACKGROUND_COLOR;
        scroll_style.visuals.widgets.inactive.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.widgets.active.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.selection.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.widgets.hovered.bg_fill = EXTRA_BACKGROUND_COLOR;

        ScrollArea::vertical()
        .stick_to_right()
        .auto_shrink([false,true])
        .show(ui,|ui| {         
            ui.reset_style();

            self.render_steamgriddb_settings(ui);

            self.render_steam_settings(ui);

            self.render_epic_settings(ui);


            #[cfg(target_family = "unix")]
            {
                ui.heading("Heroic");
                ui.checkbox(&mut self.settings.heroic.enabled, "Import form Heroic");
    
                ui.add_space(SECTION_SPACING);
            }

            self.render_legendary_settings(ui);
            self.render_itch_settings(ui);
            self.render_origin_settings(ui);
            self.render_gog_settings(ui);
            self.render_uplay_settings(ui);
            self.render_lutris_settings(ui);
         
        });
    }

    fn render_lutris_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Lutris");
        ui.checkbox(&mut self.settings.lutris.enabled, "Import form Lutris");
        if self.settings.lutris.enabled{
            ui.horizontal(|ui| {
                let mut empty_string ="".to_string();
                let lutris_location = self.settings.lutris.executable.as_mut().unwrap_or(&mut empty_string);
                ui.label("Lutris Location: ");
                if ui.text_edit_singleline(lutris_location).changed(){
                    self.settings.lutris.executable = Some(lutris_location.to_string());
                }
            });
        }
    }

    fn render_uplay_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Uplay");
        ui.checkbox(&mut self.settings.uplay.enabled, "Import form Uplay");
        ui.add_space(SECTION_SPACING);
    }

    fn render_gog_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("GoG Galaxy");
        ui.checkbox(&mut self.settings.gog.enabled, "Import form GoG Galaxy");
        if self.settings.gog.enabled {
            ui.horizontal(|ui| {
                let mut empty_string ="".to_string();
                let itch_location = self.settings.gog.location.as_mut().unwrap_or(&mut empty_string);
                ui.label("GoG Galaxy Folder: ");
                if ui.text_edit_singleline(itch_location).changed(){
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
        ui.checkbox(&mut self.settings.itch.enabled, "Import form Itch.io");
        if self.settings.itch.enabled {
            ui.horizontal(|ui| {
                let mut empty_string ="".to_string();
                let itch_location = self.settings.itch.location.as_mut().unwrap_or(&mut empty_string);
                ui.label("Itch.io Folder: ");
                if ui.text_edit_singleline(itch_location).changed(){
                    self.settings.itch.location = Some(itch_location.to_string());
                }
            });
        }
        ui.add_space(SECTION_SPACING);
    }

    fn render_steam_settings(&mut self, ui: &mut egui::Ui) {
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
        ui.checkbox(&mut self.settings.steam.stop_steam, "Stop Steam before import").on_hover_text("Stops Steam if it is running when import starts");
        ui.checkbox(&mut self.settings.steam.start_steam, "Start Steam after import").on_hover_text("Starts Steam is it is not running after the import");
        ui.add_space(SECTION_SPACING);
    }

    fn render_steamgriddb_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("SteamGridDB");
        ui.checkbox(&mut self.settings.steamgrid_db.enabled, "Download images");
        if self.settings.steamgrid_db.enabled{
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
        }
        ui.add_space(SECTION_SPACING);
    }

    fn render_legendary_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Legendary & Rare");
        ui.checkbox(&mut self.settings.legendary.enabled, "Import form Legendary & Rare");
        if self.settings.legendary.enabled{
            ui.horizontal(|ui| {
                let mut empty_string ="".to_string();
                let legendary_location = self.settings.legendary.executable.as_mut().unwrap_or(&mut empty_string);
                ui.label("Legendary Executable: ").on_hover_text("The location of the legendary executable to use");
                if ui.text_edit_singleline(legendary_location).changed(){
                    self.settings.legendary.executable = Some(legendary_location.to_string());
                }
            });
        }
        ui.add_space(SECTION_SPACING);
    }

    fn render_epic_settings(&mut self, ui: &mut egui::Ui) {
        let epic_settings  = &mut self.settings.epic_games;
        ui.heading("Epic Games");
        ui.checkbox(&mut epic_settings.enabled, "Import form Epic Games");
        if epic_settings.enabled{
        ui.horizontal(|ui| {
            let mut empty_string ="".to_string();
            let epic_location = epic_settings.location.as_mut().unwrap_or(&mut empty_string);
            ui.label("Epic Manifests Location: ").on_hover_text("The location where Epic stores its manifest files that BoilR needs to read");
            if ui.text_edit_singleline(epic_location).changed(){
                epic_settings.location = Some(epic_location.to_string());
            }
        });

        let safe_mode_header = match epic_settings.safe_launch.len(){
            0 => "Force games to launch through Epic Launcher".to_string(),
            1 => "One game forced to launch through Epic Launcher".to_string(),
            x => format!("{} games forced to launch through Epic Launcher",x)
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

    fn render_import_games(&mut self, ui: &mut egui::Ui){
        
        ui.heading("Import Games");

        if self.games_to_sync.borrow().needs_fetching(){
            let (tx, rx) = watch::channel(FetchGameStatus::NeedsFetched);
            self.games_to_sync = rx;
            let settings = self.settings.clone();
            self.rt.spawn_blocking(move || {                        
                let _= tx.send(FetchGameStatus::Fetching);
                let games_to_sync = sync::get_platform_shortcuts(&settings);
                let _= tx.send(FetchGameStatus::Fetched(games_to_sync));
            });
        }
        
        let mut scroll_style = ui.style_mut();
        scroll_style.visuals.extreme_bg_color = BACKGROUND_COLOR;
        scroll_style.visuals.widgets.inactive.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.widgets.active.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.selection.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.widgets.hovered.bg_fill = EXTRA_BACKGROUND_COLOR;


        ScrollArea::vertical()        
        .stick_to_right()
        .auto_shrink([false,true])
        .show(ui,|ui| {         
            ui.reset_style();

            let borrowed_games = &*self.games_to_sync.borrow();
            match borrowed_games{                
                FetchGameStatus::Fetched(games_to_sync) => {
                    ui.label("Select the games you want to import into steam");
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
                    ui.add_space(SECTION_SPACING);
                    ui.label("Check the settings if BoilR didn't find the game you where looking for");
                },
                _=> {
                    ui.label("Finding installed games");
                },
            };   
        
                        });
                    }
}


pub fn run_sync() {
    let mut app = MyEguiApp::new();
    app.run_sync();
}


pub fn run_ui() -> Result<(), Box<dyn Error>> {
    let app = MyEguiApp::new();

    let mut native_options = eframe::NativeOptions::default();
    native_options.initial_window_size = Some(egui::Vec2{
        x:800.,
        y:500.
    });    
    native_options.icon_data = Some(get_logo_icon());
    eframe::run_native(Box::new(app), native_options);
}
