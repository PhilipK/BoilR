use eframe::{egui, epi,};
use egui::{ScrollArea, TextureHandle, ImageButton};
use futures::executor::block_on;
use steam_shortcuts_util::shortcut::ShortcutOwned;
use std::error::Error;
use tokio::runtime::Runtime;

use crate::{settings::Settings, sync::{download_images, self}, sync::run_sync};

use super::ui_images::get_import_image;


#[derive(Default)]
struct UiImages{
    import_button: Option<egui::TextureHandle>,
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
    Art,
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
        style.spacing.item_spacing = egui::vec2(15.0, 15.0);
        style.visuals.dark_mode = true;
        

        ctx.set_style(style);

        egui::SidePanel::new(egui::panel::Side::Left, "Side Panel")
            // .frame(my_frame.clone())
            .show(ctx, |ui| {
                ui.heading("BoilR");
                ui.separator();
                ui.selectable_value(&mut self.selected_menu, Menues::Import, "Import Games");
                ui.selectable_value(&mut self.selected_menu, Menues::Art, "Art");
                ui.selectable_value(&mut self.selected_menu, Menues::Settings, "Settings");
            });
            
            egui::TopBottomPanel::bottom("ActionsPanel").show(ctx, |ui|{
                let texture = self.get_import_image(ui);
                let size = texture.size_vec2();
                let image_button = ImageButton::new(texture, size);
                if ui.add(image_button).on_hover_text("Import your games into steam").clicked() {
                    self.run_sync();
                }
            });            
        egui::CentralPanel::default()
            // .frame(my_frame)
            .show(ctx, |ui| {
                match self.selected_menu {
                Menues::Import => {           
                    self.render_import_games(ui)
                }
                Menues::Art => {
                    ui.label("In the future you will be able to specify which art you want for each game");
                },
                Menues::Settings => {
                   self.render_settings(ui);
                },
            };
        
    });
}
}

impl MyEguiApp{

    fn get_import_image(&mut self, ui:&mut egui::Ui) -> &mut TextureHandle {
    self.ui_images.import_button.get_or_insert_with(|| {
        // Load the texture only once.
        ui.ctx().load_texture("import_image", get_import_image())
    })
}

    fn render_settings(&mut self, ui: &mut egui::Ui){
        ui.heading("Settings");

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
                        });
                    }
}



pub fn run_ui() -> Result<(), Box<dyn Error>> {
    let app = MyEguiApp::new();

    let native_options = eframe::NativeOptions::default();

    eframe::run_native(Box::new(app), native_options);
}
