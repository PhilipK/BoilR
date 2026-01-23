use std::path::{Path, PathBuf};

use egui::{Grid, ImageButton};
use futures::executor::block_on;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

use crate::{
    steamgriddb::{get_image_extension, ImageType, ToDownload},
    ui::{
        images::{
            constants::MAX_WIDTH, hasimagekey::HasImageKey, image_select_state::ImageSelectState,
            possible_image::PossibleImage, useraction::UserAction,
        },
        FetchStatus, MyEguiApp,
    },
};

pub fn render_page_pick_image(
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
        FetchStatus::Fetched(images) => {
            let x = Grid::new("ImageThumbnailSelectGrid")
                .spacing([column_padding, column_padding])
                .show(ui, |ui| {
                    for possible_image in images {
                        let image = egui::Image::new(&possible_image.thumbnail_url)
                            .max_width(column_width)
                            .shrink_to_fit();
                        let calced = image.calc_size(
                            egui::Vec2 {
                                x: column_width,
                                y: f32::INFINITY,
                            },
                            image.size(),
                        );
                        let button = ImageButton::new(image);
                        let clicked = ui
                            .add_sized(calced, button)
                            .on_hover_text("Pick image")
                            .clicked();
                        if clicked {
                            return Some(possible_image);
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
                return Some(UserAction::ImageSelected(x.clone()));
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
    info!(url = %image.full_url, "Image selected for download");
    // We must have a user here
    let state = &app.image_selected_state;
    if let (Some(user), Some(selected_image_type), Some(selected_shortcut)) = (
        state.steam_user.as_ref(),
        state.image_type_selected.as_ref(),
        state.selected_shortcut.as_ref(),
    ) {
        let get_image_extension = &get_image_extension(&image.mime);
        let ext = get_image_extension;
        let to_download_to_path = Path::new(&user.steam_user_data_folder)
            .join("config")
            .join("grid")
            .join(selected_image_type.file_name(selected_shortcut.app_id(), ext));

        debug!(
            app_name = %selected_shortcut.name(),
            image_type = ?selected_image_type,
            destination = %to_download_to_path.display(),
            "Downloading image"
        );

        delete_images_of_type(user, selected_shortcut, selected_image_type);

        let app_name = selected_shortcut.name();
        let to_download = ToDownload {
            path: to_download_to_path.clone(),
            url: image.full_url.clone(),
            app_name: app_name.to_string(),
            image_type: *selected_image_type,
        };
        app.rt.spawn_blocking(move || {
            match block_on(crate::steamgriddb::download_to_download(&to_download)) {
                Ok(_) => info!(path = %to_download.path.display(), "Image downloaded successfully"),
                Err(e) => error!(error = %e, path = %to_download.path.display(), "Failed to download image"),
            }
        });

        {
            app.image_selected_state.image_type_selected = None;
            app.image_selected_state.image_options = watch::channel(FetchStatus::NeedsFetched).1;
        }
    } else {
        warn!("Cannot download image: missing user, image type, or shortcut selection");
    }
}

fn delete_images_of_type(
    user: &crate::steam::SteamUsersInfo,
    selected_shortcut: &crate::ui::images::gametype::GameType,
    selected_image_type: &ImageType,
) {
    //Delete old possible images
    let data_folder = Path::new(&user.steam_user_data_folder);
    //Keep deleting images of this type untill we don't find any more
    let mut path = image_path(selected_shortcut, selected_image_type, data_folder);
    while path.exists() {
        let _ = std::fs::remove_file(path);
        path = image_path(selected_shortcut, selected_image_type, data_folder);
    }
}

fn image_path(
    selected_shortcut: &crate::ui::images::gametype::GameType,
    selected_image_type: &ImageType,
    data_folder: &Path,
) -> PathBuf {
    selected_shortcut.key(selected_image_type, data_folder).0
}
