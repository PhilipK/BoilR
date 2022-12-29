use std::path::Path;

use egui::{Button, ImageButton};
use steam_shortcuts_util::shortcut::ShortcutOwned;

use crate::{
    steam::SteamUsersInfo,
    steamgriddb::ImageType,
    ui::{
        images::{
            gametype::GameType, hasimagekey::HasImageKey, image_resize::clamp_to_width,
            texturestate::TextureDownloadState, useraction::UserAction,
        },
        MyEguiApp,
    },
};

pub fn render_page_shortcut_images_overview(app: &MyEguiApp, ui: &mut egui::Ui) -> Option<UserAction> {
    let shortcuts = &app.image_selected_state.user_shortcuts;

    let width = ui.available_size().x;
    let column_width = 100.;
    let column_padding = 23.;
    let columns = (width / (column_width + column_padding)).floor() as u32;
    let mut cur_column = 0;
    match shortcuts {
        Some(shortcuts) => {
            let user_info = &app.image_selected_state.steam_user.as_ref().unwrap();
            if let Some(action) = egui::Grid::new("ui_images")
                .show(ui, |ui| {
                    for shortcut in shortcuts {
                        let action = render_image(app, shortcut, user_info, column_width, ui);
                        if action.is_some() {
                            return action;
                        }
                        cur_column += 1;
                        if cur_column >= columns {
                            cur_column = 0;
                            ui.end_row();
                        }
                    }
                    ui.end_row();
                    None
                })
                .inner
            {
                return action;
            }
        }
        None => {
            ui.label("Could not find any shortcuts");
        }
    }
    None
}

fn render_image(
    app: &MyEguiApp,
    shortcut: &ShortcutOwned,
    user_info: &&SteamUsersInfo,
    column_width: f32,
    ui: &mut egui::Ui,
) -> Option<Option<UserAction>> {
    let (_, key) = shortcut.key(
        &ImageType::Grid,
        Path::new(&user_info.steam_user_data_folder),
    );
    let mut clicked = false;

    let texture = app.image_selected_state.image_handles.get(&key);
    if let Some(texture) = texture {
        if let TextureDownloadState::Loaded(texture) = &texture.value() {
            let mut size = texture.size_vec2();
            clamp_to_width(&mut size, column_width);
            let image_button = ImageButton::new(texture, size);
            clicked = ui
                .add(image_button)
                .on_hover_text(&shortcut.app_name)
                .clicked();
        }
    } else {
        let button = ui.add_sized(
            [column_width, column_width * 1.6],
            Button::new(&shortcut.app_name).wrap(true),
        );
        clicked = clicked || button.clicked();
    }

    if clicked {
        return Some(Some(UserAction::ShortcutSelected(GameType::Shortcut(
            shortcut.clone(),
        ))));
    }
    None
}
