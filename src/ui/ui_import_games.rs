use eframe::egui;
use egui::ScrollArea;
use futures::executor::block_on;

use steam_shortcuts_util::shortcut::ShortcutOwned;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::platforms::ShortcutToImport;
use boilr_core::config::get_renames_file;
#[cfg(target_family = "unix")]
use boilr_core::steam::setup_proton_games;
use boilr_core::sync;

use boilr_core::sync::{download_images, SyncProgress};

use super::{all_ready, get_all_games};
use super::{
    ui_colors::{BACKGROUND_COLOR, EXTRA_BACKGROUND_COLOR},
    MyEguiApp,
};
use crate::backups::backup_shortcuts;

const SECTION_SPACING: f32 = 25.0;

pub enum FetchStatus<T> {
    NeedsFetched,
    Fetching,
    Fetched(T),
}

impl<T> FetchStatus<T> {
    pub fn is_some(&self) -> bool {
        match self {
            FetchStatus::NeedsFetched => false,
            FetchStatus::Fetching => false,
            FetchStatus::Fetched(_) => true,
        }
    }
}

impl MyEguiApp {
    pub(crate) fn render_import_games(&mut self, ui: &mut egui::Ui) {
        ui.heading("Import Games");

        let scroll_style = ui.style_mut();
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
                    FetchStatus::NeedsFetched => {ui.label("Need to find games");},
                    FetchStatus::Fetching => {
                        ui.horizontal(|ui|{
                            ui.spinner();
                            ui.label("Finding installed games");
                        });
                    },
                    FetchStatus::Fetched(shortcuts) => {
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
                                                        println!("Write rename file at {rename_file_path:?} with result: {res:?}");
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

    pub fn run_sync_blocking(&mut self) -> eyre::Result<()> {
        self.run_sync(true)
    }

    pub fn run_sync_async(&mut self) {
        let _ = self.run_sync(false);
    }
    fn run_sync(&mut self, wait: bool) -> eyre::Result<()> {
        let (sender, reciever) = watch::channel(SyncProgress::NotStarted);
        let settings = self.settings.clone();
        if settings.steam.stop_steam {
            boilr_core::steam::ensure_steam_stopped();
        }

        self.status_reciever = reciever;
        let renames = self.rename_map.clone();
        let all_ready = all_ready(&self.games_to_sync);
        let _ = sender.send(SyncProgress::Starting);
        if all_ready {
            let shortcuts_to_import = get_all_games(&self.games_to_sync);
            let handle: JoinHandle<eyre::Result<()>> = self.rt.spawn_blocking(move || {
                #[cfg(target_family = "unix")]
                setup_proton(shortcuts_to_import.iter());

                let import_games = to_shortcut_owned(shortcuts_to_import);

                let mut some_sender = Some(sender);
                backup_shortcuts(&settings.steam);
                let usersinfo =
                    sync::sync_shortcuts(&settings, &import_games, &mut some_sender, &renames)?;
                let task = download_images(&settings, &usersinfo, &mut some_sender);
                block_on(task);
                //Run a second time to fix up shortcuts after images are downloaded
                if let Err(e) = sync::fix_all_shortcut_icons(&settings) {
                    eprintln!("Could not fix shortcuts with error {e}");
                }

                if let Some(sender) = some_sender {
                    let _ = sender.send(SyncProgress::Done);
                }
                if settings.steam.start_steam {
                    boilr_core::steam::ensure_steam_started(&settings.steam);
                }
                Ok(())
            });
            if wait {
                self.rt.block_on(handle)??;
            }
        }
        Ok(())
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
                boilr_core::sync::symlinks::ensure_links_folder_created(name);
            }
            if shortcut_info.needs_proton {
                shortcuts_to_proton.push(format!("{}", shortcut_info.shortcut.app_id));
            }

            if shortcut_info.needs_symlinks {
                boilr_core::sync::symlinks::create_sym_links(&shortcut_info.shortcut);
            }
        }
        if let Err(err) = setup_proton_games(&shortcuts_to_proton) {
            eprintln!("failed to save proton settings: {err:?}");
        }
    }
}
