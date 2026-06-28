use std::cmp::Ordering;

use eframe::egui;
use egui::ScrollArea;
use futures::executor::block_on;

use steam_shortcuts_util::shortcut::ShortcutOwned;
use tokio::sync::watch;
use tokio::task::JoinHandle;

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
        ui.heading(self.language.t("import_games"));

        let scroll_style = ui.style_mut();
        scroll_style.visuals.extreme_bg_color = BACKGROUND_COLOR;
        scroll_style.visuals.widgets.inactive.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.widgets.active.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.selection.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.widgets.hovered.bg_fill = EXTRA_BACKGROUND_COLOR;

        ScrollArea::vertical()
            .stick_to_right(true)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.reset_style();
                ui.label(self.language.t("select_games"));
                ui.add_space(8.0);

                for (name, status) in &self.games_to_sync {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        match &*status.borrow() {
                            FetchStatus::NeedsFetched => {
                                ui.heading(name);
                                ui.label(self.language.t("need_to_find_games"));
                            }
                            FetchStatus::Fetching => {
                                ui.heading(name);
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(self.language.t("finding_installed_games"));
                                });
                            }
                            FetchStatus::Fetched(shortcuts) => match shortcuts {
                                Ok(shortcuts) => {
                                    let mut shortcuts_to_render = shortcuts.iter().cloned().collect::<Vec<_>>();
                                    sort_shortcuts_for_import(&mut shortcuts_to_render, &self.settings.blacklisted_games);

                                    let app_ids: Vec<u32> = shortcuts_to_render
                                        .iter()
                                        .map(|shortcut_to_import| shortcut_to_import.shortcut.app_id)
                                        .collect();
                                    let selected_count = selection_count_for_app_ids(app_ids.iter().copied(), &self.settings.blacklisted_games);
                                    let total_count = app_ids.len();

                                    ui.horizontal(|ui| {
                                        ui.heading(name);
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.small_button(self.language.t("select_all")).clicked() {
                                                apply_selection_for_app_ids(app_ids.iter().copied(), &mut self.settings.blacklisted_games, true);
                                            }
                                            if ui.small_button(self.language.t("clear_selection")).clicked() {
                                                apply_selection_for_app_ids(app_ids.iter().copied(), &mut self.settings.blacklisted_games, false);
                                            }
                                        });
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label(format!("{} {selected_count}/{total_count} {}", self.language.t("selected"), self.language.t("games")));
                                    });
                                    ui.add_space(6.0);

                                    if shortcuts_to_render.is_empty() {
                                        ui.label(self.language.t("did_not_find_any_games"));
                                    }

                                    let (selected_shortcuts, unselected_shortcuts): (Vec<_>, Vec<_>) = shortcuts_to_render
                                        .iter()
                                        .partition(|s| !self.settings.blacklisted_games.contains(&s.shortcut.app_id));

                                    // Renderiza itens selecionados normalmente
                                    for shortcut_to_import in &selected_shortcuts {
                                        render_shortcut_row(ui, shortcut_to_import, &mut self.settings.blacklisted_games, &mut self.rename_map, &mut self.current_edit, &self.language);
                                    }

                                    // Itens desmarcados colapsados
                                    if !unselected_shortcuts.is_empty() {
                                        ui.add_space(4.0);
                                        let label = format!(
                                            "{} ({})",
                                            self.language.t("not_selected"),
                                            unselected_shortcuts.len()
                                        );
                                        egui::CollapsingHeader::new(label)
                                            .default_open(false)
                                            .show(ui, |ui| {
                                                for shortcut_to_import in &unselected_shortcuts {
                                                    render_shortcut_row(ui, shortcut_to_import, &mut self.settings.blacklisted_games, &mut self.rename_map, &mut self.current_edit, &self.language);
                                                }
                                            });
                                    }
                                }
                                // FIX (UX): mostrar mensagem de erro na UI em vez de só eprintln
                                Err(err) => {
                                    ui.heading(name);
                                    ui.colored_label(
                                        egui::Color32::from_rgb(200, 80, 80),
                                        format!("⚠ {}: {err}", self.language.t("failed_to_find_games")),
                                    );
                                }
                            },
                        }
                    });
                    ui.add_space(8.0);
                }
                ui.add_space(SECTION_SPACING);

                ui.label(self.language.t("check_settings"));
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
            crate::steam::ensure_steam_stopped();
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
                if let Err(e) = sync::fix_all_shortcut_icons(&settings) {
                    eprintln!("Could not fix shortcuts with error {e}");
                }

                if let Some(sender) = some_sender {
                    let _ = sender.send(SyncProgress::Done);
                }
                if settings.steam.start_steam {
                    crate::steam::ensure_steam_started(&settings.steam);
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

fn render_shortcut_row(
    ui: &mut egui::Ui,
    shortcut_to_import: &ShortcutToImport,
    blacklisted_games: &mut Vec<u32>,
    rename_map: &mut std::collections::HashMap<u32, String>,
    current_edit: &mut Option<u32>,
    language: &crate::ui::locale::AppLanguage,
) {
    let shortcut = &shortcut_to_import.shortcut;
    let mut import_game = !blacklisted_games.contains(&shortcut.app_id);
    let app_id = shortcut.app_id;
    let default_name = shortcut.app_name.to_owned();

    let row_response = ui.horizontal(|ui| {
        if *current_edit == Some(app_id) {
            if let Some(new_name) = rename_map.get_mut(&app_id) {
                ui.text_edit_singleline(new_name).request_focus();
                if ui.button(language.t("rename")).clicked() {
                    if new_name.is_empty() {
                        *new_name = shortcut.app_name.to_string();
                    }
                    *current_edit = None;
                    let rename_file_path = get_renames_file();
                    let contents = serde_json::to_string(&rename_map);
                    if let Ok(contents) = contents {
                        let res = std::fs::write(&rename_file_path, contents);
                        println!("Write rename file at {rename_file_path:?} with result: {res:?}");
                    }
                }
            }
        } else {
            let name = rename_map.get(&app_id).unwrap_or(&shortcut.app_name);
            let response = ui.checkbox(&mut import_game, name);
            if response.clicked() {
                if import_game {
                    blacklisted_games.retain(|id| *id != app_id);
                } else if !blacklisted_games.contains(&app_id) {
                    blacklisted_games.push(app_id);
                }
            }
        }
    }).response;

    row_response.context_menu(|ui| {
        if ui.button(language.t("rename")).clicked() {
            rename_map.entry(app_id).or_insert_with(|| default_name.clone());
            *current_edit = Some(app_id);
            ui.close_menu();
        }
    });
}

fn sort_shortcuts_for_import(shortcuts: &mut Vec<ShortcutToImport>, blacklisted_ids: &[u32]) {
    shortcuts.sort_by(|left, right| {
        let left_selected = !blacklisted_ids.contains(&left.shortcut.app_id);
        let right_selected = !blacklisted_ids.contains(&right.shortcut.app_id);

        match (left_selected, right_selected) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => left.shortcut.app_name.cmp(&right.shortcut.app_name),
        }
    });
}

fn selection_count_for_app_ids<I>(app_ids: I, blacklisted_ids: &[u32]) -> usize
where
    I: IntoIterator<Item = u32>,
{
    app_ids
        .into_iter()
        .filter(|app_id| !blacklisted_ids.contains(app_id))
        .count()
}

fn apply_selection_for_app_ids<I>(app_ids: I, blacklisted_ids: &mut Vec<u32>, selected: bool)
where
    I: IntoIterator<Item = u32>,
{
    for app_id in app_ids {
        if selected {
            blacklisted_ids.retain(|id| *id != app_id);
        } else if !blacklisted_ids.contains(&app_id) {
            blacklisted_ids.push(app_id);
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

// FIX: módulo de testes unificado (havia dois `mod tests` separados → erro de compilação)
#[cfg(test)]
mod tests {
    use super::{apply_selection_for_app_ids, selection_count_for_app_ids, sort_shortcuts_for_import};
    use crate::platforms::ShortcutToImport;
    use steam_shortcuts_util::shortcut::ShortcutOwned;

    fn shortcut(app_id: u32, app_name: &str) -> ShortcutToImport {
        ShortcutToImport {
            shortcut: ShortcutOwned {
                app_id,
                app_name: app_name.to_string(),
                ..Default::default()
            },
            needs_proton: false,
            needs_symlinks: false,
        }
    }

    #[test]
    fn selected_shortcuts_are_sorted_before_unselected_ones() {
        let mut shortcuts = vec![shortcut(2, "Zulu"), shortcut(1, "Alpha")];
        // app_id=1 está na blacklist → unselected; app_id=2 está selected
        sort_shortcuts_for_import(&mut shortcuts, &[1]);

        // FIX: assert corrigido — selected (app_id=2/Zulu) deve vir primeiro
        assert_eq!(shortcuts[0].shortcut.app_id, 2);
        assert_eq!(shortcuts[1].shortcut.app_id, 1);
    }

    #[test]
    fn selection_count_only_counts_selected_games() {
        let app_ids = [1, 2, 3];
        let blacklisted_ids = vec![2];

        assert_eq!(selection_count_for_app_ids(app_ids.iter().copied(), &blacklisted_ids), 2);
    }

    #[test]
    fn apply_selection_for_app_ids_updates_blacklist() {
        let app_ids = [1, 2, 3];
        let mut blacklisted_ids = vec![2];

        apply_selection_for_app_ids(app_ids.iter().copied(), &mut blacklisted_ids, true);
        assert!(blacklisted_ids.is_empty());

        apply_selection_for_app_ids(app_ids.iter().copied(), &mut blacklisted_ids, false);
        assert_eq!(blacklisted_ids, vec![1, 2, 3]);
    }
}

#[cfg(target_family = "unix")]
fn setup_proton<'a, I>(shortcut_infos: I)
where
    I: IntoIterator<Item = &'a (String, Vec<ShortcutToImport>)>,
{
    for (name, shortcuts) in shortcut_infos {
        // FIX: reset por plataforma — o vec acumulava IDs de todas as plataformas anteriores
        let mut shortcuts_to_proton = vec![];

        for shortcut_info in shortcuts {
            if shortcut_info.needs_proton {
                crate::sync::symlinks::ensure_links_folder_created(name);
                shortcuts_to_proton.push(format!("{}", shortcut_info.shortcut.app_id));
            }

            if shortcut_info.needs_symlinks {
                crate::sync::symlinks::create_sym_links(&shortcut_info.shortcut);
            }
        }

        if let Err(err) = setup_proton_games(&shortcuts_to_proton) {
            eprintln!("failed to save proton settings for {name}: {err:?}");
        }
    }
}