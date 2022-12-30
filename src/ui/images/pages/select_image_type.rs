use std::path::Path;

use egui::ImageButton;

use crate::{
    steamgriddb::ImageType,
    ui::{
        components::render_image_from_path_image_type,
        images::{
            gametype::GameType, hasimagekey::HasImageKey, image_resize::clamp_to_width,
            image_select_state::ImageSelectState, texturestate::TextureDownloadState,
            useraction::UserAction, ImageHandles,
        },
    },
};

const MAX_WIDTH: f32 = 300.;

pub fn render_page_shortcut_select_image_type(
    ui: &mut egui::Ui,
    state: &ImageSelectState,
) -> Option<UserAction> {
    let shortcut = state.selected_shortcut.as_ref().unwrap();
    let user_path = &state.steam_user.as_ref().unwrap().steam_user_data_folder;

    let thumbnail = |ui: &mut egui::Ui, image_type: &ImageType| {
        ui.label(image_type.name());
        if render_thumbnail_new(ui, &state.image_handles, shortcut, image_type, user_path) {
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
        render_image_types_as_list(shortcut, user_path, state, ui)
    };

    if ui
        .button("Click here if the images are for a wrong game")
        .clicked()
    {
        return Some(UserAction::CorrectGridId);
    }
    x
}

fn render_image_types_as_list(
    shortcut: &GameType,
    user_path: &String,
    state: &ImageSelectState,
    ui: &mut egui::Ui,
) -> Option<UserAction> {
    let types = ImageType::all();
    for image_type in types {
        let texture = texture_from_iamge_type(shortcut, image_type, user_path, state);
        let response = ui
            .vertical(|ui| {
                ui.label(image_type.name());
                render_thumbnail(ui, texture)
            })
            .inner;
        if response.clicked() {
            return Some(UserAction::ImageTypeSelected(*image_type));
        }
    }
    None
}

fn texture_from_iamge_type(
    shortcut: &GameType,
    image_type: &ImageType,
    user_path: &String,
    state: &ImageSelectState,
) -> Option<egui::TextureHandle> {
    let (_path, key) = shortcut.key(image_type, Path::new(&user_path));
    state.image_handles.get(&key).and_then(|k| match k.value() {
        TextureDownloadState::Loaded(texture) => Some(texture.clone()),
        _ => None,
    })
}

fn render_thumbnail_new(
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

fn render_thumbnail(ui: &mut egui::Ui, image: Option<egui::TextureHandle>) -> egui::Response {
    if let Some(texture) = image {
        let mut size = texture.size_vec2();
        clamp_to_width(&mut size, MAX_WIDTH);
        let image_button = ImageButton::new(&texture, size);
        ui.add(image_button)
    } else {
        ui.button("Pick an image")
    }
}
