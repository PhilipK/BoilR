//! Store accounts: sign in to a store in a login window, then list every game the account
//! owns. The store logic lives in `boilr-stores`; this module runs the login window and
//! exposes the accounts to the UI.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, PoisonError,
};

use boilr::platforms::{
    accounts::{account_code_name, stores, InstallWith, StoreId},
    get_platforms,
};
use serde::Serialize;
use tauri::{
    webview::PageLoadEvent, AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};
use toml::Value;

use crate::{load_platform_table, persist_platform_table};

#[derive(Debug, Clone, Serialize)]
pub struct StoreAccount {
    #[serde(flatten)]
    status: stores::AccountStatus,
    code_name: String,
    enabled: bool,
    install_with: InstallWith,
    /// Leave out betas, demos, playtests and test servers.
    skip_prereleases: bool,
    /// Whether "Heroic first" is a choice here: Heroic installs games on Linux only.
    heroic_available: bool,
    /// Something the user should know about the action that returned this account, such as
    /// a sign-out the store could not be told about, or a first fetch that failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    notice: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct LoginEvent {
    store: String,
    ok: bool,
    message: String,
}

fn parse_store(key: &str) -> Result<StoreId, String> {
    StoreId::from_key(key).ok_or_else(|| format!("Unknown store {key}"))
}

fn account(store: StoreId) -> StoreAccount {
    let code_name = account_code_name(store);
    let platform_settings = load_platform_table(code_name).unwrap_or_default();
    let enabled = platform_settings
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let install_with = platform_settings
        .get("install_with")
        .cloned()
        .and_then(|value| value.try_into::<InstallWith>().ok())
        .unwrap_or_default();
    let skip_prereleases = platform_settings
        .get("skip_prereleases")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    StoreAccount {
        status: stores::account_status(store),
        code_name: code_name.to_string(),
        enabled,
        install_with,
        skip_prereleases,
        heroic_available: cfg!(not(windows)),
        notice: None,
    }
}

fn account_with_notice(store: StoreId, notice: Option<String>) -> StoreAccount {
    StoreAccount {
        notice,
        ..account(store)
    }
}

/// A change to one account's settings; `None` leaves a setting as it is.
#[derive(Default)]
struct AccountChange {
    enabled: Option<bool>,
    install_with: Option<InstallWith>,
    skip_prereleases: Option<bool>,
}

fn set_account_settings(store: StoreId, change: AccountChange) -> Result<(), String> {
    let AccountChange {
        enabled,
        install_with,
        skip_prereleases,
    } = change;
    let code_name = account_code_name(store);
    let mut table = load_platform_table(code_name)?;
    if let Some(enabled) = enabled {
        table.insert("enabled".to_string(), Value::Boolean(enabled));
    }
    if let Some(skip) = skip_prereleases {
        table.insert("skip_prereleases".to_string(), Value::Boolean(skip));
    }
    if let Some(install_with) = install_with {
        let value = Value::try_from(install_with).map_err(|err| err.to_string())?;
        table.insert("install_with".to_string(), value);
    }
    persist_platform_table(code_name, table)
}

#[tauri::command(async)]
pub fn list_store_accounts() -> Vec<StoreAccount> {
    // Make sure every account section exists, so settings and sources stay in step.
    let _ = get_platforms();
    StoreId::ALL.into_iter().map(account).collect()
}

#[tauri::command(async)]
pub fn update_store_account(
    store: String,
    enabled: Option<bool>,
    install_with: Option<InstallWith>,
    skip_prereleases: Option<bool>,
) -> Result<StoreAccount, String> {
    let store = parse_store(&store)?;
    set_account_settings(
        store,
        AccountChange {
            enabled,
            install_with,
            skip_prereleases,
        },
    )?;
    Ok(account(store))
}

#[tauri::command]
pub async fn refresh_store_library(store: String) -> Result<StoreAccount, String> {
    let store = parse_store(&store)?;
    stores::refresh_library(store)
        .await
        .map_err(|err| err.to_string())?;
    Ok(account(store))
}

#[tauri::command]
pub async fn sign_out_store(store: String) -> Result<StoreAccount, String> {
    let store = parse_store(&store)?;
    let warning = stores::sign_out(store)
        .await
        .map_err(|err| err.to_string())?;
    // A source without a sign-in would only report an error on every sync.
    set_account_settings(
        store,
        AccountChange {
            enabled: Some(false),
            ..Default::default()
        },
    )?;
    Ok(account_with_notice(store, warning))
}

/// Opens the store's sign-in page for a manual login in the user's own browser.
#[tauri::command(async)]
pub fn open_store_sign_in_page(store: String) -> Result<(), String> {
    let store = parse_store(&store)?;
    let url = match stores::pending_login(store).manual {
        stores::ManualLogin::Paste { url, .. } => url,
        stores::ManualLogin::Unsupported => {
            return Err(format!("{} has no manual sign-in", store.display_name()))
        }
    };
    open_in_browser(&url).map_err(|err| format!("Could not open the browser: {err}"))
}

fn open_in_browser(url: &str) -> std::io::Result<()> {
    // Windows' URL handler opens the default browser. `explorer <url>` does that only for
    // short, simple links: a long sign-in link (Amazon's) or one with commas opens File
    // Explorer instead.
    #[cfg(windows)]
    let mut command = {
        let mut command = std::process::Command::new("rundll32.exe");
        command.arg("url.dll,FileProtocolHandler").arg(url);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(url);
        command
    };
    command.spawn().map(|_| ())
}

/// Finishes a login with a value the user pasted (an API key, a code or a URL). Uses the
/// same pending login as the sign-in page the user opened, which Amazon needs (its PKCE
/// verifier and device serial are part of the sign-in link).
#[tauri::command]
pub async fn submit_store_login(store: String, pasted: String) -> Result<StoreAccount, String> {
    let store = parse_store(&store)?;
    let pending = stores::pending_login(store);
    let capture = stores::LoginCapture {
        pasted: Some(pasted.trim().to_string()),
        ..Default::default()
    };
    stores::complete_login(&pending, &capture)
        .await
        .map_err(|err| err.to_string())?;
    let notice = after_sign_in(store).await;
    Ok(account_with_notice(store, notice))
}

/// A first library fetch. Signing in adds nothing to Steam by itself: the account becomes
/// a game source when the user turns it on, knowing how many games that adds. Returns what
/// the user should know next.
async fn after_sign_in(store: StoreId) -> Option<String> {
    Some(match stores::refresh_library(store).await {
        Ok(library) => {
            let enabled = account(store).enabled;
            games_found(library.games.len(), enabled)
        }
        Err(err) => format!("Could not list its games yet: {err}"),
    })
}

fn games_found(count: usize, enabled: bool) -> String {
    let games = if count == 1 { "game" } else { "games" };
    if enabled {
        format!("This account owns {count} {games}.")
    } else {
        format!(
            "This account owns {count} {games}. Turn on \"Add every game this account owns\" to add them to Steam."
        )
    }
}

/// Fetches the libraries that got old, off the main thread, and tells the windows which
/// stores have a new list (`store-libraries-refreshed`, with the store keys). Game lists
/// read what was last fetched, so the app never waits on a store to show them.
pub fn refresh_libraries_in_background(app: AppHandle) {
    std::thread::spawn(move || {
        let refreshed: Vec<&str> = boilr::platforms::accounts::refresh_stale_libraries()
            .into_iter()
            .map(|store| store.key())
            .collect();
        if !refreshed.is_empty() {
            let _ = app.emit("store-libraries-refreshed", refreshed);
        }
    });
}

/// Where a login window is sent to hand over the text of a page it loaded: a host that
/// does not exist, so the navigation is only ever seen by BoilR, which stops it.
const PAGE_TEXT_URL: &str = "https://boilr.invalid/login-page-text";

/// Sends the window to [`PAGE_TEXT_URL`] with the text the page shows. A login window has
/// no access to BoilR's commands, so a navigation is how its page text gets out.
const HAND_OVER_PAGE_TEXT: &str = "window.location.replace('https://boilr.invalid/login-page-text?text=' + encodeURIComponent((document.body ? document.body.innerText : '').slice(0, 8000)));";

/// How long a loaded done page gets to hand over its text before the login goes on
/// without it.
const PAGE_TEXT_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

fn starts_with_any(url: &str, prefixes: &[String]) -> bool {
    prefixes
        .iter()
        .any(|prefix| url.starts_with(prefix.as_str()))
}

/// The text a login window handed over, from the URL it was sent to.
fn page_text_from(url: &tauri::Url) -> Option<String> {
    if !url.as_str().starts_with(PAGE_TEXT_URL) {
        return None;
    }
    Some(
        url.query_pairs()
            .find(|(name, _)| name == "text")
            .map(|(_, text)| text.into_owned())
            .unwrap_or_default(),
    )
}

/// Opens the store's sign-in page in its own window. When the window reaches the page the
/// store lands on after signing in, the navigation is stopped, the cookies are collected,
/// and the login is completed. For a store whose done page holds the login code (see
/// `PendingLogin::page_text_url_prefixes`), the page is loaded and its text handed over
/// first. The result arrives as a `store-login` event.
#[tauri::command]
pub async fn start_store_login(app: AppHandle, store: String) -> Result<(), String> {
    let store = parse_store(&store)?;
    let pending = stores::pending_login(store);
    if pending.done_url_prefixes.is_empty() {
        return Err(format!(
            "{} signs in with a pasted value, not a login window",
            store.display_name()
        ));
    }
    let label = format!("login-{}", store.key());
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.set_focus();
        return Ok(());
    }
    let start_url = pending
        .start_url
        .parse()
        .map_err(|err| format!("Bad sign-in URL: {err}"))?;

    let finished = Arc::new(AtomicBool::new(false));
    // The done page the window is loading for its text, once it got there.
    let reading_page: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let pending = Arc::new(pending);
    let mut builder = WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(start_url))
        .title(format!("Sign in to {}", store.display_name()))
        .inner_size(520.0, 760.0)
        // A private session per login: nothing of a previous sign-in leaks in or stays behind.
        .incognito(true);
    if let Some(user_agent) = pending.user_agent.as_deref() {
        builder = builder.user_agent(user_agent);
    }
    let finish = {
        let app = app.clone();
        let label = label.clone();
        let finished = finished.clone();
        let pending = pending.clone();
        move |final_url: Option<String>, page_text: Option<String>| {
            if !finished.swap(true, Ordering::SeqCst) {
                tauri::async_runtime::spawn(finish_login(
                    app.clone(),
                    label.clone(),
                    pending.clone(),
                    final_url,
                    page_text,
                ));
            }
        }
    };
    let on_navigation = {
        let finish = finish.clone();
        let pending = pending.clone();
        let reading_page = reading_page.clone();
        move |url: &tauri::Url| {
            if let Some(text) = page_text_from(url) {
                // Only a done page BoilR chose to load may hand over text.
                let done_page = reading_page
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone();
                if let Some(done_page) = done_page {
                    finish(Some(done_page), Some(text));
                }
                return false;
            }
            if !starts_with_any(url.as_str(), &pending.done_url_prefixes) {
                return true;
            }
            if starts_with_any(url.as_str(), &pending.page_text_url_prefixes) {
                *reading_page.lock().unwrap_or_else(PoisonError::into_inner) =
                    Some(url.to_string());
                return true;
            }
            finish(Some(url.to_string()), None);
            false
        }
    };
    let on_page_load = {
        let finish = finish.clone();
        let pending = pending.clone();
        let reading_page = reading_page.clone();
        move |window: tauri::WebviewWindow, payload: tauri::webview::PageLoadPayload<'_>| {
            if payload.event() != PageLoadEvent::Finished {
                return;
            }
            // The page that loaded is the done page, or where the store sent the window
            // from it.
            if !starts_with_any(payload.url().as_str(), &pending.page_text_url_prefixes) {
                return;
            }
            let done_page = reading_page
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            let Some(done_page) = done_page else {
                return;
            };
            if let Err(err) = window.eval(HAND_OVER_PAGE_TEXT) {
                eprintln!("Could not read the sign-in page: {err}");
            }
            // A page that does not hand its text over must not leave the login hanging:
            // the store module then falls back to what it can do with the cookies.
            let finish = finish.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(PAGE_TEXT_WAIT).await;
                finish(Some(done_page), None);
            });
        }
    };
    let window = builder
        .on_navigation(on_navigation)
        .on_page_load(on_page_load)
        .build()
        .map_err(|err| format!("Could not open the sign-in window: {err}"))?;

    let app_for_close = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) && !finished.swap(true, Ordering::SeqCst)
        {
            let _ = app_for_close.emit(
                "store-login",
                LoginEvent {
                    store: store.key().to_string(),
                    ok: false,
                    message: "Sign-in window closed".to_string(),
                },
            );
        }
    });
    Ok(())
}

async fn finish_login(
    app: AppHandle,
    label: String,
    pending: Arc<stores::PendingLogin>,
    final_url: Option<String>,
    page_text: Option<String>,
) {
    let store = pending.store;
    let cookies = collect_cookies(&app, &label, &pending.cookie_urls).await;
    let capture = stores::LoginCapture {
        final_url,
        page_text,
        cookies,
        pasted: None,
    };
    let result = stores::complete_login(&pending, &capture).await;
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.destroy();
    }
    let event = match result {
        Ok(name) => {
            let notice = after_sign_in(store).await;
            let signed_in = match name {
                Some(name) => format!("Signed in as {name}."),
                None => "Signed in.".to_string(),
            };
            LoginEvent {
                store: store.key().to_string(),
                ok: true,
                message: match notice {
                    Some(notice) => format!("{signed_in} {notice}"),
                    None => signed_in,
                },
            }
        }
        Err(err) => LoginEvent {
            store: store.key().to_string(),
            ok: false,
            message: err.to_string(),
        },
    };
    let _ = app.emit("store-login", event);
}

/// Reads the login window's cookies: every cookie of its private session, and those the
/// webview reports for `urls` (in case it cannot list them all). WebView2 deadlocks when
/// cookies are read from an event handler, so this runs on a blocking thread.
///
/// A cookie's domain arrives without its leading dot (the `cookie` crate drops it), so a
/// domain cookie and a host-only one look the same here; `boilr-stores` allows for that.
async fn collect_cookies(app: &AppHandle, label: &str, urls: &[String]) -> Vec<stores::Cookie> {
    let Some(window) = app.get_webview_window(label) else {
        return vec![];
    };
    let urls = urls.to_vec();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut cookies = vec![];
        match window.cookies() {
            Ok(found) => {
                cookies.extend(found.iter().filter_map(|cookie| store_cookie(cookie, None)))
            }
            Err(err) => eprintln!("Could not read the sign-in window's cookies: {err}"),
        }
        for url in urls {
            let Ok(parsed) = url.parse::<tauri::Url>() else {
                continue;
            };
            let host = parsed.host_str().unwrap_or_default().to_string();
            match window.cookies_for_url(parsed) {
                Ok(found) => cookies.extend(
                    found
                        .iter()
                        .filter_map(|cookie| store_cookie(cookie, Some(&host))),
                ),
                Err(err) => eprintln!("Could not read cookies for {url}: {err}"),
            }
        }
        cookies
    })
    .await;
    one_per_name_and_domain(result.unwrap_or_default())
}

/// A webview cookie as `boilr-stores` takes it. A cookie without a domain belongs to the
/// host it was asked for; without either it cannot be placed.
fn store_cookie(cookie: &tauri::webview::Cookie<'_>, host: Option<&str>) -> Option<stores::Cookie> {
    let domain = cookie
        .domain()
        .or(host)
        .filter(|domain| !domain.is_empty())?;
    Some(stores::Cookie {
        name: cookie.name().to_string(),
        value: cookie.value().to_string(),
        domain: domain.to_string(),
    })
}

fn one_per_name_and_domain(mut cookies: Vec<stores::Cookie>) -> Vec<stores::Cookie> {
    cookies.sort_by(|a, b| (&a.domain, &a.name).cmp(&(&b.domain, &b.name)));
    cookies.dedup_by(|a, b| a.domain == b.domain && a.name == b.name);
    cookies
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_text_comes_from_the_hand_over_address_only() {
        let handed_over: tauri::Url =
            "https://boilr.invalid/login-page-text?text=%7B%22authorizationCode%22%3A%22abc%22%7D"
                .parse()
                .expect("a URL");
        assert_eq!(
            page_text_from(&handed_over).as_deref(),
            Some("{\"authorizationCode\":\"abc\"}")
        );
        let empty: tauri::Url = PAGE_TEXT_URL.parse().expect("a URL");
        assert_eq!(page_text_from(&empty).as_deref(), Some(""));
        let other: tauri::Url = "https://www.epicgames.com/id/api/redirect?text=x"
            .parse()
            .expect("a URL");
        assert_eq!(page_text_from(&other), None);
        // The script sends the window to the address that is read here.
        assert!(HAND_OVER_PAGE_TEXT.contains(&format!("'{PAGE_TEXT_URL}?text='")));
    }

    #[test]
    fn cookies_keep_the_domain_the_webview_gives_or_the_host_asked_for() {
        // The `cookie` crate drops a domain's leading dot: a `.battle.net` cookie arrives
        // as `battle.net`, like a host-only cookie of that host would.
        let domain_cookie = tauri::webview::Cookie::build(("login", "1"))
            .domain(".battle.net")
            .build();
        let host_only = tauri::webview::Cookie::build(("sid", "2")).build();
        assert_eq!(
            store_cookie(&domain_cookie, Some("oauth.battle.net")).map(|c| c.domain),
            Some("battle.net".to_string())
        );
        assert_eq!(
            store_cookie(&host_only, Some("accounts.ea.com")).map(|c| c.domain),
            Some("accounts.ea.com".to_string())
        );
        assert!(store_cookie(&host_only, None).is_none());

        let kept = one_per_name_and_domain(vec![
            stores::Cookie {
                name: "sid".into(),
                value: "1".into(),
                domain: "accounts.ea.com".into(),
            },
            stores::Cookie {
                name: "sid".into(),
                value: "1".into(),
                domain: "accounts.ea.com".into(),
            },
            stores::Cookie {
                name: "sid".into(),
                value: "2".into(),
                domain: "ea.com".into(),
            },
        ]);
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn signing_in_says_how_many_games_turning_the_source_on_adds() {
        assert_eq!(games_found(1, true), "This account owns 1 game.");
        assert!(games_found(212, false).starts_with("This account owns 212 games. Turn on"));
    }
}
