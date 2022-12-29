use egui::{Grid, ImageButton};
use futures::executor::block_on;

use crate::{
    steamgriddb::{ImageType, ToDownload},
    ui::{
        images::{
            constants::MAX_WIDTH, image_resize::clamp_to_width,
            image_select_state::ImageSelectState, texturestate::TextureDownloadState,
            useraction::UserAction,
        },
        ui_images::load_image_from_path,
        FetcStatus, MyEguiApp,
    },
};

pub fn render_page_pick_image(
    app: &MyEguiApp,
    ui: &mut egui::Ui,
    image_type: &ImageType,
    state: &ImageSelectState,
) -> Option<UserAction> {
    ui.label(image_type.name());

    if let Some(action) = ui
        .horizontal(|ui| {
            if ui
                .small_button("Clear image?")
                .on_hover_text("Click here to clear the image")
                .clicked()
            {
                return Some(UserAction::ImageTypeCleared(*image_type, false));
            }

            if ui
                .small_button("Stop downloading this image?")
                .on_hover_text("Stop downloading this type of image for this shortcut at all")
                .clicked()
            {
                return Some(UserAction::ImageTypeCleared(*image_type, true));
            }
            None
        })
        .inner
    {
        return Some(action);
    }
    let column_padding = 10.;
    let column_width = MAX_WIDTH * 0.75;
    let width = ui.available_width();
    let columns = (width / (column_width + column_padding)).floor() as u32;
    let mut column = 0;
    match &*state.image_options.borrow() {
        FetcStatus::Fetched(images) => {
            let x = Grid::new("ImageThumbnailSelectGrid")
                .spacing([column_padding, column_padding])
                .show(ui, |ui| {
                    for image in images {
                        let image_key =
                            image.thumbnail_path.as_path().to_string_lossy().to_string();

                        match state.image_handles.get_mut(&image_key) {
                            Some(mut state) => {
                                match state.value() {
                                    TextureDownloadState::Downloading => {
                                        ui.ctx().request_repaint();
                                        //nothing to do,just wait
                                        ui.spinner();
                                    }
                                    TextureDownloadState::Downloaded => {
                                        //Need to load
                                        let image_data =
                                            load_image_from_path(&image.thumbnail_path);
                                        match image_data {
                                            Ok(image_data) => {
                                                let handle = ui.ctx().load_texture(
                                                    &image_key,
                                                    image_data,
                                                    egui::TextureOptions::LINEAR,
                                                );
                                                *state.value_mut() =
                                                    TextureDownloadState::Loaded(handle);
                                                ui.spinner();
                                            }
                                            Err(_) => {
                                                *state.value_mut() = TextureDownloadState::Failed
                                            }
                                        }
                                        ui.ctx().request_repaint();
                                    }
                                    TextureDownloadState::Loaded(texture_handle) => {
                                        //need to show
                                        let mut size = texture_handle.size_vec2();
                                        clamp_to_width(&mut size, column_width);
                                        let image_button = ImageButton::new(texture_handle, size);
                                        if ui.add_sized(size, image_button).clicked() {
                                            return Some(UserAction::ImageSelected(image.clone()));
                                        }
                                    }
                                    TextureDownloadState::Failed => {
                                        ui.label("Failed to load image");
                                    }
                                }
                            }
                            None => {
                                //We need to start a download
                                let image_handles = &app.image_selected_state.image_handles;
                                let path = &image.thumbnail_path;
                                //Redownload if file is too small
                                if !path.exists()
                                    || std::fs::metadata(path).map(|m| m.len()).unwrap_or_default()
                                        < 2
                                {
                                    image_handles.insert(
                                        image_key.clone(),
                                        TextureDownloadState::Downloading,
                                    );
                                    let to_download = ToDownload {
                                        path: path.clone(),
                                        url: image.thumbnail_url.clone(),
                                        app_name: "Thumbnail".to_string(),
                                        image_type: *image_type,
                                    };
                                    let image_handles = image_handles.clone();
                                    let image_key = image_key.clone();
                                    app.rt.spawn_blocking(move || {
                                        block_on(crate::steamgriddb::download_to_download(
                                            &to_download,
                                        ))
                                        .unwrap();
                                        image_handles
                                            .insert(image_key, TextureDownloadState::Downloaded);
                                    });
                                } else {
                                    image_handles.insert(
                                        image_key.clone(),
                                        TextureDownloadState::Downloaded,
                                    );
                                }
                            }
                        }
                        column += 1;
                        if column >= columns {
                            column = 0;
                            ui.end_row();
                        }
                    }

                    None
                })
                .inner;
            if x.is_some() {
                return x;
            }
        }
        _ => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Finding possible images");
            });
            ui.ctx().request_repaint();
        }
    }
    None
}
