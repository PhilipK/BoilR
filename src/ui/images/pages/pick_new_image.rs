use std::path::Path;

use egui::{Grid, };
use futures::executor::block_on;
use tokio::{ sync::watch};

use crate::{
    steamgriddb::{get_image_extension, ImageType, ToDownload},
    ui::{
        images::{
            constants::MAX_WIDTH,
            hasimagekey::HasImageKey,
            image_select_state::{ ImageSelectState},
            possible_image::PossibleImage,
            useraction::UserAction,
        },
        FetcStatus, MyEguiApp, components::render_image_from_path_or_url,
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
                        let path = image.thumbnail_path.as_path();
                        if render_image_from_path_or_url(
                            ui,
                            &state.image_handles,
                            &path,
                            column_width,
                            &image.full_url,
                            image_type,
                            &app.rt,
                            &image.thumbnail_url,
                        ) {
                            return Some(image.clone());
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
                if let Some(x) = x {
                return Some(UserAction::ImageSelected(x));
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

pub fn handle_image_selected(app: &mut MyEguiApp, image: PossibleImage) {
    //We must have a user here
    let user = app.image_selected_state.steam_user.as_ref().unwrap();
    let selected_image_type = app
        .image_selected_state
        .image_type_selected
        .as_ref()
        .unwrap();
    let selected_shortcut = app.image_selected_state.selected_shortcut.as_ref().unwrap();

    let ext = get_image_extension(&image.mime);
    let to_download_to_path = Path::new(&user.steam_user_data_folder)
        .join("config")
        .join("grid")
        .join(selected_image_type.file_name(selected_shortcut.app_id(), ext));

    //Delete old possible images

    let data_folder = Path::new(&user.steam_user_data_folder);

    //Keep deleting images of this type untill we don't find any more
    let mut path = get_shortcut_image_path(app, data_folder);
    while Path::new(&path).exists() {
        let _ = std::fs::remove_file(&path);
        path = get_shortcut_image_path(app, data_folder);
    }

    //Put the loaded thumbnail into the image handler map, we can use that for preview
    let full_image_key = to_download_to_path.to_string_lossy().to_string();
    let _ = app
        .image_selected_state
        .image_handles
        .remove(&full_image_key);
    let thumbnail_key = image.thumbnail_path.to_string_lossy().to_string();
    let thumbnail = app
        .image_selected_state
        .image_handles
        .remove(&thumbnail_key);
    if let Some((_key, thumbnail)) = thumbnail {
        app.image_selected_state
            .image_handles
            .insert(full_image_key, thumbnail);
    }

    let app_name = selected_shortcut.name();
    let to_download = ToDownload {
        path: to_download_to_path,
        url: image.full_url.clone(),
        app_name: app_name.to_string(),
        image_type: *selected_image_type,
    };
    app.rt.spawn_blocking(move || {
        let _ = block_on(crate::steamgriddb::download_to_download(&to_download));
    });

    clear_loaded_images(app);
    {
        app.image_selected_state.image_type_selected = None;
        app.image_selected_state.image_options = watch::channel(FetcStatus::NeedsFetched).1;
    }
}

fn get_shortcut_image_path(app: &MyEguiApp, data_folder: &Path) -> String {
    app.image_selected_state
        .selected_shortcut
        .as_ref()
        .unwrap()
        .key(
            &app.image_selected_state.image_type_selected.unwrap(),
            data_folder,
        )
        .1
}

fn clear_loaded_images(app: &mut MyEguiApp) {
    if let FetcStatus::Fetched(options) = &*app.image_selected_state.image_options.borrow() {
        for option in options {
            let key = option.thumbnail_path.to_string_lossy().to_string();
            app.image_selected_state.image_handles.remove(&key);
        }
    }
}
