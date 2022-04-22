use eframe::{egui, epi};
use egui::{ImageButton, Rounding, Stroke, TextureHandle};
use futures::executor::block_on;
use std::error::Error;
use tokio::{
    runtime::Runtime,
    sync::watch::{self, Receiver},
};

use crate::{
    egs::ManifestItem,
    settings::Settings,
    sync::{self, download_images, SyncProgress},
};

use super::{
    ui_colors::{
        BACKGROUND_COLOR, BG_STROKE_COLOR, EXTRA_BACKGROUND_COLOR, LIGHT_ORANGE, ORANGE, PURLPLE,
        TEXT_COLOR,
    },
    ui_images::{get_import_image, get_logo, get_logo_icon},
    ui_import_games::FetchGameStatus,
};

const SECTION_SPACING: f32 = 25.0;

#[derive(Default)]
struct UiImages {
    import_button: Option<egui::TextureHandle>,
    logo_32: Option<egui::TextureHandle>,
}

pub struct MyEguiApp {
    selected_menu: Menues,
    pub(crate) settings: Settings,
    pub(crate) rt: Runtime,
    ui_images: UiImages,
    pub(crate) games_to_sync: Receiver<FetchGameStatus>,
    pub(crate) status_reciever: Receiver<SyncProgress>,
    pub(crate) epic_manifests: Option<Vec<ManifestItem>>,
}

impl MyEguiApp {
    pub fn new() -> Self {
        let runtime = Runtime::new().unwrap();
        Self {
            selected_menu: Menues::Import,
            settings: Settings::new().expect("We must be able to load our settings"),
            rt: runtime,
            games_to_sync: watch::channel(FetchGameStatus::NeedsFetched).1,
            ui_images: UiImages::default(),
            status_reciever: watch::channel(SyncProgress::NotStarted).1,
            epic_manifests: None,
        }
    }
    pub fn run_sync(&mut self) {
        let (sender, reciever) = watch::channel(SyncProgress::NotStarted);
        let settings = self.settings.clone();
        if settings.steam.stop_steam {
            crate::steam::ensure_steam_stopped();
        }

        self.status_reciever = reciever;
        self.rt.spawn_blocking(move || {
            MyEguiApp::save_settings_to_file(&settings);
            let mut some_sender = Some(sender);
            let usersinfo = sync::run_sync(&settings, &mut some_sender).unwrap();
            let task = download_images(&settings, &usersinfo, &mut some_sender);
            block_on(task);
            if let Some(sender) = some_sender {
                let _ = sender.send(SyncProgress::Done);
            }
            if settings.steam.start_steam {
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
        _storage: Option<&dyn epi::Storage>,
    ) {
        ctx.set_pixels_per_point(1.0);
        let mut style: egui::Style = (*ctx.style()).clone();
        create_style(&mut style);
        ctx.set_style(style);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &epi::Frame) {
        let frame = egui::Frame::default()
            .stroke(Stroke::new(0., BACKGROUND_COLOR))
            .fill(BACKGROUND_COLOR);
        egui::SidePanel::new(egui::panel::Side::Left, "Side Panel")
            .default_width(40.0)
            .frame(frame)
            .show(ctx, |ui| {
                let texture = self.get_logo_image(ui);
                let size = texture.size_vec2();
                ui.image(texture, size);
                ui.add_space(SECTION_SPACING);

                let changed = ui
                    .selectable_value(&mut self.selected_menu, Menues::Import, "Import Games")
                    .changed();
                let changed = changed
                    || ui
                        .selectable_value(&mut self.selected_menu, Menues::Settings, "Settings")
                        .changed();
                if changed {
                    self.games_to_sync = watch::channel(FetchGameStatus::NeedsFetched).1;
                }
            });
        if self.games_to_sync.borrow().is_some() {
            egui::TopBottomPanel::new(egui::panel::TopBottomSide::Bottom, "Bottom Panel")
                .frame(frame)
                .show(ctx, |ui| {
                    let (status_string, syncing) = match &*self.status_reciever.borrow() {
                        SyncProgress::NotStarted => ("".to_string(), false),
                        SyncProgress::Starting => ("Starting Import".to_string(), true),
                        SyncProgress::FoundGames { games_found } => {
                            (format!("Found {} games to  import", games_found), true)
                        }
                        SyncProgress::FindingImages => (format!("Searching for images"), true),
                        SyncProgress::DownloadingImages { to_download } => {
                            (format!("Downloading {} images ", to_download), true)
                        }
                        SyncProgress::Done => (format!("Done importing games"), false),
                    };
                    if status_string != "" {
                        ui.label(status_string);
                    }

                    let texture = self.get_import_image(ui);
                    let size = texture.size_vec2();
                    let image_button = ImageButton::new(texture, size * 0.5);
                    if ui
                        .add(image_button)
                        .on_hover_text("Import your games into steam")
                        .clicked()
                        && !syncing
                    {
                        self.run_sync();
                    }
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.selected_menu {
                Menues::Import => {
                    self.render_import_games(ui);
                }
                Menues::Settings => {
                    self.render_settings(ui);
                }
            };
        });
    }
}

fn create_style(style: &mut egui::Style) {
    style.spacing.item_spacing = egui::vec2(15.0, 15.0);
    style.visuals.button_frame = false;
    style.visuals.dark_mode = true;
    style.visuals.override_text_color = Some(TEXT_COLOR);
    style.visuals.widgets.noninteractive.rounding = Rounding {
        ne: 0.0,
        nw: 0.0,
        se: 0.0,
        sw: 0.0,
    };
    style.visuals.faint_bg_color = PURLPLE;
    style.visuals.extreme_bg_color = EXTRA_BACKGROUND_COLOR;
    style.visuals.widgets.active.bg_fill = BACKGROUND_COLOR;
    style.visuals.widgets.active.bg_stroke = Stroke::new(2.0, BG_STROKE_COLOR);
    style.visuals.widgets.active.fg_stroke = Stroke::new(2.0, LIGHT_ORANGE);
    style.visuals.widgets.open.bg_fill = BACKGROUND_COLOR;
    style.visuals.widgets.open.bg_stroke = Stroke::new(2.0, BG_STROKE_COLOR);
    style.visuals.widgets.open.fg_stroke = Stroke::new(2.0, LIGHT_ORANGE);
    style.visuals.widgets.noninteractive.bg_fill = BACKGROUND_COLOR;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(2.0, BG_STROKE_COLOR);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(2.0, ORANGE);
    style.visuals.widgets.inactive.bg_fill = BACKGROUND_COLOR;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(2.0, BG_STROKE_COLOR);
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(2.0, ORANGE);
    style.visuals.widgets.hovered.bg_fill = BACKGROUND_COLOR;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(2.0, BG_STROKE_COLOR);
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(2.0, LIGHT_ORANGE);
    style.visuals.selection.bg_fill = PURLPLE;
}

impl MyEguiApp {
    fn get_import_image(&mut self, ui: &mut egui::Ui) -> &mut TextureHandle {
        self.ui_images.import_button.get_or_insert_with(|| {
            // Load the texture only once.
            ui.ctx().load_texture("import_image", get_import_image())
        })
    }

    fn get_logo_image(&mut self, ui: &mut egui::Ui) -> &mut TextureHandle {
        self.ui_images.logo_32.get_or_insert_with(|| {
            // Load the texture only once.
            ui.ctx().load_texture("logo32", get_logo())
        })
    }
}

pub fn run_sync() {
    let mut app = MyEguiApp::new();
    app.run_sync();
}

pub fn run_ui() -> Result<(), Box<dyn Error>> {
    let app = MyEguiApp::new();

    let mut native_options = eframe::NativeOptions::default();
    native_options.initial_window_size = Some(egui::Vec2 { x: 800., y: 500. });
    native_options.icon_data = Some(get_logo_icon());
    eframe::run_native(Box::new(app), native_options);
}
