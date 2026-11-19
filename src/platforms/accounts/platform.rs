use boilr_stores::{self as stores, OwnedLibrary, StoreId};
use serde::{Deserialize, Serialize};

#[cfg(feature = "egui-ui")]
use super::block_on_fresh;
use super::{
    account_code_name,
    targets::{owned_shortcut, Launchers},
};
use crate::platforms::{load_settings, GamesPlatform, ShortcutToImport};

/// What the shortcut of an owned, not installed game opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallWith {
    /// The store's launcher when it is installed, then Heroic (Linux), then the store page.
    #[default]
    Auto,
    /// Heroic first (Epic, GOG and Amazon games on Linux).
    Heroic,
    /// Always the store page in the browser.
    Browser,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub install_with: InstallWith,
    /// Leave out betas, demos, playtests and test servers ([`is_prerelease`]).
    #[serde(default = "default_true")]
    pub skip_prereleases: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AccountSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            install_with: InstallWith::default(),
            skip_prereleases: true,
        }
    }
}

/// Whether a title names a beta, a demo, a playtest or a test server rather than the game:
/// "Barony (Beta)", "Shadwen demo", "Mortal Shell Tech Beta", "KillingFloor2Beta". Whole
/// words only, so "Demon's Tilt" or "Alpha Protocol" stay.
pub fn is_prerelease(title: &str) -> bool {
    const WORDS: [&str; 4] = ["beta", "demo", "playtest", "pts"];
    const PHRASES: [&str; 3] = ["test server", "tech test", "public test"];
    let lower = title.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    if words.iter().any(|w| WORDS.contains(w)) {
        return true;
    }
    let spaced = words.join(" ");
    if PHRASES.iter().any(|phrase| spaced.contains(phrase)) {
        return true;
    }
    // A store id used as a title, its words run together ("KillingFloor2Beta").
    title
        .split_whitespace()
        .any(|word| word.len() > 6 && (word.ends_with("Beta") || word.ends_with("Demo")))
}

#[derive(Clone)]
pub struct AccountPlatform {
    pub store: StoreId,
    pub settings: AccountSettings,
    // State of the egui settings panel, shared with its background tasks.
    #[cfg(feature = "egui-ui")]
    panel: std::sync::Arc<std::sync::Mutex<Panel>>,
    #[cfg(feature = "egui-ui")]
    pasted: String,
}

impl AccountPlatform {
    pub fn new<S: AsRef<str>>(store: StoreId, settings: S) -> Self {
        AccountPlatform {
            store,
            settings: load_settings(settings),
            #[cfg(feature = "egui-ui")]
            panel: Default::default(),
            #[cfg(feature = "egui-ui")]
            pasted: String::new(),
        }
    }

    /// The library as it was last fetched. Listing games never waits on a store: front
    /// ends fetch libraries in the background (see [`super::refresh_stale_libraries`]),
    /// and a failed fetch keeps the last list, so a network problem never removes the
    /// account's games from Steam.
    pub fn library(&self) -> eyre::Result<OwnedLibrary> {
        let store = self.store;
        if stores::load_session(store)?.is_none() {
            return Err(eyre::format_err!(
                "Not signed in to {}. Sign in from this source's settings.",
                store.display_name()
            ));
        }
        stores::cached_library(store).ok_or_else(|| {
            eyre::format_err!(
                "The games of the {} account have not been listed yet. Refresh its library \
                 from this source's settings.",
                store.display_name()
            )
        })
    }
}

impl GamesPlatform for AccountPlatform {
    fn name(&self) -> &str {
        match self.store {
            StoreId::Epic => "Epic Games account",
            StoreId::Gog => "GOG account",
            StoreId::Amazon => "Amazon Games account",
            StoreId::Ea => "EA account",
            StoreId::BattleNet => "Battle.net account",
            StoreId::Humble => "Humble Bundle account",
            StoreId::Itch => "itch.io account",
        }
    }

    fn code_name(&self) -> &str {
        account_code_name(self.store)
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        let library = self.library()?;
        let launchers = Launchers::detect();
        Ok(library
            .games
            .iter()
            .filter(|game| !(self.settings.skip_prereleases && is_prerelease(&game.title)))
            .map(|game| ShortcutToImport {
                shortcut: owned_shortcut(game, self.settings.install_with, &launchers),
                needs_proton: false,
                needs_symlinks: false,
                installed: false,
            })
            .collect())
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    #[cfg(feature = "egui-ui")]
    fn render_ui(&mut self, ui: &mut egui::Ui) {
        self.render_account_ui(ui);
    }
}

/// What the egui settings panel shows. Sign-in and refresh run on a background thread and
/// report here, so the window never waits on the network.
#[cfg(feature = "egui-ui")]
#[derive(Default)]
struct Panel {
    /// Read from disk once, and again after every change.
    status: Option<stores::AccountStatus>,
    busy: bool,
    message: Option<String>,
    /// A sign-in finished: the pasted value is no longer needed.
    signed_in: bool,
    /// A sign-out finished: the account is no longer a source.
    signed_out: bool,
}

/// What a background task did to the sign-in.
#[cfg(feature = "egui-ui")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Unchanged,
    SignedIn,
    SignedOut,
}

/// What signing in found, and how to add it: signing in alone adds nothing to Steam.
#[cfg(feature = "egui-ui")]
fn games_to_add(store: StoreId) -> String {
    match stores::cached_library(store).map(|library| library.games.len()) {
        Some(count) => {
            format!("This account owns {count} games. Tick the box above to add them to Steam.")
        }
        None => "Its games could not be listed yet; use Refresh library.".to_string(),
    }
}

#[cfg(feature = "egui-ui")]
fn lock_panel(panel: &std::sync::Mutex<Panel>) -> std::sync::MutexGuard<'_, Panel> {
    panel
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(feature = "egui-ui")]
impl AccountPlatform {
    /// Runs `work` on a background thread; its message shows in the panel when it is done.
    /// `work` returns the message and what happened to the sign-in.
    fn run_in_background<F>(&self, ctx: &egui::Context, work: F)
    where
        F: FnOnce() -> (String, Outcome) + Send + 'static,
    {
        {
            let mut panel = lock_panel(&self.panel);
            if panel.busy {
                return;
            }
            panel.busy = true;
            panel.message = None;
        }
        let panel = self.panel.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let (message, outcome) = work();
            {
                let mut panel = lock_panel(&panel);
                panel.busy = false;
                panel.message = Some(message);
                panel.signed_in |= outcome == Outcome::SignedIn;
                panel.signed_out |= outcome == Outcome::SignedOut;
                panel.status = None;
            }
            ctx.request_repaint();
        });
    }

    fn render_account_ui(&mut self, ui: &mut egui::Ui) {
        use boilr_stores::{LoginCapture, ManualLogin};

        let store = self.store;
        let (status, busy, message) = {
            let mut panel = lock_panel(&self.panel);
            if panel.signed_in {
                panel.signed_in = false;
                self.pasted.clear();
            }
            if panel.signed_out {
                // A source without a sign-in would only report an error on every sync.
                panel.signed_out = false;
                self.settings.enabled = false;
            }
            let status = panel
                .status
                .get_or_insert_with(|| stores::account_status(store))
                .clone();
            (status, panel.busy, panel.message.clone())
        };

        ui.heading(self.name().to_string());
        ui.checkbox(
            &mut self.settings.enabled,
            "Add every game this account owns, installed or not",
        );
        if status.signed_in {
            let who = status
                .account_name
                .clone()
                .unwrap_or_else(|| "your account".to_string());
            let games = match (status.game_count, status.fetched_at) {
                (Some(count), Some(_)) => format!("{count} games in the library"),
                _ => "library not fetched yet".to_string(),
            };
            ui.label(format!("Signed in as {who}, {games}."));
            if status.needs_sign_in {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    "The store no longer accepts this sign-in, so the games listed are from \
                     before. Sign out and sign in again.",
                );
            } else if let Some(error) = &status.last_error {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    format!(
                        "The last refresh failed, so the games listed are from before: {error}"
                    ),
                );
            }
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!busy, |ui| {
                    if ui.button("Refresh library").clicked() {
                        self.run_in_background(ui.ctx(), move || {
                            let message = match block_on_fresh(stores::refresh_library(store)) {
                                Ok(Ok(library)) => format!("Found {} games.", library.games.len()),
                                Ok(Err(err)) | Err(err) => format!("Could not refresh: {err}"),
                            };
                            (message, Outcome::Unchanged)
                        });
                    }
                    if ui.button("Sign out").clicked() {
                        self.run_in_background(ui.ctx(), move || {
                            match block_on_fresh(stores::sign_out(store)) {
                                Ok(Ok(None)) => ("Signed out.".to_string(), Outcome::SignedOut),
                                Ok(Ok(Some(warning))) => (warning, Outcome::SignedOut),
                                Ok(Err(err)) | Err(err) => {
                                    (format!("Could not sign out: {err}"), Outcome::Unchanged)
                                }
                            }
                        });
                    }
                });
            });
        } else {
            match &status.manual {
                ManualLogin::Paste {
                    instructions,
                    placeholder,
                    ..
                } => {
                    // The link comes from the login in progress, not from the status: a
                    // store's link can be tied to the login that finishes it.
                    if ui.link("Open the sign-in page in your browser").clicked() {
                        if let ManualLogin::Paste { url, .. } = stores::pending_login(store).manual
                        {
                            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                        }
                    }
                    ui.label(instructions);
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.pasted)
                                .hint_text(placeholder.as_str()),
                        );
                        ui.add_enabled_ui(!busy, |ui| {
                            if ui.button("Sign in").clicked() {
                                // The same pending login as the link above: Amazon's sign-in
                                // link carries its PKCE challenge.
                                let pending = stores::pending_login(store);
                                let capture = LoginCapture {
                                    pasted: Some(self.pasted.trim().to_string()),
                                    ..Default::default()
                                };
                                self.run_in_background(ui.ctx(), move || {
                                    let signed_in = block_on_fresh(async {
                                        let name =
                                            stores::complete_login(&pending, &capture).await?;
                                        if let Err(err) = stores::refresh_library(store).await {
                                            eprintln!(
                                                "Could not fetch the {} library: {err:?}",
                                                store.key()
                                            );
                                        }
                                        Ok::<_, eyre::Report>(name)
                                    })
                                    .and_then(|r| r);
                                    match signed_in {
                                        Ok(name) => (
                                            format!(
                                                "Signed in{}. {}",
                                                name.map(|n| format!(" as {n}"))
                                                    .unwrap_or_default(),
                                                games_to_add(store)
                                            ),
                                            Outcome::SignedIn,
                                        ),
                                        Err(err) => (
                                            format!("Could not sign in: {err}"),
                                            Outcome::Unchanged,
                                        ),
                                    }
                                });
                            }
                        });
                    });
                }
                ManualLogin::Unsupported => {
                    ui.label(format!(
                        "{} needs its own sign-in window. Sign in from the BoilR 2 app; this app then uses that sign-in too.",
                        store.display_name()
                    ));
                }
            }
        }
        ui.checkbox(
            &mut self.settings.skip_prereleases,
            "Leave out betas, demos, playtests and test servers",
        );
        egui::ComboBox::from_label("Games that are not installed open")
            .selected_text(match self.settings.install_with {
                InstallWith::Auto => "the store's launcher, or its web page",
                InstallWith::Heroic => "Heroic first",
                InstallWith::Browser => "the store's web page",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.settings.install_with,
                    InstallWith::Auto,
                    "the store's launcher, or its web page",
                );
                if cfg!(not(windows)) {
                    ui.selectable_value(
                        &mut self.settings.install_with,
                        InstallWith::Heroic,
                        "Heroic first",
                    );
                }
                ui.selectable_value(
                    &mut self.settings.install_with,
                    InstallWith::Browser,
                    "the store's web page",
                );
            });
        if busy {
            ui.spinner();
        }
        if let Some(message) = &message {
            ui.label(message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_to_disabled_and_survive_unknown_fields() {
        let platform = AccountPlatform::new(StoreId::Gog, "");
        assert!(!platform.enabled());
        let platform = AccountPlatform::new(StoreId::Gog, "enabled = true\nfuture = 1\n");
        assert!(platform.enabled());
        assert_eq!(platform.settings.install_with, InstallWith::Auto);
        let platform =
            AccountPlatform::new(StoreId::Gog, "enabled = true\ninstall_with = \"browser\"\n");
        assert_eq!(platform.settings.install_with, InstallWith::Browser);
        assert!(platform.settings.skip_prereleases, "on unless turned off");
        let platform = AccountPlatform::new(StoreId::Gog, "skip_prereleases = false\n");
        assert!(!platform.settings.skip_prereleases);
    }

    #[test]
    fn betas_demos_and_test_servers_are_told_apart_from_games() {
        for title in [
            "Barony (Beta)",
            "Shadwen demo",
            "Mortal Shell Tech Beta",
            "KillingFloor2Beta",
            "Some Game Playtest",
            "Game - Public Test Server",
            "Game PTS",
        ] {
            assert!(is_prerelease(title), "{title}");
        }
        for title in [
            "DEMON'S TILT",
            "Hell is other demons",
            "Alpha Protocol",
            "Betrayal at Krondor",
            "Demolish & Build",
            "Frostpunk",
        ] {
            assert!(!is_prerelease(title), "{title}");
        }
    }
}
