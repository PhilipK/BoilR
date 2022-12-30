use std::path::Path;

use crate::ui::images::{
    gametype::GameType, hasimagekey::HasImageKey, image_select_state::ImageSelectState,
    useraction::UserAction, ImageHandles,
};
use crate::{steamgriddb::ImageType, ui::components::render_image_from_path_image_type};

const MAX_WIDTH: f32 = 300.;

pub fn render_page_shortcut_select_image_type(
    ui: &mut egui::Ui,
    state: &ImageSelectState,
) -> Option<UserAction> {
    let shortcut = state.selected_shortcut.as_ref().unwrap();
    let user_path = &state.steam_user.as_ref().unwrap().steam_user_data_folder;

    let thumbnail = |ui: &mut egui::Ui, image_type: &ImageType| {
        if render_thumbnail(ui, &state.image_handles, shortcut, image_type, user_path) {
            Some(UserAction::ImageTypeSelected(*image_type))
        } else {
            None
        }
    };
    let x = if ui.available_width() > MAX_WIDTH * 3. {
        ui.horizontal(|ui| {
            let x = ui.vertical(|ui| thumbnail(ui, &ImageType::Grid)).inner;
            if x.is_some() {
                return x;
            }
            let x = ui
                .vertical(|ui| {
                    let types = &[ImageType::Hero, ImageType::WideGrid, ImageType::Logo];
                    types
                        .iter()
                        .flat_map(|image_type| thumbnail(ui, image_type))
                        .next()
                })
                .inner;
            if x.is_some() {
                return x;
            }
            ui.vertical(|ui| {
                let types = &[ImageType::Icon, ImageType::BigPicture];
                types
                    .iter()
                    .flat_map(|image_type| thumbnail(ui, image_type))
                    .next()
            })
            .inner
        })
        .inner
    } else {
        let types = ImageType::all();
        types
            .iter()
            .flat_map(|image_type| thumbnail(ui, image_type))
            .next()
    };

    if ui
        .button("Click here if the images are for a wrong game")
        .clicked()
    {
        return Some(UserAction::CorrectGridId);
    }
    x
}

fn render_thumbnail(
    ui: &mut egui::Ui,
    image_handles: &ImageHandles,
    shortcut: &GameType,
    image_type: &ImageType,
    user_path: &String,
) -> bool {
    let (path, _key) = shortcut.key(image_type, Path::new(&user_path));
    let text = format!("Pick {} image", image_type.name());
    render_image_from_path_image_type(
        ui,
        image_handles,
        path.as_path(),
        MAX_WIDTH,
        &text,
        image_type,
    )
}
