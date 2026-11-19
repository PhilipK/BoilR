//! Store accounts: sign in to a game store, keep the session, and list every game the
//! account owns, installed or not. Modelled on Playnite's library plugins
//! (github.com/JosefNemec/PlayniteExtensions, `source/Libraries`), which talk to the same
//! store services.
//!
//! This crate knows nothing about Steam or about UIs. A front end runs a login (a webview,
//! or the user pasting a value), hands the result to [`complete_login`], and later calls
//! [`refresh_library`]. The `boilr` crate turns the cached library into Steam shortcuts.
#![deny(clippy::unwrap_in_result)]
#![deny(clippy::get_unwrap)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]

pub mod amazon;
pub mod battlenet;
pub mod ea;
pub mod epic;
pub mod gog;
pub mod http;
pub mod humble;
pub mod itch;
mod storage;

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock, PoisonError},
    time::Duration,
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub use storage::{load_library, load_session};
pub(crate) use storage::{save_library, save_session};

/// A store BoilR can sign in to. The `key` is used in file names and settings; never change it.
/// It is also how a store is written in saved libraries and sent to front ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoreId {
    Epic,
    Gog,
    Amazon,
    Ea,
    // `battle_net` is how early builds of this branch wrote it.
    #[serde(rename = "battlenet", alias = "battle_net")]
    BattleNet,
    Humble,
    Itch,
}

impl StoreId {
    pub const ALL: [StoreId; 7] = [
        StoreId::Epic,
        StoreId::Gog,
        StoreId::Amazon,
        StoreId::Ea,
        StoreId::BattleNet,
        StoreId::Humble,
        StoreId::Itch,
    ];

    /// Stable identifier, used for file names, settings sections and shortcut identities.
    pub fn key(&self) -> &'static str {
        match self {
            StoreId::Epic => "epic",
            StoreId::Gog => "gog",
            StoreId::Amazon => "amazon",
            StoreId::Ea => "ea",
            StoreId::BattleNet => "battlenet",
            StoreId::Humble => "humble",
            StoreId::Itch => "itch",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            StoreId::Epic => "Epic Games Store",
            StoreId::Gog => "GOG",
            StoreId::Amazon => "Amazon Games",
            StoreId::Ea => "EA app",
            StoreId::BattleNet => "Battle.net",
            StoreId::Humble => "Humble Bundle",
            StoreId::Itch => "itch.io",
        }
    }

    pub fn from_key(key: &str) -> Option<StoreId> {
        StoreId::ALL.into_iter().find(|s| s.key() == key)
    }
}

/// One game the account owns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedGame {
    pub store: StoreId,
    /// The store's stable id for the game, the same id its launcher and BoilR's installed-game
    /// platforms use, so an owned game and its installed copy can be matched:
    /// Epic `appName`, GOG product id, Amazon product id, EA offer id, Battle.net product
    /// code, Humble `machine_name`, itch game id.
    pub id: String,
    pub title: String,
    /// Store-specific values the install target needs (for example Epic's `namespace` and
    /// `catalog_item_id`, or an itch game's `url`). Keys are documented per store module.
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
}

impl OwnedGame {
    pub fn new(store: StoreId, id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            store,
            id: id.into(),
            title: title.into(),
            extra: BTreeMap::new(),
        }
    }

    pub fn with_extra(mut self, key: &str, value: impl Into<String>) -> Self {
        self.extra.insert(key.to_string(), value.into());
        self
    }

    pub fn extra(&self, key: &str) -> Option<&str> {
        self.extra.get(key).map(String::as_str)
    }
}

/// The owned games of one account, as last fetched.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OwnedLibrary {
    pub account_name: Option<String>,
    /// Unix time in seconds.
    pub fetched_at: u64,
    pub games: Vec<OwnedGame>,
    /// Some games could not be looked up when this list was made (a catalog service was
    /// down). The list is good to use, and is fetched again after
    /// [`RETRY_AFTER_FAILURE_SECS`] instead of waiting until it is old.
    #[serde(default)]
    pub partial: bool,
}

/// A signed-in account. `data` is owned by the store module (tokens, cookies, ids) and is
/// stored as-is in the user's config folder, readable only by the user.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub account_name: Option<String>,
    pub data: serde_json::Value,
}

// Sessions, cookies and logins hold secrets: their `Debug` output never shows them, so a
// stray `{:?}` or `dbg!` cannot put a token in a log.
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("account_name", &self.account_name)
            .field("data", &"<hidden>")
            .finish()
    }
}

/// A cookie captured from a login webview. `domain` is as the webview reports it, which
/// does not tell a host-only cookie from a domain cookie (see [`http::cookie_jar`]).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
}

impl std::fmt::Debug for Cookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cookie")
            .field("name", &self.name)
            .field("value", &"<hidden>")
            .field("domain", &self.domain)
            .finish()
    }
}

/// How a login can be completed without a webview: the user signs in with their own browser
/// and pastes one value back.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ManualLogin {
    /// Not possible: this store needs the built-in login window.
    Unsupported,
    /// Open `url`, sign in, and paste what `instructions` describes.
    Paste {
        url: String,
        instructions: String,
        placeholder: String,
    },
}

/// Everything a front end needs to run a login, plus state the store needs to finish it.
#[derive(Clone, Serialize, Deserialize)]
pub struct PendingLogin {
    pub store: StoreId,
    /// Where the login window starts.
    pub start_url: String,
    /// The login is done once the window navigates to a URL starting with one of these.
    /// The window should not load that URL; it hands it to [`complete_login`] instead.
    pub done_url_prefixes: Vec<String>,
    /// The window does load a done URL that starts with one of these, and hands the text
    /// the page shows to [`complete_login`] as [`LoginCapture::page_text`]. Epic's done page
    /// is JSON that holds the login code, and only the signed-in browser may fetch it.
    #[serde(default)]
    pub page_text_url_prefixes: Vec<String>,
    /// URLs whose cookies the window collects when the login is done. A window that can
    /// list all its cookies hands those over too.
    pub cookie_urls: Vec<String>,
    pub manual: ManualLogin,
    /// The user agent the login window should use, when the store's sign-in page needs a
    /// launcher's own (Epic's captcha only passes for the Epic Games Launcher's).
    #[serde(default)]
    pub user_agent: Option<String>,
    /// Store-private state carried from [`pending_login`] to [`complete_login`] (PKCE
    /// verifiers, device serials).
    #[serde(default)]
    pub state: serde_json::Value,
}

impl std::fmt::Debug for PendingLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The start URL can carry a PKCE challenge and the state its verifier.
        f.debug_struct("PendingLogin")
            .field("store", &self.store)
            .field("done_url_prefixes", &self.done_url_prefixes)
            .field("manual", &matches!(self.manual, ManualLogin::Paste { .. }))
            .finish_non_exhaustive()
    }
}

/// What a login produced.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct LoginCapture {
    /// The URL that matched `done_url_prefixes` (webview login).
    pub final_url: Option<String>,
    /// The text of the done page, for [`PendingLogin::page_text_url_prefixes`].
    #[serde(default)]
    pub page_text: Option<String>,
    /// Cookies for `cookie_urls` (webview login).
    #[serde(default)]
    pub cookies: Vec<Cookie>,
    /// The value the user pasted (manual login).
    pub pasted: Option<String>,
}

impl std::fmt::Debug for LoginCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The final URL holds a login code, the page text and the pasted value may too.
        f.debug_struct("LoginCapture")
            .field("final_url", &self.final_url.is_some())
            .field("page_text", &self.page_text.is_some())
            .field("cookies", &self.cookies.len())
            .field("pasted", &self.pasted.is_some())
            .finish()
    }
}

/// The sign-in in progress per store. A login started in one place (a window, a link the
/// user opens in their browser) is finished in another (a pasted value), so both must use
/// the same [`PendingLogin`]: Amazon's PKCE verifier and device serial live in its state.
static PENDING_LOGINS: Mutex<BTreeMap<StoreId, PendingLogin>> = Mutex::new(BTreeMap::new());

/// The sign-in in progress for `store`, started on first use. [`complete_login`] ends it
/// when it succeeds; until then every caller gets the same one.
pub fn pending_login(store: StoreId) -> PendingLogin {
    let mut pending = PENDING_LOGINS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    pending
        .entry(store)
        .or_insert_with(|| begin_login(store))
        .clone()
}

fn end_pending_login(store: StoreId) {
    PENDING_LOGINS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&store);
}

/// Starts a new login. Front ends use [`pending_login`], which keeps one per store.
fn begin_login(store: StoreId) -> PendingLogin {
    match store {
        StoreId::Epic => epic::begin_login(),
        StoreId::Gog => gog::begin_login(),
        StoreId::Amazon => amazon::begin_login(),
        StoreId::Ea => ea::begin_login(),
        StoreId::BattleNet => battlenet::begin_login(),
        StoreId::Humble => humble::begin_login(),
        StoreId::Itch => itch::begin_login(),
    }
}

/// Finishes a login and stores the session. Returns the account name when the store has one.
pub async fn complete_login(
    pending: &PendingLogin,
    capture: &LoginCapture,
) -> eyre::Result<Option<String>> {
    complete_login_inner(pending, capture)
        .await
        .map_err(redacted)
}

async fn complete_login_inner(
    pending: &PendingLogin,
    capture: &LoginCapture,
) -> eyre::Result<Option<String>> {
    let store = pending.store;
    let client = http::client()?;
    let session = match store {
        StoreId::Epic => epic::complete_login(&client, pending, capture).await,
        StoreId::Gog => gog::complete_login(&client, pending, capture).await,
        StoreId::Amazon => amazon::complete_login(&client, pending, capture).await,
        StoreId::Ea => ea::complete_login(&client, pending, capture).await,
        StoreId::BattleNet => battlenet::complete_login(&client, pending, capture).await,
        StoreId::Humble => humble::complete_login(&client, pending, capture).await,
        StoreId::Itch => itch::complete_login(&client, pending, capture).await,
    }?;
    save_new_login(store, &session)?;
    end_pending_login(store);
    record_failure(store, None);
    Ok(session.account_name)
}

/// Saves a new login. The cached library is kept, to be fetched again on next use, only
/// when it is known to belong to the same account: another account's games must not show
/// up under this one, and an account without games could never replace them.
fn save_new_login(store: StoreId, session: &Session) -> eyre::Result<()> {
    let _lock = storage::lock_account(store)?;
    save_session(store, session)?;
    if let Some(mut library) = cached_library(store) {
        let same_account = matches!(
            (&library.account_name, &session.account_name),
            (Some(before), Some(now)) if before == now
        );
        if same_account {
            library.fetched_at = 0;
            save_library(store, &library)?;
        } else {
            storage::delete_library(store)?;
        }
    }
    Ok(())
}

/// The session's store-private data as the store module's own type. Data that can't be read
/// (a file from another version, or damaged) needs a new sign-in.
pub(crate) fn session_data<T: DeserializeOwned>(
    store: StoreId,
    session: &Session,
) -> eyre::Result<T> {
    serde_json::from_value(session.data.clone()).map_err(|err| {
        http::login_rejected(format!(
            "The saved {} login can't be read ({err}). Sign in again.",
            store.display_name()
        ))
    })
}

/// A game's title as stores write it, without trademark signs and with single spaces, as
/// Playnite's `RemoveTrademarks` does. Every store module uses it, so one game has one
/// title whichever store lists it.
pub(crate) fn clean_title(title: &str) -> String {
    title
        .chars()
        .filter(|c| !matches!(c, '™' | '®' | '©'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The error as one message (with its causes) and without secrets, keeping its kind: a
/// rejected login, a transient failure, or an account that changed during the refresh.
fn redacted(err: eyre::Report) -> eyre::Report {
    let message = http::redact(&format!("{err:#}"));
    if is_account_changed(&err) {
        eyre::Report::new(AccountChanged(message))
    } else if http::is_login_rejected(&err) {
        http::login_rejected(message)
    } else if http::is_transient(&err) {
        http::transient(message)
    } else {
        eyre::Report::msg(message)
    }
}

/// The account was signed out of, or signed in again, while a refresh was running. The
/// refresh's results belong to the old login and were dropped.
#[derive(Debug)]
struct AccountChanged(String);

impl std::fmt::Display for AccountChanged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AccountChanged {}

fn is_account_changed(err: &eyre::Report) -> bool {
    err.chain().any(|cause| cause.is::<AccountChanged>())
}

fn account_changed(store: StoreId) -> eyre::Report {
    eyre::Report::new(AccountChanged(format!(
        "The {} account changed while its games were being fetched; that list was dropped.",
        store.display_name()
    )))
}

/// One lock per store in this process; [`storage::lock_refresh`] is the same across
/// processes. Two refreshes of one account at once would both use (and rotate) the same
/// refresh token.
fn refresh_lock(store: StoreId) -> Arc<tokio::sync::Mutex<()>> {
    static LOCKS: OnceLock<BTreeMap<StoreId, Arc<tokio::sync::Mutex<()>>>> = OnceLock::new();
    LOCKS
        .get_or_init(|| {
            StoreId::ALL
                .into_iter()
                .map(|store| (store, Arc::new(tokio::sync::Mutex::new(()))))
                .collect()
        })
        .get(&store)
        .cloned()
        .unwrap_or_default()
}

/// The last failed refresh of a store, kept on disk until a refresh works or the account is
/// signed out: listing games does not retry an unreachable store on every call, and front
/// ends can say what went wrong, also after a restart.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Failure {
    /// Unix seconds.
    at: u64,
    message: String,
    /// The store refused the saved login; nothing else asks the user to sign in again.
    needs_sign_in: bool,
}

/// How long [`refresh_if_stale`] waits after a failed refresh before it tries again.
pub const RETRY_AFTER_FAILURE_SECS: u64 = 15 * 60;

/// A refresh that takes longer than this is given up, whatever the store is doing.
pub const REFRESH_DEADLINE: Duration = Duration::from_secs(3 * 60);

/// Records the outcome of a refresh: `None` clears the failure.
fn record_failure(store: StoreId, error: Option<&eyre::Report>) {
    let failure = error.map(|err| Failure {
        at: now_unix(),
        message: err.to_string(),
        needs_sign_in: http::is_login_rejected(err),
    });
    if let Err(err) = storage::save_failure(store, failure.as_ref()) {
        eprintln!(
            "Could not record the state of the {} account: {err}",
            store.display_name()
        );
    }
}

fn last_failure(store: StoreId) -> Option<Failure> {
    storage::load_failure(store)
}

fn failed_recently(store: StoreId) -> bool {
    last_failure(store)
        .is_some_and(|failure| now_unix().saturating_sub(failure.at) < RETRY_AFTER_FAILURE_SECS)
}

/// How a refresh was asked for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Asked {
    /// By the user, or right after a sign-in: a much shorter list is taken as it is.
    Explicitly,
    /// Because the cache got old: a much shorter list is not trusted.
    Automatically,
}

/// Runs a refresh under the cross-process lock and the deadline, and records how it went.
/// A refresh dropped because the account changed is not a failure of the new login.
/// `still_needed` is asked once the lock is held: another process may have done the work.
async fn refresh_and_record(
    store: StoreId,
    asked: Asked,
    still_needed: impl Fn() -> bool,
) -> eyre::Result<Option<OwnedLibrary>> {
    let Some(_other_processes) = storage::lock_refresh(store, REFRESH_DEADLINE).await? else {
        return Err(http::transient(format!(
            "Another BoilR is still fetching the {} library",
            store.display_name()
        )));
    };
    if !still_needed() {
        return Ok(None);
    }
    let result =
        match tokio::time::timeout(REFRESH_DEADLINE, refresh_library_locked(store, asked)).await {
            Ok(result) => result.map_err(redacted),
            Err(_) => Err(http::transient(format!(
                "Fetching the {} library took longer than {} minutes and was stopped",
                store.display_name(),
                REFRESH_DEADLINE.as_secs() / 60
            ))),
        };
    match &result {
        Ok(_) => record_failure(store, None),
        Err(err) if is_account_changed(err) => {}
        Err(err) => record_failure(store, Some(err)),
    }
    result.map(Some)
}

/// The cached library, or `None` when there is none or it can't be read (a damaged file is
/// replaced by the next refresh instead of breaking the account).
pub fn cached_library(store: StoreId) -> Option<OwnedLibrary> {
    match load_library(store) {
        Ok(library) => library,
        Err(err) => {
            eprintln!(
                "The saved {} library is unreadable, it will be fetched again: {err}",
                store.display_name()
            );
            None
        }
    }
}

/// Fetches the owned games with the stored session, then stores the refreshed session and
/// the library. On error the previous library stays, so a network problem never empties it.
/// This is the refresh a user asks for: it accepts a list much shorter than the last one.
pub async fn refresh_library(store: StoreId) -> eyre::Result<OwnedLibrary> {
    let lock = refresh_lock(store);
    let _guard = lock.lock().await;
    refresh_and_record(store, Asked::Explicitly, || true)
        .await?
        .ok_or_else(|| eyre::format_err!("The {} library was not fetched", store.display_name()))
}

/// The library, fetched again first when it is older than `max_age_secs`. A failed fetch
/// falls back to the cache and is not retried for [`RETRY_AFTER_FAILURE_SECS`]. Concurrent
/// callers, in this process or another, wait for one fetch instead of each making their own.
///
/// This talks to the store, for up to [`REFRESH_DEADLINE`]. Listing games must not wait on
/// that: it reads [`cached_library`], and a front end calls this in the background.
pub async fn refresh_if_stale(store: StoreId, max_age_secs: u64) -> eyre::Result<OwnedLibrary> {
    let none_yet = || eyre::format_err!("No library fetched yet");
    let cached = cached_library(store);
    if !is_stale(cached.as_ref(), max_age_secs) || (cached.is_some() && failed_recently(store)) {
        return cached.ok_or_else(none_yet);
    }
    let lock = refresh_lock(store);
    let _guard = lock.lock().await;
    if failed_recently(store) {
        return cached_library(store).ok_or_else(|| {
            eyre::format_err!(
                "Could not fetch the {} library a moment ago; it is tried again later",
                store.display_name()
            )
        });
    }
    // Another caller, or another BoilR, may have fetched it while this one waited.
    let still_stale = || is_stale(cached_library(store).as_ref(), max_age_secs);
    match refresh_and_record(store, Asked::Automatically, still_stale).await {
        Ok(Some(library)) => Ok(library),
        Ok(None) => cached_library(store).ok_or_else(none_yet),
        Err(err) => {
            // While the account is still signed in, a list from before beats no list: a
            // failed refresh must not take the account's games out of Steam.
            let signed_in = load_session(store).ok().flatten().is_some();
            match cached_library(store).filter(|_| signed_in) {
                Some(cached) => {
                    eprintln!(
                        "Could not refresh the {} library, using the one from before: {err}",
                        store.display_name()
                    );
                    Ok(cached)
                }
                None => Err(err),
            }
        }
    }
}

/// What a store module gets, besides the session, to list an account's games.
pub struct Fetch<'a> {
    store: StoreId,
    /// The session on disk, as this refresh knows it.
    saved: Session,
    /// The games of the last list of this same account; empty on a first fetch. A module
    /// carries over from it what it could not look up this time, or what must stay the same
    /// from one list to the next (an id).
    pub previous: &'a [OwnedGame],
    /// A module sets this when some games could not be looked up; see
    /// [`OwnedLibrary::partial`].
    pub partial: bool,
}

impl<'a> Fetch<'a> {
    /// Saves a session whose tokens were just renewed, before the games are listed. A store
    /// that ends the old refresh token on use would otherwise lose the login when BoilR is
    /// closed during a long fetch.
    pub fn checkpoint(&mut self, session: &Session) -> eyre::Result<()> {
        let _lock = storage::lock_account(self.store)?;
        if load_session(self.store)?.as_ref() != Some(&self.saved) {
            return Err(account_changed(self.store));
        }
        save_session(self.store, session)?;
        self.saved = session.clone();
        Ok(())
    }

    /// The game with this id in the previous list.
    pub fn previous_game(&self, id: &str) -> Option<&'a OwnedGame> {
        self.previous.iter().find(|game| game.id == id)
    }
}

async fn refresh_library_locked(store: StoreId, asked: Asked) -> eyre::Result<OwnedLibrary> {
    let started_with = load_session(store)?
        .ok_or_else(|| eyre::format_err!("Not signed in to {}", store.display_name()))?;
    let previous = cached_library(store).map(|l| l.games).unwrap_or_default();
    let mut fetch = Fetch {
        store,
        saved: started_with.clone(),
        previous: &previous,
        partial: false,
    };
    let mut session = started_with;
    let client = http::client()?;
    let result = match store {
        StoreId::Epic => epic::fetch_owned(&client, &mut session, &mut fetch).await,
        StoreId::Gog => gog::fetch_owned(&client, &mut session, &mut fetch).await,
        StoreId::Amazon => amazon::fetch_owned(&client, &mut session, &mut fetch).await,
        StoreId::Ea => ea::fetch_owned(&client, &mut session, &mut fetch).await,
        StoreId::BattleNet => battlenet::fetch_owned(&client, &mut session, &mut fetch).await,
        StoreId::Humble => humble::fetch_owned(&client, &mut session, &mut fetch).await,
        StoreId::Itch => itch::fetch_owned(&client, &mut session, &mut fetch).await,
    };
    let fetched = Fetched {
        partial: fetch.partial,
        result,
    };
    commit_refresh(store, &fetch.saved, &session, fetched, asked)
}

/// What a store module's `fetch_owned` produced.
struct Fetched {
    result: eyre::Result<Vec<OwnedGame>>,
    partial: bool,
}

/// A library this long or longer is not replaced by one less than half its size when the
/// refresh was automatic.
const SHRINK_GUARD_MIN_GAMES: usize = 20;

/// Saves what a refresh produced: the session (tokens may have been renewed even when listing
/// the games failed) and the library.
///
/// A list that looks cut short is not saved, because the sync removes from Steam every game
/// the list lacks. An empty list never replaces a list with games. A list less than half as
/// long as the last one is only taken when the user asked for the refresh.
fn commit_refresh(
    store: StoreId,
    started_with: &Session,
    session: &Session,
    fetched: Fetched,
    asked: Asked,
) -> eyre::Result<OwnedLibrary> {
    let _lock = storage::lock_account(store)?;
    // Signing out (or in again) while the games were being fetched wins: saving now would
    // bring back the old login.
    if load_session(store)?.as_ref() != Some(started_with) {
        return Err(account_changed(store));
    }
    if session != started_with {
        save_session(store, session)?;
    }
    let mut games = fetched.result?;
    games.retain(|g| !g.id.is_empty() && !g.title.trim().is_empty());
    games.sort_by(|a, b| a.id.cmp(&b.id));
    games.dedup_by(|a, b| a.id == b.id);
    let before = cached_library(store).map_or(0, |library| library.games.len());
    if games.is_empty() && before > 0 {
        return Err(eyre::format_err!(
            "{} listed no games, but it listed {before} before. The earlier list is kept, \
             because an empty answer is more likely a store problem than an empty account. \
             If the account really has no games, sign out and in again.",
            store.display_name()
        ));
    }
    if asked == Asked::Automatically
        && before >= SHRINK_GUARD_MIN_GAMES
        && games.len().saturating_mul(2) < before
    {
        return Err(eyre::format_err!(
            "{} listed {} games, down from {before}. The earlier list is kept in case the \
             store's answer was cut short. Refresh the library from the account's settings \
             to take the shorter list.",
            store.display_name(),
            games.len()
        ));
    }
    let library = OwnedLibrary {
        account_name: session.account_name.clone(),
        fetched_at: now_unix(),
        games,
        partial: fetched.partial,
    };
    save_library(store, &library)?;
    Ok(library)
}

/// Signs out. Where the store offers a way, it is told first that BoilR's login is no longer
/// needed: Amazon removes BoilR from the account's devices and Epic ends the session. The
/// other stores keep the login valid until it expires. Then the session and the cached
/// library are removed, also when the store could not be told; that case returns a warning.
pub async fn sign_out(store: StoreId) -> eyre::Result<Option<String>> {
    let warning = match load_session(store).ok().flatten() {
        Some(session) => end_remote_session(store, &session).await.err().map(|err| {
            format!(
                "Signed out, but {} may still list BoilR's login: {}",
                store.display_name(),
                redacted(err)
            )
        }),
        None => None,
    };
    delete_account(store)?;
    Ok(warning)
}

async fn end_remote_session(store: StoreId, session: &Session) -> eyre::Result<()> {
    let client = http::client()?;
    match store {
        StoreId::Amazon => amazon::sign_out(&client, session).await,
        StoreId::Epic => epic::sign_out(&client, session).await,
        StoreId::Gog | StoreId::Ea | StoreId::BattleNet | StoreId::Humble | StoreId::Itch => Ok(()),
    }
}

/// Removes the session and the cached library, without telling the store; see [`sign_out`].
pub fn delete_account(store: StoreId) -> eyre::Result<()> {
    {
        let _lock = storage::lock_account(store)?;
        storage::delete_account(store)?;
    }
    end_pending_login(store);
    record_failure(store, None);
    Ok(())
}

/// Whether a library should be fetched again: it is older than `max_age_secs`, was never
/// fetched, or is [partial](OwnedLibrary::partial) and older than
/// [`RETRY_AFTER_FAILURE_SECS`].
pub fn is_stale(library: Option<&OwnedLibrary>, max_age_secs: u64) -> bool {
    match library {
        Some(library) => {
            let max_age = if library.partial {
                max_age_secs.min(RETRY_AFTER_FAILURE_SECS)
            } else {
                max_age_secs
            };
            now_unix().saturating_sub(library.fetched_at) > max_age
        }
        None => true,
    }
}

/// The state of one store account, for display.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountStatus {
    /// The same as `key`.
    pub store: StoreId,
    pub key: String,
    pub name: String,
    pub signed_in: bool,
    pub account_name: Option<String>,
    pub game_count: Option<usize>,
    pub fetched_at: Option<u64>,
    /// How to sign in without the login window. Its `url` is only good to show: the link
    /// to open comes from [`pending_login`] at the moment the user asks for it, because a
    /// store's link can be tied to the login in progress (Amazon's is).
    pub manual: ManualLogin,
    /// Whether the built-in login window can sign in to this store.
    pub webview_login: bool,
    /// Why the last refresh failed, while none has succeeded since.
    pub last_error: Option<String>,
    /// The last refresh failed because the store no longer accepts the saved login.
    pub needs_sign_in: bool,
}

/// Reads the session and the cached library from disk; front ends call it after a change,
/// not on every repaint. It starts no login: when none is in progress, the sign-in details
/// come from a login that is thrown away.
pub fn account_status(store: StoreId) -> AccountStatus {
    let session = load_session(store).ok().flatten();
    let library = load_library(store).ok().flatten();
    let pending = PENDING_LOGINS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&store)
        .cloned()
        .unwrap_or_else(|| begin_login(store));
    let failure = last_failure(store).filter(|_| session.is_some());
    AccountStatus {
        store,
        key: store.key().to_string(),
        name: store.display_name().to_string(),
        signed_in: session.is_some(),
        account_name: session
            .and_then(|s| s.account_name)
            .or_else(|| library.as_ref().and_then(|l| l.account_name.clone())),
        game_count: library.as_ref().map(|l| l.games.len()),
        fetched_at: library.map(|l| l.fetched_at),
        manual: pending.manual,
        webview_login: !pending.done_url_prefixes.is_empty(),
        needs_sign_in: failure.as_ref().is_some_and(|f| f.needs_sign_in),
        last_error: failure.map(|f| f.message),
    }
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tests below share the test folder, the pending logins and the failure records,
    /// so they run one at a time.
    fn alone() -> std::sync::MutexGuard<'static, ()> {
        storage::test_lock()
    }

    #[test]
    fn a_sign_in_keeps_its_state_until_it_ends() {
        let _alone = alone();
        // Amazon's sign-in link carries a PKCE challenge: the link the user opened and the
        // login that finishes it must share the verifier.
        let opened = pending_login(StoreId::Amazon);
        let finished = pending_login(StoreId::Amazon);
        assert_eq!(opened.start_url, finished.start_url);
        assert_eq!(opened.state, finished.state);

        end_pending_login(StoreId::Amazon);
        assert_ne!(pending_login(StoreId::Amazon).state, opened.state);
        end_pending_login(StoreId::Amazon);
    }

    #[test]
    fn reading_a_status_starts_no_login() {
        let _alone = alone();
        end_pending_login(StoreId::Amazon);
        let status = account_status(StoreId::Amazon);
        assert!(status.webview_login);
        assert!(matches!(status.manual, ManualLogin::Paste { .. }));
        assert!(!PENDING_LOGINS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains_key(&StoreId::Amazon));
    }

    #[test]
    fn recent_failures_hold_off_retries_and_survive_a_restart() {
        let _alone = alone();
        let store = StoreId::Humble;
        record_failure(store, Some(&http::login_rejected("401 from https://x")));
        assert!(failed_recently(store));
        // The record is a file, so it is still there when BoilR starts again.
        assert!(storage::load_failure(store).is_some_and(|f| f.needs_sign_in));
        record_failure(store, None);
        assert!(!failed_recently(store));
        assert!(storage::load_failure(store).is_none());
    }

    #[test]
    fn only_a_refused_login_is_recorded_as_needing_a_sign_in() {
        let _alone = alone();
        let store = StoreId::Humble;
        let needs_sign_in = |err: eyre::Report| {
            record_failure(store, Some(&redacted(err)));
            let failure = last_failure(store);
            record_failure(store, None);
            failure.is_some_and(|f| f.needs_sign_in)
        };
        assert!(needs_sign_in(http::login_rejected("signed out")));
        // An outage, a bot block, and a message that merely says so do not.
        assert!(!needs_sign_in(http::transient("no connection")));
        assert!(!needs_sign_in(eyre::Report::new(http::HttpStatus {
            status: 403,
            url: "https://store.example/".into(),
            body: String::new(),
        })));
        assert!(!needs_sign_in(eyre::format_err!(
            "the store is down. Sign in again."
        )));
    }

    #[test]
    fn stores_are_written_by_their_key() -> eyre::Result<()> {
        let _alone = alone();
        for store in StoreId::ALL {
            assert_eq!(serde_json::to_value(store)?, serde_json::json!(store.key()));
            assert_eq!(StoreId::from_key(store.key()), Some(store));
        }
        // What early builds wrote is still read.
        let old: StoreId = serde_json::from_str("\"battle_net\"")?;
        assert_eq!(old, StoreId::BattleNet);

        let status = serde_json::to_value(account_status(StoreId::BattleNet))?;
        assert_eq!(status.get("store"), Some(&serde_json::json!("battlenet")));
        assert_eq!(status.get("key"), Some(&serde_json::json!("battlenet")));
        let back: AccountStatus = serde_json::from_value(status)?;
        assert_eq!(back.store, StoreId::BattleNet);
        Ok(())
    }

    #[test]
    fn redacted_errors_keep_their_kind_and_lose_secrets() {
        let err = http::login_rejected("401 from https://a.com/x?refresh_token=SECRET")
            .wrap_err("Could not load the library");
        let clean = redacted(err);
        assert!(http::is_login_rejected(&clean));
        let message = clean.to_string();
        assert!(message.contains("Could not load the library"), "{message}");
        assert!(message.contains("https://a.com/x"), "{message}");
        assert!(!message.contains("SECRET"), "{message}");

        let outage = redacted(http::transient("timed out").wrap_err("Could not load"));
        assert!(http::is_transient(&outage) && !http::is_login_rejected(&outage));
        let plain = redacted(eyre::format_err!("unexpected answer"));
        assert!(!http::is_transient(&plain) && !http::is_login_rejected(&plain));
    }

    #[test]
    fn debug_output_shows_no_secrets() {
        let session = Session {
            account_name: Some("player".into()),
            data: serde_json::json!({ "refresh_token": "SECRET" }),
        };
        let cookie = Cookie {
            name: "sid".into(),
            value: "SECRET".into(),
            domain: "accounts.ea.com".into(),
        };
        let capture = LoginCapture {
            final_url: Some("https://x/?code=SECRET".into()),
            page_text: Some("SECRET".into()),
            cookies: vec![cookie.clone()],
            pasted: Some("SECRET".into()),
        };
        let pending = begin_login(StoreId::Amazon);
        let shown = format!("{session:?} {cookie:?} {capture:?} {pending:?}");
        assert!(!shown.contains("SECRET"), "{shown}");
        assert!(shown.contains("player") && shown.contains("accounts.ea.com"));
        let verifier = pending
            .state
            .get("code_verifier")
            .and_then(|v| v.as_str())
            .unwrap_or("no verifier in the state");
        assert!(!shown.contains(verifier), "{shown}");
    }

    #[test]
    fn titles_lose_trademark_signs_and_extra_spaces() {
        assert_eq!(clean_title("  FEZ\u{2122}  "), "FEZ");
        assert_eq!(clean_title("The Sims\u{ae}  4 \u{a9}"), "The Sims 4");
        assert_eq!(clean_title("Plain Title"), "Plain Title");
    }

    #[test]
    fn unreadable_session_data_asks_for_a_new_sign_in() {
        #[derive(Debug, Deserialize)]
        struct Needs {
            #[allow(dead_code)]
            token: String,
        }
        let broken = Session {
            account_name: None,
            data: serde_json::json!({ "other": 1 }),
        };
        let result = session_data::<Needs>(StoreId::Gog, &broken);
        assert!(result.is_err_and(|e| http::is_login_rejected(&e)));
    }

    fn session(token: &str) -> Session {
        Session {
            account_name: Some("player".into()),
            data: serde_json::json!({ "token": token }),
        }
    }

    fn library(count: usize) -> OwnedLibrary {
        OwnedLibrary {
            account_name: Some("player".into()),
            fetched_at: 1,
            games: (0..count)
                .map(|i| OwnedGame::new(StoreId::Itch, i.to_string(), "Game"))
                .collect(),
            partial: false,
        }
    }

    fn games(count: usize) -> Fetched {
        Fetched {
            result: Ok(library(count).games),
            partial: false,
        }
    }

    fn commit(
        store: StoreId,
        started_with: &Session,
        session: &Session,
        fetched: Fetched,
    ) -> eyre::Result<OwnedLibrary> {
        commit_refresh(store, started_with, session, fetched, Asked::Explicitly)
    }

    #[test]
    fn a_refresh_never_saves_over_a_sign_out() -> eyre::Result<()> {
        let _alone = alone();
        let store = StoreId::Itch;
        let started_with = session("old");
        save_session(store, &started_with)?;
        delete_account(store)?;

        let result = commit(store, &started_with, &session("renewed"), games(2));
        assert!(result.is_err_and(|e| is_account_changed(&e)));
        assert_eq!(load_session(store)?, None);
        assert!(cached_library(store).is_none());
        Ok(())
    }

    #[test]
    fn a_refresh_never_saves_over_a_new_sign_in() -> eyre::Result<()> {
        let _alone = alone();
        let store = StoreId::BattleNet;
        let started_with = session("old");
        save_session(store, &started_with)?;
        save_new_login(store, &session("new"))?;

        let result = commit(store, &started_with, &session("renewed"), games(2));
        assert!(result.is_err_and(|e| is_account_changed(&e)));
        assert_eq!(load_session(store)?, Some(session("new")));
        delete_account(store)?;
        Ok(())
    }

    #[test]
    fn a_refresh_saves_renewed_tokens_even_when_listing_fails() -> eyre::Result<()> {
        let _alone = alone();
        let store = StoreId::Amazon;
        let started_with = session("old");
        save_session(store, &started_with)?;

        let failed = commit(
            store,
            &started_with,
            &session("renewed"),
            Fetched {
                result: Err(eyre::format_err!("network down")),
                partial: false,
            },
        );
        assert!(failed.is_err());
        assert_eq!(load_session(store)?, Some(session("renewed")));

        let renewed = session("renewed");
        let saved = commit(store, &renewed, &renewed, games(2))?;
        assert_eq!(saved.games.len(), 2);
        assert_eq!(cached_library(store).map(|l| l.games.len()), Some(2));
        delete_account(store)?;
        Ok(())
    }

    #[test]
    fn renewed_tokens_can_be_saved_before_the_games_are_listed() -> eyre::Result<()> {
        let _alone = alone();
        let store = StoreId::Gog;
        let started_with = session("old");
        save_session(store, &started_with)?;
        let mut fetch = Fetch {
            store,
            saved: started_with,
            previous: &[],
            partial: false,
        };

        fetch.checkpoint(&session("renewed"))?;
        assert_eq!(load_session(store)?, Some(session("renewed")));
        // The refresh goes on from the saved session.
        let saved = commit(store, &fetch.saved, &session("renewed"), games(1))?;
        assert_eq!(saved.games.len(), 1);

        // After a sign-out there is nothing to save over.
        delete_account(store)?;
        let gone = fetch.checkpoint(&session("again"));
        assert!(gone.is_err_and(|e| is_account_changed(&e)));
        assert_eq!(load_session(store)?, None);
        Ok(())
    }

    #[test]
    fn an_empty_answer_does_not_replace_a_library() -> eyre::Result<()> {
        let _alone = alone();
        let store = StoreId::Humble;
        let current = session("t");
        save_session(store, &current)?;
        save_library(store, &library(3))?;

        assert!(commit(store, &current, &current, games(0)).is_err());
        assert_eq!(cached_library(store).map(|l| l.games.len()), Some(3));

        // After signing out and in again there is no earlier list to protect.
        delete_account(store)?;
        save_session(store, &current)?;
        assert!(commit(store, &current, &current, games(0))?
            .games
            .is_empty());
        delete_account(store)?;
        Ok(())
    }

    #[test]
    fn a_much_shorter_list_is_only_taken_when_the_user_asked() -> eyre::Result<()> {
        let _alone = alone();
        let store = StoreId::Epic;
        let current = session("t");
        save_session(store, &current)?;
        save_library(store, &library(40))?;

        let automatic = |count| {
            commit_refresh(
                store,
                &current,
                &current,
                games(count),
                Asked::Automatically,
            )
        };
        // Less than half of what was there looks like a list cut short.
        assert!(automatic(19).is_err());
        assert_eq!(cached_library(store).map(|l| l.games.len()), Some(40));
        // Losing a few games is normal.
        assert_eq!(automatic(30)?.games.len(), 30);
        // Asked for by the user, the short list is taken.
        assert_eq!(commit(store, &current, &current, games(5))?.games.len(), 5);
        // Short libraries are not guarded.
        assert_eq!(automatic(1)?.games.len(), 1);
        delete_account(store)?;
        Ok(())
    }

    #[test]
    fn signing_in_again_keeps_the_library_of_the_same_account_only() -> eyre::Result<()> {
        let _alone = alone();
        let store = StoreId::Epic;
        let fresh = OwnedLibrary {
            fetched_at: now_unix(),
            ..library(3)
        };
        save_library(store, &fresh)?;
        // The same account: its games stay, to be fetched again on next use.
        save_new_login(store, &session("new"))?;
        assert!(is_stale(cached_library(store).as_ref(), 60));
        assert_eq!(cached_library(store).map(|l| l.games.len()), Some(3));

        // Another account, or one whose name is not known: no games carried over.
        save_library(store, &fresh)?;
        let other = Session {
            account_name: Some("someone else".into()),
            ..session("other")
        };
        save_new_login(store, &other)?;
        assert!(cached_library(store).is_none());
        // So an account without games is taken as it is.
        assert!(commit(store, &other, &other, games(0))?.games.is_empty());

        save_library(store, &fresh)?;
        let unnamed = Session {
            account_name: None,
            ..session("unnamed")
        };
        save_new_login(store, &unnamed)?;
        assert!(cached_library(store).is_none());
        delete_account(store)?;
        Ok(())
    }

    #[test]
    fn a_partial_library_is_fetched_again_sooner() {
        let complete = OwnedLibrary {
            fetched_at: now_unix().saturating_sub(RETRY_AFTER_FAILURE_SECS + 60),
            ..library(1)
        };
        let partial = OwnedLibrary {
            partial: true,
            ..complete.clone()
        };
        let six_hours = 6 * 60 * 60;
        assert!(!is_stale(Some(&complete), six_hours));
        assert!(is_stale(Some(&partial), six_hours));
        assert!(is_stale(None, six_hours));
    }
}
