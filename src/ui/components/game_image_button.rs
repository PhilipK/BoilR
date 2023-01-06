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
    render_possible_image(
        ui,
        image_handles,
        path,
        max_width,
        text,
        &ImageType::Grid,
        None,
        None,
    )
}

pub fn render_image_from_path_image_type(
    ui: &mut egui::Ui,
    image_handles: &ImageHandles,
    path: &Path,
    max_width: f32,
    text: &str,
    image_type: &ImageType,
) -> bool {
    render_possible_image(
        ui,
        image_handles,
        path,
        max_width,
        text,
        image_type,
        None,
        None,
    )
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
