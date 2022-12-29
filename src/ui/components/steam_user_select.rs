use crate::steam::SteamUsersInfo;

pub fn render_user_select<'a>(
    steam_user: Option<&'a SteamUsersInfo>,
    steam_users: &'a [SteamUsersInfo],
    ui: &mut egui::Ui,
) -> Option<&'a SteamUsersInfo> {
    if steam_user.is_none() {
        if steam_users.len() > 0 {
            return Some(&steam_users[0]);
        }
    } else {
        let mut selected_user = steam_user.as_ref().unwrap().clone();
        let id_before = selected_user.user_id.clone();
        if steam_users.len() > 0 {
            let combo_box = egui::ComboBox::new("ImageUserSelect", "")
                .selected_text(format!("Steam user id: {}", &selected_user.user_id));
            combo_box.show_ui(ui, |ui| {
                for user in steam_users {
                    ui.selectable_value(&mut selected_user, &user, &user.user_id);
                }
            });
        }
        let id_now = selected_user.user_id.clone();
        if !id_before.eq(&id_now) {
            return Some(selected_user);
        }
    }
    None
}
