//! GOG: sign in with the OAuth flow of the GOG Galaxy client and list the owned games.
//!
//! Login: the window opens `auth.gog.com/auth` for the Galaxy client id; after sign-in GOG
//! redirects to `embed.gog.com/on_login_success?origin=client&code=...`. The code is
//! exchanged at `auth.gog.com/token` for an access token (valid about an hour), a refresh
//! token and the user id. Same client id, secret and redirect as Heroic's gogdl
//! (`gogdl/auth.py`) and Minigalaxy (`minigalaxy/api.py`).
//!
//! Library: `embed.gog.com/account/getFilteredProducts?mediaType=1&page=N` with the Bearer
//! token (what Minigalaxy uses; Playnite reads the same endpoint on `www.gog.com` with web
//! cookies). `mediaType=1` leaves out movies; DLC is not listed on its own, except DLC whose
//! base game the account does not own (`isBaseProductMissing`), which is skipped.
//!
//! [`OwnedGame::extra`] keys:
//! - `slug`: the store slug (`https://www.gog.com/game/<slug>`), when GOG sends one.

use eyre::WrapErr;
use reqwest::Client;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    clean_title, http, now_unix, session_data, Fetch, LoginCapture, ManualLogin, OwnedGame,
    PendingLogin, Session, StoreId,
};

/// The GOG Galaxy client's OAuth client. Public: shipped in Galaxy and used by Heroic's
/// gogdl and Minigalaxy.
const CLIENT_ID: &str = "46899977096215655";
const CLIENT_SECRET: &str = "9d85c43b1482497dbbce61f6e4aa173a433796eeae2ca8c5f6129f2dc4de46d9";
const REDIRECT_URI: &str = "https://embed.gog.com/on_login_success?origin=client";
/// Where GOG sends the browser after sign-in. The login pages themselves live on
/// `auth.gog.com` and `login.gog.com`, so they never match.
const DONE_PREFIX: &str = "https://embed.gog.com/on_login_success";
const AUTH_URL: &str = "https://auth.gog.com/auth";
const TOKEN_URL: &str = "https://auth.gog.com/token";
const PRODUCTS_URL: &str = "https://embed.gog.com/account/getFilteredProducts";

/// Refresh the access token when it expires within this many seconds.
const REFRESH_MARGIN_SECS: u64 = 120;
/// More pages than this is taken as a server that keeps reporting more pages: an error, not
/// the end of the list.
const MAX_PAGES: usize = 500;

/// Products GOG files as games that are only goodies (comics, artbooks, wallpapers).
/// List from Minigalaxy (`minigalaxy/constants.py`, `IGNORE_GAME_IDS`).
const NOT_GAMES: [&str; 6] = [
    "1424856371", // Hotline Miami 2: Wrong Number - Digital Comics
    "1980301910", // The Witcher Goodies Collection
    "2005648906", // Spring Sale Goodies Collection #1
    "1486144755", // Cyberpunk 2077 Goodies Collection
    "1581684020", // A Plague Tale Digital Goodies Pack
    "1185685769", // CDPR Goodie Pack Content
];

/// What BoilR keeps in [`Session::data`] for GOG.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GogSession {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: String,
    /// Unix time in seconds when `access_token` stops working.
    pub expires_at: u64,
}

// The tokens stay out of `Debug` output, so a stray `{:?}` cannot put them in a log.
impl std::fmt::Debug for GogSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GogSession")
            .field("access_token", &"<hidden>")
            .field("refresh_token", &"<hidden>")
            .field("user_id", &self.user_id)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Response of `auth.gog.com/token` (for both the code and the refresh grant). It holds
/// tokens, so it has no `Debug`.
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    #[serde(deserialize_with = "string_or_number")]
    user_id: String,
    expires_in: u64,
}

/// Response of `users.gog.com/users/<id>`; only the name is used.
#[derive(Debug, Deserialize)]
struct UserResponse {
    username: Option<String>,
}

/// One page of `account/getFilteredProducts`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProductsPage {
    /// 0 when GOG does not say.
    #[serde(default)]
    total_pages: u32,
    /// Required: an answer without it is not a page of the library, and must not be read as
    /// an empty one.
    products: Vec<Product>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Product {
    #[serde(deserialize_with = "string_or_number")]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    is_game: Option<bool>,
    #[serde(default)]
    is_movie: Option<bool>,
    #[serde(default)]
    is_base_product_missing: Option<bool>,
}

pub fn begin_login() -> PendingLogin {
    let start_url = login_url();
    PendingLogin {
        page_text_url_prefixes: vec![],
        store: StoreId::Gog,
        start_url: start_url.clone(),
        done_url_prefixes: vec![DONE_PREFIX.to_string()],
        cookie_urls: vec![],
        manual: ManualLogin::Paste {
            url: start_url,
            instructions: "Sign in to GOG in your browser. When the page turns blank and its \
                           address starts with embed.gog.com/on_login_success, copy the whole \
                           address from the address bar and paste it here."
                .to_string(),
            placeholder: "https://embed.gog.com/on_login_success?origin=client&code=..."
                .to_string(),
        },
        user_agent: None,
        state: serde_json::Value::Null,
    }
}

fn login_url() -> String {
    let mut url = String::from(AUTH_URL);
    url.push('?');
    url.push_str(
        &url::form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", CLIENT_ID)
            .append_pair("redirect_uri", REDIRECT_URI)
            .append_pair("response_type", "code")
            .append_pair("layout", "galaxy")
            .finish(),
    );
    url
}

pub async fn complete_login(
    client: &Client,
    _pending: &PendingLogin,
    capture: &LoginCapture,
) -> eyre::Result<Session> {
    let code = code_from_capture(capture)?;
    let token: TokenResponse = http::send_json(client.get(TOKEN_URL).query(&[
        ("client_id", CLIENT_ID),
        ("client_secret", CLIENT_SECRET),
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", REDIRECT_URI),
    ]))
    .await
    .wrap_err("Could not finish the GOG sign-in")?;
    let gog = session_from_token(token, now_unix());
    // The name is only for display; a failure here should not fail the login.
    let account_name = fetch_username(client, &gog).await.ok().flatten();
    Ok(Session {
        account_name,
        data: serde_json::to_value(&gog)?,
    })
}

pub async fn fetch_owned(
    client: &Client,
    session: &mut Session,
    fetch: &mut Fetch<'_>,
) -> eyre::Result<Vec<OwnedGame>> {
    let mut gog: GogSession = session_data(StoreId::Gog, session)?;
    let mut renewed = false;
    if needs_refresh(&gog, now_unix()) {
        gog = renew(client, &gog, session, fetch).await?;
        renewed = true;
    }
    if session.account_name.is_none() {
        session.account_name = fetch_username(client, &gog).await.ok().flatten();
    }
    let games = match fetch_products(client, &gog).await {
        // GOG can end a token before it expires: renew it once and try again.
        Err(err) if !renewed && http::is_login_rejected(&err) => {
            gog = renew(client, &gog, session, fetch).await?;
            fetch_products(client, &gog).await
        }
        result => result,
    };
    games.wrap_err("Could not load the GOG library")
}

/// Renews the tokens and saves them right away: GOG ends the old refresh token when it is
/// used, so the new one must not be lost when BoilR is closed while the games are listed.
async fn renew(
    client: &Client,
    gog: &GogSession,
    session: &mut Session,
    fetch: &mut Fetch<'_>,
) -> eyre::Result<GogSession> {
    let renewed = refresh(client, gog).await?;
    session.data = serde_json::to_value(&renewed)?;
    fetch.checkpoint(session)?;
    Ok(renewed)
}

async fn fetch_products(client: &Client, gog: &GogSession) -> eyre::Result<Vec<OwnedGame>> {
    http::paginate("GOG", 1u32, MAX_PAGES, |page| async move {
        let response = http::send(
            client
                .get(PRODUCTS_URL)
                .bearer_auth(&gog.access_token)
                .query(&[("mediaType", "1"), ("page", &page.to_string())]),
        )
        .await?;
        // Where the request ended up after redirects. Reading the body takes the response.
        let final_url = response.url().clone();
        let body = response.text().await.map_err(http::request_failed)?;
        let products = parse_products_page(&final_url, &body)?;
        Ok((games_from_page(&products), next_page(&products, page)))
    })
    .await
}

async fn refresh(client: &Client, gog: &GogSession) -> eyre::Result<GogSession> {
    let token: TokenResponse = http::send_json(client.get(TOKEN_URL).query(&[
        ("client_id", CLIENT_ID),
        ("client_secret", CLIENT_SECRET),
        ("grant_type", "refresh_token"),
        ("refresh_token", gog.refresh_token.as_str()),
    ]))
    .await
    .map_err(refresh_failure)?;
    Ok(session_from_token(token, now_unix()))
}

/// Whether a failed token renewal means GOG refused the refresh token. The token service
/// answers a token it no longer accepts with HTTP 400 (`invalid_grant`) or 401. Anything
/// else (no connection, a timeout, a rate limit, 5xx) says nothing about the login.
fn refresh_was_refused(status: Option<u16>) -> bool {
    matches!(status, Some(400 | 401))
}

/// A failed token renewal as the error to report: a rejected login only when GOG refused
/// the refresh token, otherwise the error as it is, with context.
fn refresh_failure(err: eyre::Report) -> eyre::Report {
    if refresh_was_refused(http::status_of(&err)) {
        http::login_rejected(format!(
            "GOG no longer accepts the saved login. Sign in again. ({err})"
        ))
    } else {
        err.wrap_err("Could not renew the GOG login")
    }
}

async fn fetch_username(client: &Client, gog: &GogSession) -> eyre::Result<Option<String>> {
    let user: UserResponse = http::send_json(
        client
            .get(format!("https://users.gog.com/users/{}", gog.user_id))
            .bearer_auth(&gog.access_token),
    )
    .await?;
    Ok(user.username.filter(|name| !name.trim().is_empty()))
}

fn session_from_token(token: TokenResponse, now: u64) -> GogSession {
    GogSession {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        user_id: token.user_id,
        expires_at: now.saturating_add(token.expires_in),
    }
}

fn needs_refresh(gog: &GogSession, now: u64) -> bool {
    now.saturating_add(REFRESH_MARGIN_SECS) >= gog.expires_at
}

/// The authorization code from a webview redirect or from what the user pasted: the whole
/// redirect URL, its query string, or the bare code.
fn code_from_capture(capture: &LoginCapture) -> eyre::Result<String> {
    let input = capture
        .final_url
        .as_deref()
        .or(capture.pasted.as_deref())
        .map(str::trim)
        .unwrap_or_default();
    if input.is_empty() {
        return Err(eyre::format_err!(
            "Nothing to finish the GOG sign-in with. Paste the address of the page GOG shows after you sign in."
        ));
    }
    let query = match input.split_once('?') {
        Some((_, query)) => Some(query),
        None if input.contains('=') => Some(input),
        None => None,
    };
    let Some(query) = query else {
        if input
            .chars()
            .any(|c| c.is_whitespace() || c == '/' || c == ':')
        {
            return Err(eyre::format_err!(
                "That does not look like the GOG sign-in address. Paste the whole address that starts with {DONE_PREFIX}."
            ));
        }
        return Ok(input.to_string());
    };
    let query = query.split('#').next().unwrap_or_default();
    let param = |name: &str| {
        url::form_urlencoded::parse(query.as_bytes())
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
            .filter(|v| !v.is_empty())
    };
    if let Some(code) = param("code") {
        return Ok(code);
    }
    if let Some(error) = param("error") {
        let detail = param("error_description").unwrap_or(error);
        return Err(eyre::format_err!(
            "GOG did not complete the sign-in: {detail}"
        ));
    }
    Err(eyre::format_err!(
        "The GOG sign-in address has no code in it. Paste the whole address that starts with {DONE_PREFIX}."
    ))
}

/// Reads one page of the library. `final_url` is where the request ended up after redirects.
///
/// Without a valid token GOG sends its sign-in page instead of the library; only that is a
/// rejected login. Any other answer that is not a page of the library (a captive portal, a
/// maintenance page, JSON without `products`) is an error that says nothing about the login.
fn parse_products_page(final_url: &url::Url, body: &str) -> eyre::Result<ProductsPage> {
    if body.trim_start().starts_with('<') {
        if is_sign_in_page(final_url, body) {
            return Err(http::login_rejected(
                "GOG did not accept the saved login (it sent its sign-in page instead of the library). Sign in again.",
            ));
        }
        return Err(eyre::format_err!(
            "Unexpected answer from GOG: a web page from {} instead of the library. Body starts with: {}",
            http::without_query(final_url),
            http::snippet(body)
        ));
    }
    serde_json::from_str(body).map_err(|err| {
        eyre::format_err!(
            "Unexpected answer from GOG instead of the library: {err}. Body starts with: {}",
            http::snippet(body)
        )
    })
}

/// Whether a web page that came instead of the library is GOG's sign-in page.
///
/// It is when GOG itself served it (HTTPS on a `gog.com` host: a captive portal or a proxy's
/// error page is not) and it is a login page: the request was redirected to GOG's login
/// host, to a login path, or to the store with `#openlogin` (which opens the sign-in form),
/// or the page is nothing but a refresh that sends the browser there.
fn is_sign_in_page(final_url: &url::Url, body: &str) -> bool {
    let host = final_url
        .host_str()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if final_url.scheme() != "https" || !http::domain_matches("gog.com", &host) {
        return false;
    }
    let opens_login = |text: &str| text.to_ascii_lowercase().contains("openlogin");
    let body = body.to_ascii_lowercase();
    matches!(host.as_str(), "login.gog.com" | "auth.gog.com")
        || final_url.path().split('/').any(|segment| {
            matches!(
                segment.to_ascii_lowercase().as_str(),
                "login" | "signin" | "sign-in" | "sign_in"
            )
        })
        || final_url.fragment().is_some_and(opens_login)
        || (body.contains("http-equiv") && body.contains("refresh") && opens_login(&body))
}

fn games_from_page(page: &ProductsPage) -> Vec<OwnedGame> {
    page.products
        .iter()
        .filter(|p| is_owned_game(p))
        .map(|p| {
            let game = OwnedGame::new(StoreId::Gog, p.id.clone(), clean_title(&p.title));
            match p.slug.as_deref().map(str::trim) {
                Some(slug) if !slug.is_empty() => game.with_extra("slug", slug),
                _ => game,
            }
        })
        .collect()
}

fn is_owned_game(product: &Product) -> bool {
    !product.id.is_empty()
        && !clean_title(&product.title).is_empty()
        && product.is_movie != Some(true)
        && product.is_game != Some(false)
        && product.is_base_product_missing != Some(true)
        && !NOT_GAMES.contains(&product.id.as_str())
}

/// The page to ask for after page `current`, or `None` when that was the last one.
///
/// An empty page always ends the list. When GOG does not say how many pages there are
/// (`totalPages` missing or 0) but the page has products, the next page is asked for until
/// an empty one comes: stopping there could save a list cut short.
fn next_page(page: &ProductsPage, current: u32) -> Option<u32> {
    if page.products.is_empty() {
        return None;
    }
    (page.total_pages == 0 || current < page.total_pages).then_some(current.saturating_add(1))
}

/// GOG sends ids as numbers in some responses and as strings in others.
fn string_or_number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Id {
        Text(String),
        Number(u64),
    }
    Ok(match Id::deserialize(deserializer)? {
        Id::Text(text) => text,
        Id::Number(number) => number.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = include_str!("../testdata/gog/token.json");
    const PAGE_1: &str = include_str!("../testdata/gog/products_page1.json");
    const PAGE_2: &str = include_str!("../testdata/gog/products_page2.json");
    const USER: &str = include_str!("../testdata/gog/user.json");

    fn capture_url(url: &str) -> LoginCapture {
        LoginCapture {
            final_url: Some(url.to_string()),
            ..Default::default()
        }
    }

    fn capture_paste(text: &str) -> LoginCapture {
        LoginCapture {
            pasted: Some(text.to_string()),
            ..Default::default()
        }
    }

    /// Where a library request ends up when GOG answers it without a redirect.
    fn library_url() -> eyre::Result<url::Url> {
        Ok(url::Url::parse(&format!(
            "{PRODUCTS_URL}?mediaType=1&page=1"
        ))?)
    }

    fn status(status: u16) -> eyre::Report {
        eyre::Report::new(http::HttpStatus {
            status,
            url: TOKEN_URL.into(),
            body: r#"{"error":"invalid_grant"}"#.into(),
        })
    }

    #[test]
    fn login_window_starts_outside_the_done_prefix() {
        let pending = begin_login();
        assert!(pending.start_url.starts_with("https://auth.gog.com/auth?"));
        assert!(pending.start_url.contains("client_id=46899977096215655"));
        assert!(pending.start_url.contains(
            "redirect_uri=https%3A%2F%2Fembed.gog.com%2Fon_login_success%3Forigin%3Dclient"
        ));
        for prefix in &pending.done_url_prefixes {
            assert!(!pending.start_url.starts_with(prefix));
            assert!(!"https://login.gog.com/login".starts_with(prefix));
            assert!(REDIRECT_URI.starts_with(prefix));
        }
        assert!(matches!(pending.manual, ManualLogin::Paste { .. }));
    }

    #[test]
    fn reads_the_code_from_the_redirect() -> eyre::Result<()> {
        let url = "https://embed.gog.com/on_login_success?origin=client&code=AbC-123_x";
        assert_eq!(code_from_capture(&capture_url(url))?, "AbC-123_x");
        assert_eq!(
            code_from_capture(&capture_paste(&format!("  {url}\n")))?,
            "AbC-123_x"
        );
        assert_eq!(
            code_from_capture(&capture_paste("origin=client&code=XYZ"))?,
            "XYZ"
        );
        assert_eq!(code_from_capture(&capture_paste("XYZ123"))?, "XYZ123");
        assert_eq!(
            code_from_capture(&capture_paste(
                "embed.gog.com/on_login_success?origin=client&code=Q#frag"
            ))?,
            "Q"
        );
        Ok(())
    }

    #[test]
    fn rejects_input_without_a_code() {
        assert!(code_from_capture(&LoginCapture::default()).is_err());
        assert!(code_from_capture(&capture_paste("https://www.gog.com/en/")).is_err());
        assert!(code_from_capture(&capture_paste("not a code")).is_err());
        let denied = code_from_capture(&capture_url(
            "https://embed.gog.com/on_login_success?origin=client&error=access_denied&error_description=User+cancelled",
        ));
        assert!(denied.is_err_and(|e| e.to_string().contains("User cancelled")));
    }

    #[test]
    fn builds_a_session_from_the_token_response() -> eyre::Result<()> {
        let token: TokenResponse = serde_json::from_str(TOKEN)?;
        let gog = session_from_token(token, 1_000);
        assert_eq!(gog.access_token, "access-1");
        assert_eq!(gog.refresh_token, "refresh-1");
        assert_eq!(gog.user_id, "48628349957132247");
        assert_eq!(gog.expires_at, 4_600);
        assert!(!needs_refresh(&gog, 1_000));
        assert!(needs_refresh(&gog, 4_500));
        assert!(needs_refresh(&gog, 5_000));

        let session = Session {
            account_name: None,
            data: serde_json::to_value(&gog)?,
        };
        assert_eq!(session_data::<GogSession>(StoreId::Gog, &session)?, gog);
        Ok(())
    }

    #[test]
    fn debug_output_hides_the_tokens() -> eyre::Result<()> {
        let token: TokenResponse = serde_json::from_str(TOKEN)?;
        let shown = format!("{:?}", session_from_token(token, 1_000));
        assert!(!shown.contains("access-1"), "{shown}");
        assert!(!shown.contains("refresh-1"), "{shown}");
        assert!(shown.contains("48628349957132247"), "{shown}");
        Ok(())
    }

    #[test]
    fn numeric_user_id_is_accepted() -> eyre::Result<()> {
        let token: TokenResponse = serde_json::from_str(
            r#"{"access_token":"a","refresh_token":"r","user_id":42,"expires_in":3600}"#,
        )?;
        assert_eq!(token.user_id, "42");
        Ok(())
    }

    #[test]
    fn unreadable_session_asks_to_sign_in_again() {
        let session = Session {
            account_name: None,
            data: serde_json::json!({"cookies": []}),
        };
        let result = session_data::<GogSession>(StoreId::Gog, &session);
        assert!(result.is_err_and(|e| {
            http::is_login_rejected(&e) && e.to_string().contains("Sign in again")
        }));
    }

    #[test]
    fn only_a_refused_refresh_token_is_a_rejected_login() {
        // GOG answers a refresh token it no longer accepts with 400 `invalid_grant`.
        assert!(refresh_was_refused(Some(400)));
        assert!(refresh_was_refused(Some(401)));
        // Blocked, rate limited, down, or no answer at all: the login may be fine.
        for other in [403, 404, 408, 429, 500, 502, 503] {
            assert!(!refresh_was_refused(Some(other)), "{other}");
        }
        assert!(!refresh_was_refused(None));
    }

    #[test]
    fn a_failed_refresh_keeps_its_kind() {
        for refused in [400, 401] {
            let err = refresh_failure(status(refused));
            assert!(http::is_login_rejected(&err), "{refused}");
            assert!(err.to_string().contains("Sign in again"), "{err}");
        }
        // An outage stays an outage, with the status still there to read.
        let outage = refresh_failure(status(503));
        assert!(!http::is_login_rejected(&outage));
        assert!(http::is_transient(&outage));
        assert_eq!(http::status_of(&outage), Some(503));
        assert!(format!("{outage:#}").contains("Could not renew the GOG login"));
        // No connection: transient, and not a reason to sign in again.
        let offline = refresh_failure(http::transient("no connection"));
        assert!(http::is_transient(&offline) && !http::is_login_rejected(&offline));
        // A bot block is neither.
        let blocked = refresh_failure(status(403));
        assert!(!http::is_login_rejected(&blocked) && !http::is_transient(&blocked));
    }

    #[test]
    fn reads_the_username() -> eyre::Result<()> {
        let user: UserResponse = serde_json::from_str(USER)?;
        assert_eq!(user.username.as_deref(), Some("player_one"));
        Ok(())
    }

    #[test]
    fn keeps_games_and_skips_movies_dlc_and_goodies() -> eyre::Result<()> {
        let page: ProductsPage = serde_json::from_str(PAGE_1)?;
        let games = games_from_page(&page);
        let ids: Vec<&str> = games.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(ids, vec!["1207658924", "1207664663", "1423049311"]);

        let unreal = games.iter().find(|g| g.id == "1207658924");
        assert_eq!(unreal.map(|g| g.title.as_str()), Some("Unreal Gold"));
        assert_eq!(unreal.and_then(|g| g.extra("slug")), Some("unreal_gold"));
        assert!(games.iter().all(|g| g.store == StoreId::Gog));
        // No slug in the response: no `slug` key.
        let no_slug = games.iter().find(|g| g.id == "1423049311");
        assert_eq!(no_slug.and_then(|g| g.extra("slug")), None);
        Ok(())
    }

    #[test]
    fn titles_lose_trademark_signs_and_extra_spaces() -> eyre::Result<()> {
        let page: ProductsPage = serde_json::from_str(
            r#"{"totalPages":1,"products":[
                {"id":1,"title":"  The Witcher\u00ae 3:  Wild Hunt\u2122 "},
                {"id":2,"title":" \u2122 "}
            ]}"#,
        )?;
        let games = games_from_page(&page);
        let titles: Vec<&str> = games.iter().map(|g| g.title.as_str()).collect();
        // A title that is only a sign is no title.
        assert_eq!(titles, vec!["The Witcher 3: Wild Hunt"]);

        let first: ProductsPage = serde_json::from_str(PAGE_1)?;
        let witcher = games_from_page(&first)
            .into_iter()
            .find(|g| g.id == "1207664663");
        assert_eq!(
            witcher.map(|g| g.title),
            Some("The Witcher: Enhanced Edition".to_string())
        );
        Ok(())
    }

    #[test]
    fn follows_pages_until_the_last() -> eyre::Result<()> {
        let url = library_url()?;
        let first = parse_products_page(&url, PAGE_1)?;
        let second = parse_products_page(&url, PAGE_2)?;
        assert_eq!(next_page(&first, 1), Some(2));
        assert_eq!(next_page(&second, 2), None);
        assert_eq!(games_from_page(&second).len(), 1);

        let empty: ProductsPage =
            serde_json::from_str(r#"{"page":1,"totalPages":0,"products":[]}"#)?;
        assert_eq!(next_page(&empty, 1), None);
        assert!(games_from_page(&empty).is_empty());

        // A page that claims more pages but is empty stops the loop.
        let stuck: ProductsPage =
            serde_json::from_str(r#"{"page":3,"totalPages":9,"products":[]}"#)?;
        assert_eq!(next_page(&stuck, 3), None);
        Ok(())
    }

    #[test]
    fn asks_for_more_pages_while_gog_does_not_say_how_many_there_are() -> eyre::Result<()> {
        let url = library_url()?;
        // No `totalPages`, or 0, on a page with products: not the end of the list.
        let unknown = parse_products_page(&url, r#"{"products":[{"id":1,"title":"A"}]}"#)?;
        assert_eq!(next_page(&unknown, 1), Some(2));
        assert_eq!(next_page(&unknown, 7), Some(8));
        let zero = parse_products_page(
            &url,
            r#"{"totalPages":0,"products":[{"id":1,"title":"A"}]}"#,
        )?;
        assert_eq!(next_page(&zero, 3), Some(4));
        // The empty page that follows is the end.
        let end = parse_products_page(&url, r#"{"products":[]}"#)?;
        assert_eq!(next_page(&end, 2), None);
        // The page asked for decides, also when it is past what GOG reports.
        let first = parse_products_page(&url, PAGE_1)?;
        assert_eq!(next_page(&first, 2), None);
        assert_eq!(next_page(&first, u32::MAX), None);
        assert_eq!(next_page(&unknown, u32::MAX), Some(u32::MAX));
        Ok(())
    }

    #[test]
    fn an_answer_without_products_is_not_an_empty_library() -> eyre::Result<()> {
        let url = library_url()?;
        for body in [
            "{}",
            r#"{"page":1,"totalPages":3}"#,
            r#"{"error":"temporarily unavailable"}"#,
            r#"{"products": 3}"#,
            r#"{"products": null}"#,
            "[]",
            "",
        ] {
            let result = parse_products_page(&url, body);
            assert!(
                result.is_err_and(|e| {
                    e.to_string().contains("Unexpected answer")
                        && !http::is_login_rejected(&e)
                        && !http::is_transient(&e)
                }),
                "{body}"
            );
        }
        Ok(())
    }

    #[test]
    fn only_gogs_sign_in_page_asks_to_sign_in_again() -> eyre::Result<()> {
        let page = "<!DOCTYPE html>\n<html><body>Something else</body></html>";
        let rejected = |url: &str, body: &str| -> eyre::Result<bool> {
            let result = parse_products_page(&url::Url::parse(url)?, body);
            let Err(err) = result else {
                eyre::bail!("a web page was read as a library page");
            };
            Ok(http::is_login_rejected(&err))
        };

        // The request was redirected to GOG's login.
        for url in [
            "https://login.gog.com/auth?client_id=1&redirect_uri=x",
            "https://auth.gog.com/auth?client_id=1",
            "https://www.gog.com/en/login?next=x",
            "https://www.gog.com/##openlogin",
            "https://www.gog.com/en##openlogin",
        ] {
            assert!(rejected(url, page)?, "{url}");
        }
        // GOG answered with a page that opens its login.
        let opens_login = "<!DOCTYPE html>\n<html><head><meta http-equiv=\"refresh\" content=\"0;url='/en##openlogin'\" /></head></html>";
        assert!(rejected(library_url()?.as_str(), opens_login)?);

        // Any other page says nothing about the login: a maintenance page of GOG's, a
        // captive portal, a proxy's error page, also when it mentions a login.
        assert!(!rejected(library_url()?.as_str(), page)?);
        assert!(!rejected("https://www.gog.com/en/maintenance", page)?);
        let mentions_login =
            "<html><script>if (hash == 'openlogin') show()</script>Down for maintenance</html>";
        assert!(!rejected(
            "https://www.gog.com/en/maintenance",
            mentions_login
        )?);
        for url in [
            "http://hotel-wifi.example/login",
            "https://portal.example/login?next=https://embed.gog.com/",
            "https://notgog.com/login",
            "https://gog.com.example/login",
            "http://login.gog.com/login",
        ] {
            assert!(!rejected(url, page)?, "{url}");
            assert!(!rejected(url, opens_login)?, "{url}");
        }
        let err = parse_products_page(
            &url::Url::parse("https://portal.example/a?token=SECRET")?,
            page,
        )
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default();
        assert!(err.contains("Unexpected answer"), "{err}");
        assert!(err.contains("https://portal.example/a"), "{err}");
        assert!(!err.contains("SECRET"), "{err}");
        Ok(())
    }
}
