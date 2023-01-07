use std::path::{Path, PathBuf};

use egui::{Button, ImageButton};
use futures::executor::block_on;
use tokio::runtime::Runtime;

use crate::steamgriddb::{ImageType, ToDownload};
use crate::ui::images::{clamp_to_width, ImageHandles, TextureDownloadState};
use crate::ui::ui_images::load_image_from_path;

pub struct DownloadedGameImageButton {
    url: String,
    path: PathBuf,
    max_width: f32,
    text: String,
    image_type: ImageType,
}

impl DownloadedGameImageButton {
    pub fn new(path: &Path, url: &str) -> Self {
        Self {
            url: url.to_string(),
            max_width: 200.0,
            path: path.to_path_buf(),
            text: Default::default(),
            image_type: ImageType::Grid,
        }
    }
    pub fn width(&mut self, max_width: f32) -> &mut Self {
        self.max_width = max_width;
        self
    }
    
    pub fn text(&mut self, text: &str) -> &mut Self {
        self.text = text.to_string();
        self
    }

    pub fn show(&self, ui: &mut egui::Ui, image_handles: &ImageHandles, rt: &Runtime) -> bool {
        render_image_from_path_or_url(
            ui,
            image_handles,
            self.path.as_path(),
            self.max_width,
            &self.text,
            &self.image_type,
            rt,
            &self.url,
        )
    }
}
impl Default for DownloadedGameImageButton {
    fn default() -> Self {
        Self::new(Path::new(""), "")
    }
}

pub struct GameImageButton {
    path: PathBuf,
    max_width: f32,
    text: String,
    image_type: ImageType,
}

impl Default for GameImageButton {
    fn default() -> Self {
        GameImageButton::new(Path::new(""))
    }
}

impl GameImageButton {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            max_width: 200.0,
            text: Default::default(),
            image_type: ImageType::Grid,
        }
    }

    pub fn width(&mut self, max_width: f32) -> &mut Self {
        self.max_width = max_width;
        self
    }
    pub fn image_type(&mut self, image_type: &ImageType) -> &mut Self {
        self.image_type = *image_type;
        self
    }
    pub fn text(&mut self, text: &str) -> &mut Self {
        self.text = text.to_string();
        self
    }

    pub fn show(&self, ui: &mut egui::Ui, image_handles: &ImageHandles) -> bool {
        render_possible_image(
            ui,
            image_handles,
            self.path.as_path(),
            self.max_width,
            self.text.as_str(),
            &self.image_type,
            None,
            None,
        )
    }
}

fn render_image_from_path_or_url(
    ui: &mut egui::Ui,
    image_handles: &ImageHandles,
    path: &Path,
    max_width: f32,
    text: &str,
    image_type: &ImageType,
    rt: &Runtime,
    url: &str,
) -> bool {
    render_possible_image(
        ui,
        image_handles,
        path,
        max_width,
        text,
        image_type,
        Some(rt),
        Some(url),
    )
}

fn render_possible_image(
    ui: &mut egui::Ui,
    image_handles: &ImageHandles,
    path: &Path,
    max_width: f32,
    text: &str,
    image_type: &ImageType,
    rt: Option<&Runtime>,
    url: Option<&str>,
) -> bool {
    let image_key = path.to_string_lossy().to_string();

    match image_handles.get_mut(&image_key) {
        Some(mut state) => {
            match state.value() {
                TextureDownloadState::Downloading => {
                    ui.ctx().request_repaint();
                    //nothing to do,just wait
                    ui.spinner();
                }
                TextureDownloadState::Downloaded => {
                    //Need to load
                    let image_data = load_image_from_path(path);
                    match image_data {
                        Ok(image_data) => {
                            let handle = ui.ctx().load_texture(
                                &image_key,
                                image_data,
                                egui::TextureOptions::LINEAR,
                            );
                            *state.value_mut() = TextureDownloadState::Loaded(handle);
                            ui.spinner();
                        }
                        Err(_) => *state.value_mut() = TextureDownloadState::Failed,
                    }
                    ui.ctx().request_repaint();
                }
                TextureDownloadState::Loaded(texture_handle) => {
                    //need to show
                    let mut size = texture_handle.size_vec2();
                    clamp_to_width(&mut size, max_width);
                    let image_button = ImageButton::new(texture_handle, size);
                    if ui
                        .add_sized(size, image_button)
                        .on_hover_text(text)
                        .clicked()
                    {
                        return true;
                    }
                }
                TextureDownloadState::Failed => {
                    let button = ui.add_sized(
                        [max_width, max_width * image_type.ratio()],
                        Button::new(text).wrap(true),
                    );
                    if button.clicked() {
                        return true;
                    }
                }
            }
        }
        None => {
            match url {
                Some(url) => {
                    //We need to start a download
                    //Redownload if file is too small
                    if !path.exists()
                        || std::fs::metadata(path).map(|m| m.len()).unwrap_or_default() < 2
                    {
                        image_handles.insert(image_key.clone(), TextureDownloadState::Downloading);
                        let to_download = ToDownload {
                            path: path.to_path_buf(),
                            url: url.to_string(),
                            app_name: "Thumbnail".to_string(),
                            image_type: *image_type,
                        };
                        let image_handles = image_handles.clone();
                        let image_key = image_key.clone();
                        if let Some(rt) = rt {
                            rt.spawn_blocking(move || {
                                match block_on(crate::steamgriddb::download_to_download(
                                    &to_download,
                                )) {
                                    Ok(_) => {
                                        image_handles
                                            .insert(image_key, TextureDownloadState::Downloaded);
                                    }
                                    Err(err) => {
                                        println!(
                                            "Failed downloading image {} error: {:?}",
                                            to_download.url, err
                                        );
                                        image_handles
                                            .insert(image_key, TextureDownloadState::Failed);
                                    }
                                }
                            });
                        }
                    } else {
                        image_handles.insert(image_key.clone(), TextureDownloadState::Downloaded);
                    }
                }
                None => {
                    //Not possible to download
                    if !path.exists() {
                        image_handles.insert(image_key, TextureDownloadState::Failed);
                    }
                }
            }
        }
    }
    false
}
