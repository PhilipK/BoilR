//! Humble Bundle: the DRM-free games in the account's purchases (bundles, store orders,
//! claimed Choice games), installed or not.
//!
//! Modelled on Playnite's HumbleLibrary (`source/Libraries/HumbleLibrary`): the session is
//! the `_simpleauth_sess` cookie of humblebundle.com; the order keys come from
//! `api/v1/user/order` (or, as Playnite does, from the JSON embedded in the library page),
//! and the orders themselves from `api/v1/orders?all_tpkds=true&gamekeys=...` in batches of 40.
//!
//! A subproduct counts as a game when it has a Windows or a Linux download: BoilR runs on
//! those two, and Linux runs Windows games through Proton. A game with only a Mac download is
//! not listed. Products that are only keys for another store (Steam, GOG, ...) have no
//! download and are skipped; like Playnite's default (`IgnoreThirdPartyStoreGames`), a
//! DRM-free game is also skipped when the account got a third-party key for it, because it
//! shows up in that store's library. Humble Trove / Games Collection is not listed
//! (Playnite's default too).
//!
//! `OwnedGame.id` is the subproduct's `machine_name`. A title the account has in several
//! orders keeps the id it was first listed with. `extra` keys:
//! - `url`: the page where the user downloads it (`/downloads?key=<order key>`).

use std::collections::{BTreeMap, BTreeSet};

use eyre::WrapErr;
use reqwest::{header::HeaderMap, Client};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    clean_title, http, session_data, Cookie, Fetch, LoginCapture, ManualLogin, OwnedGame,
    PendingLogin, Session, StoreId,
};

const LOGIN_URL: &str = "https://www.humblebundle.com/login?goto=%2Fhome%2Flibrary";
const LIBRARY_URL: &str = "https://www.humblebundle.com/home/library";
/// After signing in, Humble sends the browser to `goto`, the library page. Only that page
/// ends the login, not any `/home` page the user might reach on the way.
const DONE_URL_PREFIX: &str = "https://www.humblebundle.com/home/library";
const COOKIE_URL: &str = "https://www.humblebundle.com/";
const COOKIE_HOST: &str = "www.humblebundle.com";
const ORDER_LIST_URL: &str = "https://www.humblebundle.com/api/v1/user/order";
const ORDERS_URL: &str = "https://www.humblebundle.com/api/v1/orders";
const SESSION_COOKIE: &str = "_simpleauth_sess";
/// Playnite asks for 40 orders per request.
const ORDERS_PER_REQUEST: usize = 40;
/// Download platforms that make a subproduct a game BoilR can launch where it runs: Windows,
/// and Linux (natively, or the Windows download through Proton). Not `mac`.
const GAME_PLATFORMS: [&str; 2] = ["windows", "linux"];

/// What BoilR stores for a Humble account. No `Debug`: the cookie is the login.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct HumbleSession {
    /// Value of the `_simpleauth_sess` cookie, as the browser holds it (often in quotes).
    simpleauth_sess: String,
}

pub fn begin_login() -> PendingLogin {
    PendingLogin {
        page_text_url_prefixes: vec![],
        store: StoreId::Humble,
        start_url: LOGIN_URL.to_string(),
        done_url_prefixes: vec![DONE_URL_PREFIX.to_string()],
        cookie_urls: vec![COOKIE_URL.to_string()],
        manual: ManualLogin::Paste {
            url: LOGIN_URL.to_string(),
            instructions: format!(
                "Sign in to humblebundle.com in your browser. Then open the developer tools \
                 (F12), go to Storage (Firefox) or Application (Chrome) > Cookies > \
                 https://www.humblebundle.com, and copy the value of the cookie named \
                 {SESSION_COOKIE}. Paste it here. It is your login, so do not share it."
            ),
            placeholder: format!("{SESSION_COOKIE} cookie value"),
        },
        user_agent: None,
        state: serde_json::Value::Null,
    }
}

pub async fn complete_login(
    client: &Client,
    _pending: &PendingLogin,
    capture: &LoginCapture,
) -> eyre::Result<Session> {
    let mut session = HumbleSession {
        simpleauth_sess: session_cookie_from_capture(capture)?,
    };
    // Proves the cookie works before it is stored.
    fetch_gamekeys(client, &mut session).await?;
    Ok(Session {
        account_name: None,
        data: serde_json::to_value(&session)?,
    })
}

pub async fn fetch_owned(
    client: &Client,
    session: &mut Session,
    fetch: &mut Fetch<'_>,
) -> eyre::Result<Vec<OwnedGame>> {
    let mut data: HumbleSession = session_data(StoreId::Humble, session)?;
    let orders = fetch_all_orders(client, &mut data).await;
    // Humble may hand out a renewed cookie; keep it even when a step failed.
    session.data = serde_json::to_value(&data)?;
    Ok(owned_games(&orders?, fetch.previous))
}

/// Every order of the account: the order keys first, then the orders in batches.
async fn fetch_all_orders(
    client: &Client,
    session: &mut HumbleSession,
) -> eyre::Result<Vec<Order>> {
    let gamekeys = fetch_gamekeys(client, session)
        .await
        .wrap_err("Could not list the Humble Bundle orders")?;
    fetch_orders(client, session, &gamekeys)
        .await
        .wrap_err("Could not load the Humble Bundle orders")
}

/// The order keys of the account: from `api/v1/user/order`, or from the library page when
/// that endpoint does not answer as expected.
async fn fetch_gamekeys(client: &Client, session: &mut HumbleSession) -> eyre::Result<Vec<String>> {
    let api_error = match order_list(client, session).await {
        Ok(keys) => return Ok(keys),
        Err(err) if library_page_may_help(&err) => err,
        Err(err) => return Err(err),
    };
    let page = get_text(client, LIBRARY_URL, &[], session).await?;
    parse_library_page_gamekeys(&page).ok_or(api_error)
}

async fn order_list(client: &Client, session: &mut HumbleSession) -> eyre::Result<Vec<String>> {
    let text = get_text(client, ORDER_LIST_URL, &[("ajax", "true")], session).await?;
    parse_gamekeys(&text)
}

/// Whether the library page is worth asking for after the order list failed with `err`:
/// only when the API answered something unexpected. Without a connection, during an outage
/// or with a login Humble refuses, the page fails the same way after another wait.
fn library_page_may_help(err: &eyre::Report) -> bool {
    !http::is_transient(err) && !http::is_login_rejected(err)
}

async fn fetch_orders(
    client: &Client,
    session: &mut HumbleSession,
    gamekeys: &[String],
) -> eyre::Result<Vec<Order>> {
    let mut orders = Vec::new();
    for batch in gamekeys.chunks(ORDERS_PER_REQUEST) {
        let mut query = vec![("all_tpkds", "true")];
        query.extend(batch.iter().map(|key| ("gamekeys", key.as_str())));
        let text = get_text(client, ORDERS_URL, &query, session).await?;
        orders.extend(parse_orders(&text)?);
    }
    Ok(orders)
}

/// GETs a humblebundle.com page with the session cookie and returns the body. Turns a bounce
/// to the login page into a rejected login, and picks up a renewed session cookie.
async fn get_text(
    client: &Client,
    url: &str,
    query: &[(&str, &str)],
    session: &mut HumbleSession,
) -> eyre::Result<String> {
    let request = client
        .get(url)
        .query(query)
        .header(
            reqwest::header::COOKIE,
            format!("{SESSION_COOKIE}={}", session.simpleauth_sess),
        )
        .header("X-Requested-With", "XMLHttpRequest");
    let response = http::send(request).await?;
    // Checked before anything is taken from the response: the login page hands a visitor
    // an anonymous session cookie, which must not replace the saved one.
    if is_login_page(response.url()) {
        return Err(not_signed_in());
    }
    if let Some(renewed) = renewed_session_cookie(response.headers()) {
        session.simpleauth_sess = renewed;
    }
    response.text().await.map_err(http::request_failed)
}

/// Whether a request ended on Humble's login page, where it sends a browser whose session
/// it does not accept. Only the path counts: the query of any page can name `/login`
/// (`?goto=/login`).
fn is_login_page(url: &reqwest::Url) -> bool {
    let path = url.path().trim_end_matches('/');
    path == "/login" || path.starts_with("/login/")
}

fn not_signed_in() -> eyre::Report {
    http::login_rejected("Humble Bundle did not accept the saved login. Sign in again.")
}

/// The session cookie from a webview login, or the value the user pasted.
fn session_cookie_from_capture(capture: &LoginCapture) -> eyre::Result<String> {
    if let Some(value) = session_cookie_from_cookies(&capture.cookies) {
        return Ok(value);
    }
    if let Some(value) = capture.pasted.as_deref().and_then(parse_pasted_cookie) {
        return Ok(value);
    }
    if capture.pasted.is_some() {
        return Err(eyre::format_err!(
            "That is not a Humble Bundle session cookie. Paste the value of the {SESSION_COOKIE} cookie."
        ));
    }
    Err(eyre::format_err!(
        "The Humble Bundle login did not finish: no {SESSION_COOKIE} cookie was found. Try signing in again."
    ))
}

fn session_cookie_from_cookies(cookies: &[Cookie]) -> Option<String> {
    cookies
        .iter()
        .filter(|c| c.name == SESSION_COOKIE && http::domain_matches(&c.domain, COOKIE_HOST))
        .map(|c| c.value.trim())
        .find(|v| is_usable_cookie_value(v))
        .map(str::to_string)
}

/// Accepts the bare value, or `_simpleauth_sess=value` as copied from a cookie header.
fn parse_pasted_cookie(pasted: &str) -> Option<String> {
    let pasted = pasted.trim();
    let value = match pasted.split_once('=') {
        Some((name, rest)) if name.trim() == SESSION_COOKIE => rest,
        _ => pasted,
    };
    let value = value.split(';').next().unwrap_or_default().trim();
    is_usable_cookie_value(value).then(|| value.to_string())
}

fn is_usable_cookie_value(value: &str) -> bool {
    let unquoted = value.trim_matches('"');
    !unquoted.is_empty() && !value.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// A renewed `_simpleauth_sess` from `Set-Cookie` headers. Deletions (empty values) are ignored.
fn renewed_session_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .rev()
        .filter_map(|h| h.to_str().ok())
        .find_map(|h| {
            let (name, rest) = h.split_once('=')?;
            if name.trim() != SESSION_COOKIE {
                return None;
            }
            let value = rest.split(';').next()?.trim();
            is_usable_cookie_value(value).then(|| value.to_string())
        })
}

#[derive(Deserialize)]
struct OrderListEntry {
    gamekey: Option<String>,
}

/// Parses `api/v1/user/order`: `[{"gamekey": "..."}, ...]`.
fn parse_gamekeys(text: &str) -> eyre::Result<Vec<String>> {
    if looks_like_html(text) {
        return Err(not_signed_in());
    }
    let entries: Vec<Option<OrderListEntry>> = serde_json::from_str(text).map_err(|err| {
        eyre::format_err!(
            "Unexpected order list from Humble Bundle: {err}. Body starts with: {}",
            http::snippet(text)
        )
    })?;
    Ok(entries
        .into_iter()
        .flatten()
        .filter_map(|e| e.gamekey)
        .filter(|k| !k.is_empty())
        .collect())
}

#[derive(Deserialize)]
struct UserHome {
    #[serde(default, deserialize_with = "null_default")]
    gamekeys: Vec<String>,
}

/// The order keys in the library page's `<script id="user-home-json-data">` (what Playnite
/// reads). `None` when the page has no such data, which means the user is not signed in.
fn parse_library_page_gamekeys(html: &str) -> Option<Vec<String>> {
    let marker = html.find("id=\"user-home-json-data\"")?;
    let after_marker = html.get(marker..)?;
    let start = after_marker.find('>')? + 1;
    let body = after_marker.get(start..)?;
    let end = body.find("</script>")?;
    let home: UserHome = serde_json::from_str(body.get(..end)?.trim()).ok()?;
    Some(home.gamekeys)
}

fn looks_like_html(text: &str) -> bool {
    text.trim_start().starts_with('<')
}

/// No `Debug`: an order's key opens its download page.
#[derive(Default, Deserialize)]
struct Order {
    #[serde(default)]
    gamekey: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    subproducts: Vec<SubProduct>,
    #[serde(default)]
    tpkd_dict: Option<TpkdDict>,
}

#[derive(Debug, Default, Deserialize)]
struct SubProduct {
    #[serde(default)]
    machine_name: Option<String>,
    #[serde(default)]
    human_name: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    downloads: Vec<Download>,
}

#[derive(Debug, Default, Deserialize)]
struct Download {
    #[serde(default)]
    platform: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct TpkdDict {
    #[serde(default, deserialize_with = "null_default")]
    all_tpks: Vec<Tpk>,
}

/// A third-party key (Steam, GOG, Origin, ...) that came with an order.
#[derive(Debug, Default, Deserialize)]
struct Tpk {
    #[serde(default)]
    human_name: Option<String>,
}

/// Parses `api/v1/orders`: an object from order key to order. Some accounts get `null`
/// orders (Playnite filters them out too).
fn parse_orders(text: &str) -> eyre::Result<Vec<Order>> {
    if looks_like_html(text) {
        return Err(not_signed_in());
    }
    let orders: std::collections::BTreeMap<String, Option<Order>> = serde_json::from_str(text)
        .map_err(|err| {
            eyre::format_err!(
                "Unexpected orders from Humble Bundle: {err}. Body starts with: {}",
                http::snippet(text)
            )
        })?;
    Ok(orders.into_values().flatten().collect())
}

/// The games in the orders, following Playnite's `GetLibraryGames` with its default settings.
///
/// A title in several orders is listed once, under the `machine_name` of one of its copies
/// ([`listed_copy`]). `previous` is the last list of this account.
fn owned_games(orders: &[Order], previous: &[OwnedGame]) -> Vec<OwnedGame> {
    // Per title, every copy the account can download.
    let mut by_title: BTreeMap<&str, BTreeSet<TitleCopy<'_>>> = BTreeMap::new();
    for order in orders {
        let Some(gamekey) = order.gamekey.as_deref().filter(|k| !k.is_empty()) else {
            continue;
        };
        // Like Playnite, only the keys of the same order hide a DRM-free game.
        let key_names: Vec<&str> = order
            .tpkd_dict
            .iter()
            .flat_map(|t| &t.all_tpks)
            .filter_map(|t| t.human_name.as_deref())
            .collect();
        for product in &order.subproducts {
            let (Some(machine_name), Some(human_name)) = (
                product.machine_name.as_deref(),
                product.human_name.as_deref(),
            ) else {
                continue;
            };
            if machine_name.is_empty() {
                continue;
            }
            if !runs_here(product) || key_names.iter().any(|k| is_key_for(k, human_name)) {
                continue;
            }
            by_title
                .entry(human_name)
                .or_default()
                .insert((machine_name, gamekey));
        }
    }
    by_title
        .into_iter()
        .filter_map(|(human_name, copies)| {
            let (machine_name, gamekey) = listed_copy(&copies, previous)?;
            let url = format!("https://www.humblebundle.com/downloads?key={gamekey}");
            let title = clean_title(human_name);
            Some(OwnedGame::new(StoreId::Humble, machine_name, title).with_extra("url", url))
        })
        .collect()
}

/// A copy of a title: its `machine_name` and the key of the order it came with.
type TitleCopy<'a> = (&'a str, &'a str);

/// Whether a subproduct has a download for a platform BoilR runs on ([`GAME_PLATFORMS`]).
fn runs_here(product: &SubProduct) -> bool {
    product
        .downloads
        .iter()
        .filter_map(|d| d.platform.as_deref())
        .any(|p| GAME_PLATFORMS.contains(&p))
}

/// The copy a title is listed with; its `machine_name` is the game's id. That is the id the
/// previous list used, as long as the account still has that copy: the id is what ties the
/// game to its Steam shortcut, so an order bought later must not change it, whatever its
/// name. A title that is new to the list takes the smallest `machine_name`, which does not
/// depend on the order the orders arrive in.
fn listed_copy<'a>(
    copies: &BTreeSet<TitleCopy<'a>>,
    previous: &[OwnedGame],
) -> Option<TitleCopy<'a>> {
    copies
        .iter()
        .find(|(machine_name, _)| previous.iter().any(|game| game.id == *machine_name))
        .or_else(|| copies.first())
        .copied()
}

/// Words that may follow a game's name in the name of its third-party key.
const KEY_NAME_WORDS: &[&str] = &[
    "key",
    "keys",
    "steam",
    "epic",
    "games",
    "store",
    "gog",
    "origin",
    "ea",
    "app",
    "uplay",
    "ubisoft",
    "connect",
    "battle",
    "net",
    "rockstar",
    "launcher",
    "microsoft",
    "xbox",
    "pc",
    "drm",
    "free",
];

/// Whether a third-party key's name is for the game `title`: the same name, or the name
/// followed only by a store or "key" ("FEZ Steam Key", "FEZ (Steam)", "FEZ - Epic Games
/// Key"), the patterns of Playnite's HumbleLibrary. A key for another game whose name merely
/// contains the title ("FEZ II Steam Key", "Limbonauts") does not count.
fn is_key_for(key_name: &str, title: &str) -> bool {
    let normalize = |name: &str| clean_title(name).to_lowercase();
    let (key_name, title) = (normalize(key_name), normalize(title));
    if title.is_empty() {
        return false;
    }
    let Some(rest) = key_name.strip_prefix(&title) else {
        return false;
    };
    if rest.starts_with(char::is_alphanumeric) {
        return false;
    }
    rest.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .all(|word| KEY_NAME_WORDS.contains(&word))
}

/// Treats an explicit JSON `null` like a missing field.
fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDERS: &str = include_str!("../testdata/humble/orders.json");
    const ORDER_LIST: &str = include_str!("../testdata/humble/user_order.json");
    const LIBRARY_PAGE: &str = include_str!("../testdata/humble/library_page.html");

    fn ids(games: &[OwnedGame]) -> Vec<&str> {
        games.iter().map(|g| g.id.as_str()).collect()
    }

    /// Compiles only when the future `fetch` returns is `Send`.
    fn assert_fetch_is_send<'a, F, Fut>(_fetch: F)
    where
        F: Fn(&'a Client, &'a mut Session, &'a mut Fetch<'a>) -> Fut,
        Fut: Send,
    {
    }

    #[test]
    fn a_refresh_can_run_on_a_multi_threaded_runtime() {
        // The Tauri app runs refreshes on one, which takes `Send` futures only.
        assert_fetch_is_send(fetch_owned);
    }

    #[test]
    fn login_start_page_is_not_a_done_page() {
        let pending = begin_login();
        assert!(!pending.done_url_prefixes.is_empty());
        for prefix in &pending.done_url_prefixes {
            assert!(!pending.start_url.starts_with(prefix.as_str()));
            assert!(
                !"https://www.humblebundle.com/login?goto=%2Fhome%2Flibrary&qs=x"
                    .starts_with(prefix.as_str())
            );
        }
        assert!(LIBRARY_URL.starts_with(DONE_URL_PREFIX));
        assert!(
            "https://www.humblebundle.com/home/library?hmb_source=navbar"
                .starts_with(DONE_URL_PREFIX)
        );
    }

    #[test]
    fn reads_gamekeys_from_order_list() -> eyre::Result<()> {
        assert_eq!(
            parse_gamekeys(ORDER_LIST)?,
            vec!["aAbBcC123", "dDeEfF456", "gGhHiI789"]
        );
        Ok(())
    }

    #[test]
    fn html_instead_of_order_list_means_signed_out() {
        let err = parse_gamekeys("<!DOCTYPE html><html>login</html>").err();
        assert!(err.is_some_and(|e| {
            http::is_login_rejected(&e) && e.to_string().contains("Sign in again")
        }));
        let orders = parse_orders("<html>login</html>").err();
        assert!(orders.is_some_and(|e| http::is_login_rejected(&e)));
        // An answer that is merely not what was expected says nothing about the login.
        let odd = parse_gamekeys("{\"unexpected\": true}").err();
        assert!(odd.is_some_and(|e| !http::is_login_rejected(&e)));
    }

    #[test]
    fn only_the_login_page_itself_is_a_bounce() -> eyre::Result<()> {
        let bounced =
            |url: &str| -> eyre::Result<bool> { Ok(is_login_page(&reqwest::Url::parse(url)?)) };
        assert!(bounced(LOGIN_URL)?);
        assert!(bounced("https://www.humblebundle.com/login")?);
        assert!(bounced("https://www.humblebundle.com/login/?goto=%2Fhome")?);
        assert!(bounced("https://www.humblebundle.com/login/captcha")?);
        // The query of another page may name the login page.
        assert!(!bounced(
            "https://www.humblebundle.com/home/library?goto=/login"
        )?);
        assert!(!bounced(
            "https://www.humblebundle.com/api/v1/user/order?ajax=true&next=%2Flogin"
        )?);
        assert!(!bounced("https://www.humblebundle.com/loginhelp")?);
        assert!(!bounced("https://www.humblebundle.com/home/login")?);
        assert!(!bounced(LIBRARY_URL)?);
        Ok(())
    }

    #[test]
    fn the_library_page_is_only_tried_after_an_unexpected_answer() {
        let status = |status: u16| {
            eyre::Report::new(http::HttpStatus {
                status,
                url: ORDER_LIST_URL.into(),
                body: String::new(),
            })
        };
        // The API answered, but not with a list of orders.
        assert!(library_page_may_help(&eyre::format_err!(
            "Unexpected order list"
        )));
        assert!(library_page_may_help(&status(404)));
        // Offline, an outage, a rate limit: the page would only add another wait.
        assert!(!library_page_may_help(&http::transient("no connection")));
        assert!(!library_page_may_help(&status(503)));
        assert!(!library_page_may_help(&status(429)));
        // A refused login is refused on the page too.
        assert!(!library_page_may_help(&status(401)));
        assert!(!library_page_may_help(&not_signed_in()));
    }

    #[test]
    fn reads_the_saved_session() -> eyre::Result<()> {
        let saved = Session {
            account_name: None,
            data: serde_json::json!({ "simpleauth_sess": "\"abc|1\"" }),
        };
        let data: HumbleSession = session_data(StoreId::Humble, &saved)?;
        assert_eq!(data.simpleauth_sess, "\"abc|1\"");
        let broken = Session {
            account_name: None,
            data: serde_json::json!({ "cookie": "x" }),
        };
        let unreadable = session_data::<HumbleSession>(StoreId::Humble, &broken);
        assert!(unreadable.is_err_and(|e| http::is_login_rejected(&e)));
        Ok(())
    }

    #[test]
    fn reads_gamekeys_from_library_page() {
        assert_eq!(
            parse_library_page_gamekeys(LIBRARY_PAGE),
            Some(vec!["aAbBcC123".to_string(), "dDeEfF456".to_string()])
        );
        assert_eq!(parse_library_page_gamekeys("<html>Log in</html>"), None);
    }

    #[test]
    fn lists_drm_free_games_and_skips_keys_and_non_games() -> eyre::Result<()> {
        let games = owned_games(&parse_orders(ORDERS)?, &[]);
        // Sorted by title: Another Game, FEZ, World of Goo (Linux only; Playnite only
        // takes Windows).
        assert_eq!(
            ids(&games),
            vec!["anothergame_store", "fez_indiebundle", "worldofgoo_linux"]
        );

        let fez = games.get(1);
        assert_eq!(fez.map(|g| g.title.as_str()), Some("FEZ"));
        assert_eq!(
            fez.and_then(|g| g.extra("url")),
            Some("https://www.humblebundle.com/downloads?key=aAbBcC123")
        );
        // Only the extra the install target reads is written.
        let extras: Vec<&str> = fez
            .map(|g| g.extra.keys().map(String::as_str).collect())
            .unwrap_or_default();
        assert_eq!(extras, ["url"]);
        Ok(())
    }

    #[test]
    fn same_game_in_two_orders_is_listed_once() -> eyre::Result<()> {
        let games = owned_games(&parse_orders(ORDERS)?, &[]);
        assert_eq!(games.iter().filter(|g| g.title == "FEZ").count(), 1);
        Ok(())
    }

    fn order(gamekey: &str, games: &[(&str, &str)], keys: &[&str]) -> Order {
        order_for(&["windows"], gamekey, games, keys)
    }

    /// An order whose games each have a download for every one of `platforms`.
    fn order_for(
        platforms: &[&str],
        gamekey: &str,
        games: &[(&str, &str)],
        keys: &[&str],
    ) -> Order {
        Order {
            gamekey: Some(gamekey.into()),
            subproducts: games
                .iter()
                .map(|(machine_name, human_name)| SubProduct {
                    machine_name: Some((*machine_name).into()),
                    human_name: Some((*human_name).into()),
                    downloads: platforms
                        .iter()
                        .map(|platform| Download {
                            platform: Some((*platform).into()),
                        })
                        .collect(),
                })
                .collect(),
            tpkd_dict: Some(TpkdDict {
                all_tpks: keys
                    .iter()
                    .map(|name| Tpk {
                        human_name: Some((*name).into()),
                    })
                    .collect(),
            }),
        }
    }

    #[test]
    fn keys_only_hide_their_own_game_in_their_own_order() {
        let orders = [
            order("b", &[("limbo", "Limbo"), ("fez", "FEZ")], &[]),
            order(
                "a",
                &[("other", "Other")],
                &["Limbo Steam Key", "FEZ II Steam Key"],
            ),
            order(
                "c",
                &[("hue", "Hue"), ("braid", "Braid")],
                &["Huey Steam Key", "Braid (Steam)"],
            ),
        ];
        let games = owned_games(&orders, &[]);
        assert_eq!(ids(&games), vec!["fez", "hue", "limbo", "other"]);
    }

    #[test]
    fn games_without_a_windows_or_linux_download_are_not_listed() {
        let orders = [
            order_for(&["mac"], "a", &[("mac_only", "Mac Only")], &[]),
            order_for(
                &["mac", "windows"],
                "b",
                &[("both", "Mac And Windows")],
                &[],
            ),
            order_for(&["linux"], "c", &[("penguin", "Linux Only")], &[]),
            order_for(
                &["android", "audio", "ebook"],
                "d",
                &[("other", "No PC")],
                &[],
            ),
            order_for(&[], "e", &[("nothing", "No Download")], &[]),
            // A title with a Mac copy in one order and a Windows copy in another is listed
            // with the copy that runs here.
            order_for(&["mac"], "f", &[("split_a_mac", "Split")], &[]),
            order_for(&["windows"], "g", &[("split_b_windows", "Split")], &[]),
        ];
        let games = owned_games(&orders, &[]);
        assert_eq!(ids(&games), vec!["penguin", "both", "split_b_windows"]);
    }

    #[test]
    fn a_new_order_does_not_change_a_games_id() {
        let bought_first = order("m", &[("game_b", "Game")], &[]);
        let first = owned_games(std::slice::from_ref(&bought_first), &[]);
        assert_eq!(ids(&first), vec!["game_b"]);

        // A later order with a larger name, and one with a smaller name: the id stays.
        let larger = [
            order("a", &[("game_c", "Game")], &[]),
            order("m", &[("game_b", "Game")], &[]),
        ];
        assert_eq!(ids(&owned_games(&larger, &first)), vec!["game_b"]);
        let smaller = [
            order("z", &[("game_a", "Game")], &[]),
            order("m", &[("game_b", "Game")], &[]),
        ];
        let second = owned_games(&smaller, &first);
        assert_eq!(ids(&second), vec!["game_b"]);
        // The game still points at the order it was listed with.
        assert_eq!(
            second.first().and_then(|g| g.extra("url")),
            Some("https://www.humblebundle.com/downloads?key=m")
        );
        // And so on, list after list.
        assert_eq!(ids(&owned_games(&smaller, &second)), vec!["game_b"]);

        // Listed for the first time, a title takes its smallest name, whatever the order
        // of the orders.
        assert_eq!(ids(&owned_games(&smaller, &[])), vec!["game_a"]);
        let reversed = [
            order("m", &[("game_b", "Game")], &[]),
            order("z", &[("game_a", "Game")], &[]),
        ];
        assert_eq!(ids(&owned_games(&reversed, &[])), vec!["game_a"]);

        // The copy the list used is gone (refunded): the id moves to a copy that is left.
        let left = [
            order("z", &[("game_c", "Game")], &[]),
            order("y", &[("game_a", "Game")], &[]),
        ];
        assert_eq!(ids(&owned_games(&left, &first)), vec!["game_a"]);
        // Another game's id in the previous list changes nothing.
        let other = [OwnedGame::new(StoreId::Humble, "unrelated", "Unrelated")];
        assert_eq!(ids(&owned_games(&smaller, &other)), vec!["game_a"]);
    }

    #[test]
    fn matches_key_names_like_playnite() {
        assert!(is_key_for("FEZ", "FEZ"));
        assert!(is_key_for("FEZ Steam Key", "FEZ"));
        assert!(is_key_for("Fez (Steam)", "FEZ"));
        assert!(is_key_for("FEZ™ - Epic Games Key", "FEZ"));
        assert!(!is_key_for("FEZ II Steam Key", "FEZ"));
        assert!(!is_key_for("Limbonauts Steam Key", "Limbo"));
        assert!(!is_key_for("Chuchel Steam Key", "Hue"));
        assert!(!is_key_for("Anything", ""));
    }

    #[test]
    fn reads_pasted_cookie() {
        assert_eq!(
            parse_pasted_cookie("  \"abc|123|def\"\n"),
            Some("\"abc|123|def\"".into())
        );
        assert_eq!(
            parse_pasted_cookie("_simpleauth_sess=\"abc|123\"; Path=/"),
            Some("\"abc|123\"".into())
        );
        assert_eq!(parse_pasted_cookie(""), None);
        assert_eq!(parse_pasted_cookie("\"\""), None);
        assert_eq!(parse_pasted_cookie("two words"), None);
    }

    #[test]
    fn prefers_webview_cookie() -> eyre::Result<()> {
        let capture = LoginCapture {
            page_text: None,
            final_url: Some("https://www.humblebundle.com/home/library".into()),
            cookies: vec![
                Cookie {
                    name: "csrf_cookie".into(),
                    value: "x".into(),
                    domain: "www.humblebundle.com".into(),
                },
                Cookie {
                    name: SESSION_COOKIE.into(),
                    value: "\"sess\"".into(),
                    domain: ".humblebundle.com".into(),
                },
            ],
            pasted: None,
        };
        assert_eq!(session_cookie_from_capture(&capture)?, "\"sess\"");
        assert!(session_cookie_from_capture(&LoginCapture::default()).is_err());
        Ok(())
    }

    #[test]
    fn picks_up_renewed_cookie() -> eyre::Result<()> {
        let mut headers = HeaderMap::new();
        headers.append(
            reqwest::header::SET_COOKIE,
            "csrf_cookie=abc; Path=/".parse()?,
        );
        headers.append(
            reqwest::header::SET_COOKIE,
            "_simpleauth_sess=\"new|1\"; Domain=.humblebundle.com; Path=/; HttpOnly".parse()?,
        );
        assert_eq!(renewed_session_cookie(&headers), Some("\"new|1\"".into()));

        let mut deleted = HeaderMap::new();
        deleted.append(
            reqwest::header::SET_COOKIE,
            "_simpleauth_sess=\"\"; expires=Thu, 01 Jan 1970 00:00:00 GMT".parse()?,
        );
        assert_eq!(renewed_session_cookie(&deleted), None);
        Ok(())
    }
}
