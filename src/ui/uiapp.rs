use std::{collections::HashMap, error::Error};


use eframe::{egui, App, Frame};
use egui::{ImageButton, Rounding, Stroke, TextureHandle};
use steam_shortcuts_util::shortcut::ShortcutOwned;
use tokio::{
    runtime::Runtime,
    sync::watch::{self, Receiver},
};

use crate::{
    config::get_renames_file,
    platforms::{get_platforms, Platforms},
    settings::Settings,
    sync::SyncProgress,
};

use super::{
    ui_colors::{
        BACKGROUND_COLOR, BG_STROKE_COLOR, EXTRA_BACKGROUND_COLOR, LIGHT_ORANGE, ORANGE, PURLPLE,
        TEXT_COLOR,
    },
    ui_images::{get_import_image, get_logo, get_logo_icon, get_save_image},
    ui_import_games::FetcStatus,
    BackupState, DiconnectState, ImageSelectState,
};

const SECTION_SPACING: f32 = 25.0;

#[derive(Default)]
struct UiImages {
    import_button: Option<egui::TextureHandle>,
    save_button: Option<egui::TextureHandle>,
    logo_32: Option<egui::TextureHandle>,
}

pub struct MyEguiApp {
    selected_menu: Menues,
    pub(crate) settings: Settings,
    pub(crate) rt: Runtime,
    ui_images: UiImages,
    pub(crate) games_to_sync: Receiver<FetcStatus<Vec<(String, Vec<ShortcutOwned>)>>>,
    pub(crate) status_reciever: Receiver<SyncProgress>,
    pub(crate) image_selected_state: ImageSelectState,
    pub(crate) backup_state: BackupState,
    pub(crate) disconect_state: DiconnectState,
    pub(crate) rename_map: HashMap<u32, String>,
    pub(crate) current_edit: Option<u32>,
    pub(crate) platforms: Platforms,
}

impl MyEguiApp {
    pub fn new() -> Self {
        let runtime = Runtime::new().unwrap();
        let settings = Settings::new().expect("We must be able to load our settings");
        Self {
            selected_menu: Menues::Import,
            settings: settings.clone(),
            rt: runtime,
            games_to_sync: watch::channel(FetcStatus::NeedsFetched).1,
            ui_images: UiImages::default(),
            status_reciever: watch::channel(SyncProgress::NotStarted).1,
            image_selected_state: ImageSelectState::default(),
            backup_state: BackupState::default(),
            disconect_state: DiconnectState::default(),
            rename_map: get_rename_map(),
            current_edit: Option::None,
            platforms: get_platforms(),
        }
    }
}

fn get_rename_map() -> HashMap<u32, String> {
    try_get_rename_map().unwrap_or_default()
}

fn try_get_rename_map() -> Result<HashMap<u32, String>, Box<dyn Error>> {
    let rename_map = get_renames_file();
    let file_content = std::fs::read_to_string(rename_map)?;
    let deserialized = serde_json::from_str(&file_content)?;
    Ok(deserialized)
}

#[derive(PartialEq)]
enum Menues {
    Import,
    Settings,
    Images,
    Backup,
    Disconnect,
}

impl Default for Menues {
    fn default() -> Menues {
        Menues::Import
    }
}

impl App for MyEguiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
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

                let mut changed = ui
                    .selectable_value(&mut self.selected_menu, Menues::Import, "Import Games")
                    .changed();
                if self.settings.steamgrid_db.auth_key.is_some() {
                    changed = changed
                        || ui
                            .selectable_value(&mut self.selected_menu, Menues::Images, "Images")
                            .changed();
                }
                changed = changed
                    || ui
                        .selectable_value(&mut self.selected_menu, Menues::Settings, "Settings")
                        .changed();

                changed = changed
                    || ui
                        .selectable_value(&mut self.selected_menu, Menues::Backup, "Backup")
                        .changed();

                changed = changed
                    || ui
                        .selectable_value(&mut self.selected_menu, Menues::Disconnect, "Disconnect")
                        .changed();

                if changed {
                    self.backup_state.available_backups = None;
                }
                if changed && self.selected_menu == Menues::Settings {
                    //We reset games here, since user might change settings
                    self.games_to_sync = watch::channel(FetcStatus::NeedsFetched).1;
                }
            });

        if self.selected_menu == Menues::Settings {
            egui::TopBottomPanel::new(egui::panel::TopBottomSide::Bottom, "Bottom Panel")
                .frame(frame)
                .show(ctx, |ui| {
                    let texture = self.get_save_image(ui);
                    let size = texture.size_vec2();
                    let save_button = ImageButton::new(texture, size * 0.5);

                    if ui.add(save_button).on_hover_text("Save settings").clicked() {
                        MyEguiApp::save_settings_to_file(&self.settings.clone());
                    }
                });
        }
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
                        SyncProgress::FindingImages => ("Searching for images".to_string(), true),
                        SyncProgress::DownloadingImages { to_download } => {
                            (format!("Downloading {} images ", to_download), true)
                        }
                        SyncProgress::Done => ("Done importing games".to_string(), false),
                    };
                    if syncing {
                        ui.ctx().request_repaint();
                    }
                    if !status_string.is_empty() {
                        if syncing {
                            ui.horizontal(|c| {
                                c.spinner();
                                c.label(&status_string);
                            });
                        } else {
                            ui.label(&status_string);
                        }
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
                        self.run_sync(false);
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
                Menues::Images => {
                    self.render_ui_images(ui);
                }
                Menues::Backup => {
                    self.render_backup(ui);
                }
                Menues::Disconnect => {
                    self.render_disconnect(ui);
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
            ui.ctx().load_texture(
                "import_image",
                get_import_image(),
                egui::TextureFilter::Linear,
            )
        })
    }

    fn get_save_image(&mut self, ui: &mut egui::Ui) -> &mut TextureHandle {
        self.ui_images.save_button.get_or_insert_with(|| {
            // Load the texture only once.
            ui.ctx()
                .load_texture("save_image", get_save_image(), egui::TextureFilter::Linear)
        })
    }

    fn get_logo_image(&mut self, ui: &mut egui::Ui) -> &mut TextureHandle {
        self.ui_images.logo_32.get_or_insert_with(|| {
            // Load the texture only once.
            ui.ctx()
                .load_texture("logo32", get_logo(), egui::TextureFilter::Linear)
        })
    }
}

fn setup(ctx: &egui::Context) {
    #[cfg(target_family = "unix")]
    ctx.set_pixels_per_point(1.0);

    let mut style: egui::Style = (*ctx.style()).clone();
    create_style(&mut style);
    ctx.set_style(style);
}
pub fn run_sync() {
    let mut app = MyEguiApp::new();
    app.run_sync(true);
}

pub fn run_ui(args: Vec<String>) {
    let app = MyEguiApp::new();
    let no_v_sync = args.contains(&"--no-vsync".to_string());
    let native_options = eframe::NativeOptions {
        initial_window_size: Some(egui::Vec2 { x: 1280., y: 800. }),
        icon_data: Some(get_logo_icon()),
        vsync: !no_v_sync,
        ..Default::default()
    };
    eframe::run_native(
        "BoilR",
        native_options,
        Box::new(|cc| {
            setup(&cc.egui_ctx);
            Box::new(app)
        }),
    );
}
