use eframe::egui;
use egui::ScrollArea;
use futures::executor::block_on;

use steam_shortcuts_util::shortcut::ShortcutOwned;
use tokio::sync::watch;

use crate::config::get_renames_file;
use crate::platforms::ShortcutToImport;
#[cfg(target_family = "unix")]
use crate::steam::setup_proton_games;
use crate::sync;

use crate::sync::{download_images, SyncProgress};

use super::{all_ready, backup_shortcuts, get_all_games};
use super::{
    ui_colors::{BACKGROUND_COLOR, EXTRA_BACKGROUND_COLOR},
    MyEguiApp,
};

const SECTION_SPACING: f32 = 25.0;

pub enum FetcStatus<T> {
    NeedsFetched,
    Fetching,
    Fetched(T),
}

impl<T> FetcStatus<T> {
    pub fn is_some(&self) -> bool {
        match self {
            FetcStatus::NeedsFetched => false,
            FetcStatus::Fetching => false,
            FetcStatus::Fetched(_) => true,
        }
    }
}

impl MyEguiApp {
    pub(crate) fn render_import_games(&mut self, ui: &mut egui::Ui) {
        ui.heading("Import Games");

        let mut scroll_style = ui.style_mut();
        scroll_style.visuals.extreme_bg_color = BACKGROUND_COLOR;
        scroll_style.visuals.widgets.inactive.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.widgets.active.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.selection.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.widgets.hovered.bg_fill = EXTRA_BACKGROUND_COLOR;

        ScrollArea::vertical()
        .stick_to_right(true)
        .auto_shrink([false,true])
        .show(ui,|ui| {
            ui.reset_style();
            ui.label("Select the games you want to import into steam");
            for (name,status) in &self.games_to_sync{
                ui.heading(name);
                match &*status.borrow(){
                    FetcStatus::NeedsFetched => {ui.label("Need to find games");},
                    FetcStatus::Fetching => {
                        ui.horizontal(|ui|{
                            ui.spinner();
                            ui.label("Finding installed games");
                        });
                    },
                    FetcStatus::Fetched(shortcuts) => {
                        match shortcuts{
                            Ok(shortcuts) => {
                                if shortcuts.is_empty(){
                                    ui.label("Did not find any games");
                                }
                                for shortcut_to_import in shortcuts {
                                    let shortcut = &shortcut_to_import.shortcut;
                                    let mut import_game = !self.settings.blacklisted_games.contains(&shortcut.app_id);
                                    ui.horizontal(|ui|{
                                        if self.current_edit == Option::Some(shortcut.app_id){
                                            if let Some(new_name) = self.rename_map.get_mut(&shortcut.app_id){
                                                ui.text_edit_singleline(new_name).request_focus();
                                                if ui.button("Rename").clicked() {
                                                    if new_name.is_empty(){
                                                        *new_name = shortcut.app_name.to_string();
                                                    }
                                                    self.current_edit = Option::None;
                                                    let rename_file_path = get_renames_file();
                                                    let contents = serde_json::to_string(&self.rename_map);
                                                    if let Ok(contents) = contents{
                                                        let res = std::fs::write(&rename_file_path, contents);
                                                        println!("Write rename file at {:?} with result: {:?}",rename_file_path, res);
                                                    }
                                                }
                                            }
                                        }  else {
                                            let name = self.rename_map.get(&shortcut.app_id).unwrap_or(&shortcut.app_name);
                                            let checkbox = egui::Checkbox::new(&mut import_game,name);
                                            let response = ui.add(checkbox);
                                            if response.double_clicked(){
                                                self.rename_map.entry(shortcut.app_id).or_insert_with(|| shortcut.app_name.to_owned());
                                                self.current_edit = Option::Some(shortcut.app_id);
                                            }
                                            if response.clicked(){
                                                if !self.settings.blacklisted_games.contains(&shortcut.app_id){
                                                    self.settings.blacklisted_games.push(shortcut.app_id);
                                                } else {
                                                    self.settings.blacklisted_games.retain(|id| *id != shortcut.app_id);
                                                }
                                            }
                                        }
                                    });
                                }
                            },
                            Err(err) => {
                                ui.label("Failed finding games").on_hover_text(format!("Error message: {err}"));
                            },
                        };
                    },
                }

            };
            ui.add_space(SECTION_SPACING);

            ui.label("Check the settings if BoilR didn't find the game you where looking for");
        });
    }

    pub fn run_sync(&mut self, wait: bool) {
        let (sender, reciever) = watch::channel(SyncProgress::NotStarted);
        let settings = self.settings.clone();
        if settings.steam.stop_steam {
            crate::steam::ensure_steam_stopped();
        }

        //TODO This might break cli sync, test it

        self.status_reciever = reciever;
        let renames = self.rename_map.clone();
        let all_ready = all_ready(&self.games_to_sync);
        let _ = sender.send(SyncProgress::Starting);
        if all_ready {
            let shortcuts_to_import = get_all_games(&self.games_to_sync);
            let handle = self.rt.spawn_blocking(move || {
                #[cfg(target_family = "unix")]
                setup_proton(shortcuts_to_import.iter());

                let import_games = to_shortcut_owned(shortcuts_to_import);

                let mut some_sender = Some(sender);
                backup_shortcuts(&settings.steam);
                let usersinfo =
                    sync::sync_shortcuts(&settings, &import_games, &mut some_sender, &renames)
                        .unwrap();
                let task = download_images(&settings, &usersinfo, &mut some_sender);
                block_on(task);
                //Run a second time to fix up shortcuts after images are downloaded
                sync::sync_shortcuts(&settings, &import_games, &mut some_sender, &renames).unwrap();

                if let Some(sender) = some_sender {
                    let _ = sender.send(SyncProgress::Done);
                }
                if settings.steam.start_steam {
                    crate::steam::ensure_steam_started(&settings.steam);
                }
            });
            if wait {
                self.rt.block_on(handle).unwrap();
            }
        }
    }
}

fn to_shortcut_owned(
    shortcuts_to_import: Vec<(String, Vec<ShortcutToImport>)>,
) -> Vec<(String, Vec<ShortcutOwned>)> {
    let mut import_games = vec![];
    for (name, infos) in shortcuts_to_import {
        let mut shortcuts = vec![];
        for info in infos {
            shortcuts.push(info.shortcut);
        }
        import_games.push((name, shortcuts));
    }
    import_games
}

#[cfg(target_family = "unix")]
fn setup_proton<'a, I>(shortcut_infos: I)
where
    I: IntoIterator<Item = &'a (String, Vec<ShortcutToImport>)>,
{
    let mut shortcuts_to_proton = vec![];

    for (name, shortcuts) in shortcut_infos {
        for shortcut_info in shortcuts {
            if shortcut_info.needs_proton {
                crate::sync::symlinks::ensure_links_folder_created(name);
            }
            if shortcut_info.needs_proton {
                shortcuts_to_proton.push(format!("{}", shortcut_info.shortcut.app_id));
            }

            if shortcut_info.needs_symlinks {
                crate::sync::symlinks::create_sym_links(&shortcut_info.shortcut);
            }
        }
        setup_proton_games(&shortcuts_to_proton);
    }
}
