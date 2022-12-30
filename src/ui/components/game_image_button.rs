use std::path::Path;

use egui::{Button, ImageButton};
use futures::executor::block_on;
use tokio::runtime::Runtime;

use crate::steamgriddb::{ImageType, ToDownload};
use crate::ui::images::{clamp_to_width, ImageHandles, TextureDownloadState};
use crate::ui::ui_images::load_image_from_path;


pub fn render_image_from_path(
    ui: &mut egui::Ui,
    image_handles: &ImageHandles,
    path: &Path,
    max_width: f32,
    text: &str,
) -> bool {
    render_possible_image(ui, image_handles, path, max_width, text, None, None, None)
}

pub fn render_image_from_path_or_url(
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
        Some(image_type),
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
    image_type: Option<&ImageType>,
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
                    let image_data = load_image_from_path(&path);
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
                    let button =
                        ui.add_sized([max_width, max_width * 1.6], Button::new(text).wrap(true));
                    if button.clicked() {
                        return true;
                    }
                }
            }
        }
        None => {
            if !path.exists() && url.is_none() {
                image_handles.insert(image_key, TextureDownloadState::Failed);
            } else {
                //We need to start a download
                //Redownload if file is too small
                if !path.exists()
                    || std::fs::metadata(path).map(|m| m.len()).unwrap_or_default() < 2
                {
                    image_handles.insert(image_key.clone(), TextureDownloadState::Downloading);
                    let to_download = ToDownload {
                        path: path.to_path_buf(),
                        url: url.unwrap().to_string(),
                        app_name: "Thumbnail".to_string(),
                        image_type: *image_type.unwrap(),
                    };
                    let image_handles = image_handles.clone();
                    let image_key = image_key.clone();
                    if let Some(rt) = rt {
                        rt.spawn_blocking(move || {
                            block_on(crate::steamgriddb::download_to_download(&to_download))
                                .unwrap();
                            image_handles.insert(image_key, TextureDownloadState::Downloaded);
                        });
                    }
                } else {
                    image_handles.insert(image_key.clone(), TextureDownloadState::Downloaded);
                }
            }
        }
    }
    false
}
