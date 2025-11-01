use std::path::Path;

use egui::ImageButton;

use crate::ui::images::{
    gametype::GameType, hasimagekey::HasImageKey, image_select_state::ImageSelectState,
    useraction::UserAction,
};
use boilr_core::steamgriddb::ImageType;

const MAX_WIDTH: f32 = 300.;

pub fn render_page_shortcut_select_image_type(
    ui: &mut egui::Ui,
    state: &ImageSelectState,
) -> Option<UserAction> {
    let shortcut = &state.selected_shortcut.as_ref();
    let user_path = state
        .steam_user
        .as_ref()
        .map(|user| &user.steam_user_data_folder);
    if let (Some(shortcut), Some(user_path)) = (shortcut, user_path) {
        let thumbnail = |ui: &mut egui::Ui, image_type: &ImageType| {
            if render_thumbnail(ui, shortcut, image_type, user_path) {
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
    } else {
        None
    }
}

fn render_thumbnail(
    ui: &mut egui::Ui,
    shortcut: &GameType,
    image_type: &ImageType,
    user_path: &String,
) -> bool {
    let (_path, key) = shortcut.key(image_type, Path::new(&user_path));
    let text = format!("Pick {} image", image_type.name());
    let image = egui::Image::new(format!("file://{}", key))
        .max_width(MAX_WIDTH)
        .shrink_to_fit();
    let calced = image.calc_size(
        egui::Vec2 {
            x: MAX_WIDTH,
            y: f32::INFINITY,
        },
        image.size(),
    );
    let button = ImageButton::new(image);
    ui.add_sized(calced, button).on_hover_text(text).clicked()
}
