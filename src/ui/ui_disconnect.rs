use egui::ScrollArea;

use super::ui_colors::*;
use super::MyEguiApp;
use crate::steam::get_shortcuts_for_user;
use crate::steam::get_shortcuts_paths;
use crate::steam::ShortcutInfo;
use crate::sync::disconnect_shortcut;
use crate::sync::IsBoilRShortcut;

#[derive(Default)]
pub struct DisconnectState {
    pub connected_shortcuts: Option<Result<Vec<ShortcutInfo>, String>>,
}

impl MyEguiApp {
    pub fn render_disconnect(&mut self, ui: &mut egui::Ui) {
        let steam_settings = self.settings.steam.clone();
        let users_info = self
            .disconnect_state
            .connected_shortcuts
            .get_or_insert_with(|| {
                let users = get_shortcuts_paths(&steam_settings)
                    .map_err(|e| format!("Getting shortcut paths failed: {e}"));
                users.map(|users| {
                    let mut user_info = vec![];
                    for user in users {
                        let shortcut_info = get_shortcuts_for_user(&user);
                        if let Ok(shortcut_info) = shortcut_info {
                            user_info.push(shortcut_info);
                        }
                    }
                    user_info
                })
            });

        ui.heading("Add a disconnected Shortcuts");
        ui.label("In this section you can add a shortcut that BoilR is not in control of.");
        ui.label("This prevents BoilR from deleting or updating a shortcut it orignally added.");
        ui.label(
            "This is useful if you want to manully edit a shortcut after BoilR has imported it.",
        );

        ui.add_space(super::SECTION_SPACING);

        match users_info.as_mut() {
            Ok(users) => {
                let has_multiple_users = users.len() > 1;
                let mut redraw = 0;
                set_scroll_style(ui);
                ScrollArea::vertical()
                    .stick_to_right(true)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.reset_style();

                        for user in users.iter_mut() {
                            if has_multiple_users {
                                ui.heading(user.path.to_string_lossy().to_string());
                            }
                            for shortcut in user.shortcuts.iter() {
                                if shortcut.is_boilr_shortcut()
                                    && ui.button(&shortcut.app_name).clicked()
                                    && disconnect_shortcut(&self.settings, shortcut.app_id).is_ok()
                                {
                                    redraw = shortcut.app_id;
                                }
                            }
                        }
                    });
                if redraw != 0 {
                    self.disconnect_state.connected_shortcuts = None;
                    self.settings.blacklisted_games.push(redraw);
                }
            }
            Err(msg) => {
                ui.label(&*msg);
            }
        }
    }
}

fn set_scroll_style(ui: &mut egui::Ui) {
    let scroll_style = ui.style_mut();
    scroll_style.visuals.extreme_bg_color = BACKGROUND_COLOR;
    scroll_style.visuals.widgets.inactive.bg_fill = EXTRA_BACKGROUND_COLOR;
    scroll_style.visuals.widgets.active.bg_fill = EXTRA_BACKGROUND_COLOR;
    scroll_style.visuals.widgets.hovered.bg_fill = EXTRA_BACKGROUND_COLOR;
}
