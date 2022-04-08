use eframe::{egui, epi};
use egui::{Color32, Visuals, Widget};
use futures::executor::block_on;
use std::error::Error;
use tokio::runtime::Runtime;

use crate::{settings::Settings, sync::download_images, sync::run_sync};

struct MyEguiApp {
    selected_menu: Menues,
    settings: Settings,
    rt: Runtime,
}

impl MyEguiApp {
    pub fn new() -> Self {
        let runtime = Runtime::new().unwrap();

        Self {
            selected_menu: Menues::Sync,
            settings: Settings::new().expect("We must be able to load our settings"),
            rt: runtime,
        }
    }
    pub fn run_sync(&self) {
        let settings = self.settings.clone();
        self.rt.spawn_blocking(move || {
            //TODO get status back to ui
            let usersinfo = run_sync(&settings).unwrap();
            let task = download_images(&settings, &usersinfo);
            block_on(task);
        });
    }
}

#[derive(PartialEq)]
enum Menues {
    Sync,
    Steam,
    Images,
    Legendary,
    Origin,
    Epic,
    Itch,
    Gog,
    Uplay,
    Lutris,
    Heroic,
}

impl Default for Menues {
    fn default() -> Menues {
        Menues::Sync
    }
}

impl epi::App for MyEguiApp {
    fn name(&self) -> &str {
        "BoilR"
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &epi::Frame) {
        let mut style: egui::Style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(15.0, 15.0);

        // style.visuals.extreme_bg_color = Color32::from_rgb(13, 43, 69);
        // style.visuals.faint_bg_color = Color32::from_rgb(84, 78, 104);
        // let my_frame = egui::containers::Frame {
        //     fill: Color32::from_rgb(32, 60, 86),
        //     stroke: egui::Stroke::new(2.0, Color32::from_rgb(84, 78, 104)),
        //     ..Default::default()
        // };

        ctx.set_style(style);

        egui::SidePanel::new(egui::panel::Side::Left, "Side Panel")
            // .frame(my_frame.clone())
            .show(ctx, |ui| {
                ui.heading("BoilR");
                ui.separator();
                ui.selectable_value(&mut self.selected_menu, Menues::Sync, "Sync");
                ui.selectable_value(&mut self.selected_menu, Menues::Steam, "Steam");
                ui.selectable_value(&mut self.selected_menu, Menues::Images, "Images");
                ui.separator();
                ui.selectable_value(&mut self.selected_menu, Menues::Epic, "Epic");
                ui.selectable_value(&mut self.selected_menu, Menues::Itch, "Itch");
                ui.selectable_value(&mut self.selected_menu, Menues::Gog, "Gog");
                ui.selectable_value(&mut self.selected_menu, Menues::Origin, "Origin");
                ui.selectable_value(&mut self.selected_menu, Menues::Uplay, "Uplay");
                ui.selectable_value(&mut self.selected_menu, Menues::Lutris, "Lutris");
                ui.selectable_value(&mut self.selected_menu, Menues::Legendary, "Legendary");
                ui.selectable_value(&mut self.selected_menu, Menues::Heroic, "Heroic");
            });

        egui::CentralPanel::default()
            // .frame(my_frame)
            .show(ctx, |ui| match self.selected_menu {
                Menues::Sync => {
                    if ui.button("Synchronize").clicked() {
                        self.run_sync();
                    }
                }
                Menues::Steam => todo!(),
                Menues::Images => todo!(),
                Menues::Legendary => todo!(),
                Menues::Origin => todo!(),
                Menues::Epic => todo!(),
                Menues::Itch => todo!(),
                Menues::Gog => todo!(),
                Menues::Uplay => todo!(),
                Menues::Lutris => todo!(),
                Menues::Heroic => todo!(),
            });
    }
}

pub fn run_ui() -> Result<(), Box<dyn Error>> {
    let settings = Settings::new();
    let app = MyEguiApp::new();

    let native_options = eframe::NativeOptions::default();

    eframe::run_native(Box::new(app), native_options);
}
