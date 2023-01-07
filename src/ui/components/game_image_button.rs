use std::path::{Path, PathBuf};

use egui::{Button, ImageButton};
use futures::executor::block_on;
use tokio::runtime::Runtime;

use crate::steamgriddb::{ToDownload, ImageType};
use crate::ui::images::{clamp_to_width, ImageHandles, TextureDownloadState};
use crate::ui::ui_images::load_image_from_path;

pub struct GmeButton {
    path: PathBuf,
    max_width: f32,
    text: String,
    image_type: ImageType,
}

impl GameButton {
    pub fn new(path: &Path) -> Self {
        Self {
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

    pub fn image_type(&mut self, image_type:&ImageType) -> &mut Self{
        self.image_type = *image_type;
        self
    }

    pub fn show_download(
        &self,
        ui: &mut egui::Ui,
        image_handles: &ImageHandles,
        rt: &Runtime,
        url: &str,
    ) -> bool {
        self.render_possible_image(ui, image_handles, Some(rt), Some(url))
    }

    pub fn show(&self, ui: &mut egui::Ui, image_handles: &ImageHandles) -> bool {
        self.render_possible_image(ui, image_handles, None, None)
    }

    fn render_possible_image(
        &self,
        ui: &mut egui::Ui,
        image_handles: &ImageHandles,
        rt: Option<&Runtime>,
        url: Option<&str>,
    ) -> bool {
        {
            let path = &self.path;
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
                            clamp_to_width(&mut size, self.max_width);
                            let image_button = ImageButton::new(texture_handle, size);
                            if ui
                                .add_sized(size, image_button)
                                .on_hover_text(&self.text)
                                .clicked()
                            {
                                return true;
                            }
                        }
                        TextureDownloadState::Failed => {
                            let button = ui.add_sized(
                                [self.max_width, self.max_width * self.image_type.ratio()],
                                Button::new(&self.text).wrap(true),
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
                            download_image(
                                path,
                                image_handles,
                                &image_key,
                                url,
                                &self.image_type,
                                rt,
                            );
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
    }
}
impl Default for GameButton {
    fn default() -> Self {
        Self::new(Path::new(""))
    }
}


fn download_image(
    path: &Path,
    image_handles: &ImageHandles,
    image_key: &str,
    url: &str,
    image_type: &ImageType,
    rt: Option<&Runtime>,
) {
    //We need to start a download
    //Redownload if file is too small
    if !path.exists() || std::fs::metadata(path).map(|m| m.len()).unwrap_or_default() < 2 {
        image_handles.insert(image_key.to_string(), TextureDownloadState::Downloading);
        let to_download = ToDownload {
            path: path.to_path_buf(),
            url: url.to_string(),
            app_name: "Thumbnail".to_string(),
            image_type: *image_type,
        };
        let image_handles = image_handles.clone();
        let image_key = image_key.to_string();
        if let Some(rt) = rt {
            rt.spawn_blocking(move || {
                match block_on(crate::steamgriddb::download_to_download(&to_download)) {
                    Ok(_) => {
                        image_handles.insert(image_key, TextureDownloadState::Downloaded);
                    }
                    Err(err) => {
                        println!(
                            "Failed downloading image {} error: {:?}",
                            to_download.url, err
                        );
                        image_handles.insert(image_key, TextureDownloadState::Failed);
                    }
                }
            });
        }
    } else {
        image_handles.insert(image_key.to_string(), TextureDownloadState::Downloaded);
    }
}
