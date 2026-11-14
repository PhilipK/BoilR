//! Epic Games Store: signs in with the Epic Games Launcher's OAuth client and lists the
//! account's library through Epic's library and catalog services, the way Playnite's
//! EpicLibrary plugin and legendary do.
//!
//! `OwnedGame.id` is Epic's `appName`, the id the launcher's install manifests, legendary and
//! Heroic use (`src/platforms/egs/manifest_item.rs`, `src/platforms/heroic/`).
//!
//! `extra` keys:
//! - `namespace`: the catalog namespace.
//! - `catalog_item_id`: the catalog item id.
//! - `product_slug`: the game's page on the store (`store.epicgames.com/p/<product_slug>`),
//!   from the catalog's `com.epicgames.app.productSlug` attribute. Absent when the catalog
//!   item has none.
//!
//! `namespace`, `catalog_item_id` and `id` together form the launcher URI
//! `com.epicgames.launcher://apps/{namespace}%3A{catalog_item_id}%3A{id}?action=launch&silent=true`.
//!
//! A refresh asks the catalog only about library entries the last list does not hold: a game
//! that is already listed is taken over as it is. Entries that are not games (DLC, extras)
//! are never in the list, so they are looked up each time.

use std::collections::{BTreeMap, BTreeSet};

use eyre::WrapErr;
use reqwest::{
    header::{ACCEPT, AUTHORIZATION, COOKIE, USER_AGENT},
    Client, RequestBuilder,
};
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

use crate::{
    clean_title, http, now_unix, session_data, Fetch, LoginCapture, ManualLogin, OwnedGame,
    PendingLogin, Session, StoreId,
};

/// The Epic Games Launcher's OAuth client ("launcherAppClient2"), as used by Playnite
/// (`EpicAccountClient.cs`) and legendary (`api/egs.py`).
const CLIENT_ID: &str = "34a02cf8f4414e29b15921876da36f9a";
const CLIENT_SECRET: &str = "daafbccc737745039dffe53d94fc76cf";
/// legendary's launcher user agent for the account, library and catalog services.
const LAUNCHER_USER_AGENT: &str =
    "UELauncher/11.0.1-14907503+++Portal+Release-Live Windows/10.0.19041.1.256.64bit";
/// The user agent of the login window: an ordinary browser's, as the login works in a
/// browser. With the launcher's own (`... EpicGamesLauncher`, which Playnite used), Epic's
/// sign-in finishes as the launcher's and stops after two-factor authentication with
/// "Invalid client, redirect url is not known to the client". Whatever replays the window's
/// cookies sends the same one.
const LOGIN_USER_AGENT: &str = http::BROWSER_USER_AGENT;

const LOGIN_URL: &str = "https://www.epicgames.com/id/login";
const REDIRECT_URL: &str = "https://www.epicgames.com/id/api/redirect";
/// The same page without `www`.
const REDIRECT_URL_BARE: &str = "https://epicgames.com/id/api/redirect";
const OAUTH_TOKEN_URL: &str =
    "https://account-public-service-prod03.ol.epicgames.com/account/api/oauth/token";
const SESSION_KILL_URL: &str =
    "https://account-public-service-prod03.ol.epicgames.com/account/api/oauth/sessions/kill/";
const ACCOUNT_URL: &str =
    "https://account-public-service-prod03.ol.epicgames.com/account/api/public/account/";
const LIBRARY_ITEMS_URL: &str =
    "https://library-service.live.use1a.on.epicgames.com/library/api/public/items";
const CATALOG_URL: &str =
    "https://catalog-public-service-prod06.ol.epicgames.com/catalog/api/shared/namespace/";

/// Catalog item ids asked for in one bulk request (ids are 32 characters; this keeps URLs short).
const CATALOG_BATCH_SIZE: usize = 50;
/// Catalog requests in flight at once.
const CATALOG_CONCURRENCY: usize = 8;
/// A guard against a library service that keeps handing out cursors.
const MAX_LIBRARY_PAGES: usize = 1000;
/// Refresh the access token when it expires within this many seconds. Longer than a refresh
/// may take ([`crate::REFRESH_DEADLINE`]), so a token that passed this check lasts for the
/// whole fetch.
const REFRESH_MARGIN_SECS: u64 = 300;
/// The catalog attribute that names a game's store page (legendary and Playnite read it).
const PRODUCT_SLUG_ATTRIBUTE: &str = "com.epicgames.app.productSlug";

/// The page that hands out an authorization code for the launcher client once the user is
/// signed in at epicgames.com. It shows JSON with `authorizationCode`.
fn redirect_url() -> String {
    format!("{REDIRECT_URL}?clientId={CLIENT_ID}&responseType=code")
}

/// Epic's login page, sending the user on to [`redirect_url`] afterwards (legendary's
/// `get_auth_url`). A user who is already signed in goes straight to the code.
fn login_url() -> String {
    let redirect: String =
        url::form_urlencoded::byte_serialize(redirect_url().as_bytes()).collect();
    format!("{LOGIN_URL}?redirectUrl={redirect}")
}

pub fn begin_login() -> PendingLogin {
    let redirect_pages = vec![REDIRECT_URL.to_string(), REDIRECT_URL_BARE.to_string()];
    let mut done_url_prefixes = redirect_pages.clone();
    // Where Epic's login page sends the real launcher (Playnite, Heroic). The code is in the
    // `code` query parameter, so this page is not loaded.
    done_url_prefixes.extend([
        "http://localhost/launcher/authorized".to_string(),
        "https://localhost/launcher/authorized".to_string(),
    ]);
    PendingLogin {
        store: StoreId::Epic,
        start_url: login_url(),
        done_url_prefixes,
        // The redirect page is JSON that holds the code. The window loads it and hands over
        // its text, as Playnite and Heroic read it inside their webview: asking for the page
        // again from here comes from another user agent and TLS stack, which Epic's bot
        // protection may refuse.
        page_text_url_prefixes: redirect_pages,
        cookie_urls: vec!["https://www.epicgames.com".to_string()],
        manual: ManualLogin::Paste {
            url: login_url(),
            instructions: "Sign in to Epic Games in your browser. Epic then shows a page of text \
                           containing \"authorizationCode\". Copy the code after it (or all of \
                           the text) and paste it here. The code works for a few minutes only."
                .to_string(),
            placeholder: "authorizationCode".to_string(),
        },
        user_agent: Some(LOGIN_USER_AGENT.to_string()),
        state: serde_json::Value::Null,
    }
}

pub async fn complete_login(
    client: &Client,
    _pending: &PendingLogin,
    capture: &LoginCapture,
) -> eyre::Result<Session> {
    let pasted = capture.pasted.as_deref().filter(|p| !p.trim().is_empty());
    let code = match pasted {
        Some(pasted) => parse_pasted_code(pasted)?,
        None => match code_in_capture(capture) {
            Some(code) => code?,
            None => code_from_cookies(client, capture).await?,
        },
    };

    let tokens = request_token(
        client,
        Grant::AuthorizationCode,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("token_type", "eg1"),
        ],
    )
    .await?;
    let mut data = EpicSession::from_tokens(tokens, None, now_unix());
    if data.display_name.is_none() {
        // Only the name shown in the UI; a failure here should not fail the login.
        data.display_name = fetch_display_name(client, &data).await.ok().flatten();
    }
    Ok(Session {
        account_name: data.display_name.clone(),
        data: serde_json::to_value(&data)?,
    })
}

pub async fn fetch_owned(
    client: &Client,
    session: &mut Session,
    fetch: &mut Fetch<'_>,
) -> eyre::Result<Vec<OwnedGame>> {
    let mut data: EpicSession = session_data(StoreId::Epic, session)?;
    if data.needs_refresh(now_unix()) {
        renew(client, &mut data, session, fetch).await?;
    }
    let records = fetch_library(client, &mut data, session, fetch).await?;
    let entries = library_entries(records);

    let unknown = entries_to_look_up(&entries, fetch.previous);
    let lookup = fetch_catalog(client, &data.access_token, catalog_batches(unknown)).await;
    let listed = list_games(&entries, fetch.previous, &lookup);
    if listed.missing > 0 {
        let error = lookup
            .error
            .unwrap_or_else(|| eyre::format_err!("Epic's catalog did not answer"));
        if listed.games.is_empty() {
            return Err(error.wrap_err("Could not look up the Epic games"));
        }
        // The list is saved without them, marked as incomplete so it is fetched again soon.
        eprintln!(
            "{} Epic games could not be looked up and are left out for now: {error:#}",
            listed.missing
        );
        fetch.partial = true;
    }
    Ok(listed.games)
}

/// Renews the tokens and saves them right away: Epic ends the old refresh token on use, so
/// the new one must not be lost when BoilR is closed while the games are listed.
async fn renew(
    client: &Client,
    data: &mut EpicSession,
    session: &mut Session,
    fetch: &mut Fetch<'_>,
) -> eyre::Result<()> {
    refresh(client, data, session).await?;
    fetch.checkpoint(session)
}

/// Every record of the account's library, page by page.
async fn fetch_library(
    client: &Client,
    data: &mut EpicSession,
    session: &mut Session,
    fetch: &mut Fetch<'_>,
) -> eyre::Result<Vec<LibraryRecord>> {
    // The first page doubles as the token check: a token Epic revoked early is renewed once.
    let first = library_page(client, &data.access_token, None).await;
    let (mut records, next) = match first {
        Err(err) if http::is_login_rejected(&err) => {
            renew(client, data, session, fetch).await?;
            library_page(client, &data.access_token, None).await?
        }
        first => first?,
    };
    let Some(next) = next else {
        return Ok(records);
    };
    let access_token = data.access_token.as_str();
    let rest = http::paginate(
        "Epic's library service",
        next,
        MAX_LIBRARY_PAGES,
        |cursor| async move { library_page(client, access_token, Some(&cursor)).await },
    )
    .await?;
    records.extend(rest);
    Ok(records)
}

/// One page of the library: its records and the cursor of the next page, if there is one.
async fn library_page(
    client: &Client,
    access_token: &str,
    cursor: Option<&str>,
) -> eyre::Result<(Vec<LibraryRecord>, Option<String>)> {
    let page: LibraryPage = http::send_json(library_request(client, access_token, cursor))
        .await
        .wrap_err("Could not load the Epic library")?;
    Ok(page.into_parts())
}

// ---------------------------------------------------------------------------------------------
// Login

/// The JSON the redirect page shows.
#[derive(Deserialize)]
struct RedirectResponse {
    #[serde(rename = "authorizationCode", default)]
    authorization_code: Option<String>,
}

impl RedirectResponse {
    fn code(self) -> eyre::Result<String> {
        self.authorization_code
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .ok_or_else(no_code_without_sign_in)
    }
}

/// The redirect page answers with a `null` code to a browser that is not signed in.
fn no_code_without_sign_in() -> eyre::Report {
    eyre::format_err!(
        "Epic gave no login code because this browser is not signed in to epicgames.com. \
         Sign in, open the link again and paste what it shows."
    )
}

/// Reads the authorization code from the redirect page: its JSON, or what a JSON viewer
/// makes of it.
fn parse_redirect_response(body: &str) -> eyre::Result<String> {
    match serde_json::from_str::<RedirectResponse>(body.trim()) {
        Ok(response) => response.code(),
        Err(_) => code_after_name(body).unwrap_or_else(|| {
            Err(eyre::format_err!(
                "Epic did not return a login code. Body starts with: {}",
                http::snippet(body)
            ))
        }),
    }
}

/// The value that follows `authorizationCode` in text that is not JSON as a whole: a part of
/// the JSON (`"authorizationCode":"…",`), or what a browser's JSON viewer copies
/// (Firefox: `authorizationCode<TAB>"…"`). `None` when the text names no code.
fn code_after_name(text: &str) -> Option<eyre::Result<String>> {
    let mut signed_out = false;
    for rest in text.split("authorizationCode").skip(1) {
        let value = rest
            .split(|c: char| !c.is_ascii_alphanumeric())
            .find(|word| !word.is_empty());
        match value {
            Some(code) if looks_like_code(code) => return Some(Ok(code.to_string())),
            Some("null") => signed_out = true,
            _ => {}
        }
    }
    signed_out.then(|| Err(no_code_without_sign_in()))
}

/// Accepts what a user may paste: the bare code, the redirect page's JSON (whole, in part,
/// or as a browser's JSON viewer copies it), a `…/launcher/authorized?code=…` URL, or any
/// text that holds exactly one code.
fn parse_pasted_code(pasted: &str) -> eyre::Result<String> {
    let text = pasted.trim();
    if let Ok(response) = serde_json::from_str::<RedirectResponse>(text) {
        return response.code();
    }
    if let Some(found) = code_after_name(text) {
        return found;
    }
    if let Some(code) = http::query_param(text, "code").filter(|c| looks_like_code(c)) {
        return Ok(code);
    }
    let bare = text.trim_matches(|c| c == '"' || c == '\'').trim();
    if looks_like_code(bare) {
        return Ok(bare.to_string());
    }
    if let Some(code) = only_hex_code_in(text) {
        return Ok(code);
    }
    Err(eyre::format_err!(
        "That does not look like an Epic login code. Paste the value of \"authorizationCode\" \
         (32 letters and digits) or all of the text the Epic page shows."
    ))
}

/// What is accepted as a code where the text says it is one (after `authorizationCode`, in
/// a `code` parameter, pasted on its own). Epic's codes are 32 hex characters today; this
/// leaves room for another length.
fn looks_like_code(text: &str) -> bool {
    (16..=128).contains(&text.len()) && text.chars().all(|c| c.is_ascii_alphanumeric())
}

/// A code as Epic hands them out: 32 hex characters.
fn is_hex_code(text: &str) -> bool {
    text.len() == 32 && text.chars().all(|c| c.is_ascii_hexdigit())
}

/// The code in text that says nothing about where it is, when the text holds exactly one.
fn only_hex_code_in(text: &str) -> Option<String> {
    let codes: BTreeSet<&str> = text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| is_hex_code(word))
        .collect();
    let mut codes = codes.into_iter();
    match (codes.next(), codes.next()) {
        (Some(code), None) => Some(code.to_string()),
        _ => None,
    }
}

/// The code a login window handed over directly: in the URL of the launcher's localhost
/// callback, or in the text of the redirect page it loaded. `None` when it gave neither.
fn code_in_capture(capture: &LoginCapture) -> Option<eyre::Result<String>> {
    let in_url = capture
        .final_url
        .as_deref()
        .and_then(|url| http::query_param(url, "code"))
        .filter(|code| looks_like_code(code));
    if let Some(code) = in_url {
        return Some(Ok(code));
    }
    capture
        .page_text
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .map(parse_redirect_response)
}

/// For a login window that could not hand over the redirect page's text: asks for the page
/// with the window's cookies, under the window's user agent.
async fn code_from_cookies(client: &Client, capture: &LoginCapture) -> eyre::Result<String> {
    if capture.final_url.is_none() {
        return Err(eyre::format_err!(
            "The Epic login did not finish. Sign in again."
        ));
    }
    let cookie = http::cookie_header(&capture.cookies, "www.epicgames.com");
    if cookie.is_empty() {
        return Err(eyre::format_err!(
            "The login window returned no Epic cookies. Sign in again."
        ));
    }
    // Always our own URL, so the cookies only ever go to epicgames.com with our client id.
    let response = http::send(redirect_page_request(client, cookie)).await?;
    parse_redirect_response(&response.text().await.map_err(http::request_failed)?)
}

fn redirect_page_request(client: &Client, cookie: String) -> RequestBuilder {
    client
        .get(redirect_url())
        .header(COOKIE, cookie)
        .header(USER_AGENT, LOGIN_USER_AGENT)
        .header(ACCEPT, "application/json")
}

/// Tokens as Epic's OAuth service returns them.
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: u64,
    refresh_token: String,
    #[serde(default)]
    account_id: String,
    #[serde(rename = "displayName", default)]
    display_name: Option<String>,
}

/// Epic's error body, shared by all its services.
#[derive(Debug, Default, Deserialize)]
struct EpicError {
    #[serde(rename = "errorCode", default)]
    error_code: Option<String>,
    #[serde(rename = "errorMessage", default)]
    error_message: Option<String>,
    /// The OAuth name of the error (`invalid_grant`), which the token service sends next to
    /// its own `errorCode`.
    #[serde(default)]
    error: Option<String>,
    #[serde(rename = "continuationUrl", default)]
    continuation_url: Option<String>,
}

/// What a token is asked for with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Grant {
    /// The code of a sign-in that is being finished.
    AuthorizationCode,
    /// The saved login.
    RefreshToken,
}

fn parse_token_response(grant: Grant, status: u16, body: &str) -> eyre::Result<TokenResponse> {
    if let Ok(error) = serde_json::from_str::<EpicError>(body) {
        if error.error_code.as_deref().is_some_and(|c| !c.is_empty()) {
            return Err(token_error(grant, status, &error));
        }
    }
    if !(200..300).contains(&status) {
        return Err(eyre::Report::new(http::HttpStatus {
            status,
            url: OAUTH_TOKEN_URL.to_string(),
            body: http::snippet(body),
        })
        .wrap_err("Epic sign-in failed"));
    }
    serde_json::from_str(body).map_err(|err| {
        eyre::format_err!(
            "Unexpected answer from Epic's sign-in service: {err}. Body starts with: {}",
            http::snippet(body)
        )
    })
}

/// Only a refused refresh token is a rejected login: a refused code is about the sign-in
/// being finished, and anything else says nothing about the saved login.
fn token_error(grant: Grant, status: u16, error: &EpicError) -> eyre::Report {
    let code = error.error_code.as_deref().unwrap_or_default();
    let message = error
        .error_message
        .as_deref()
        .filter(|m| !m.is_empty())
        .unwrap_or(code);
    if code.ends_with("corrective_action_required") {
        let url = error
            .continuation_url
            .as_deref()
            .unwrap_or("https://www.epicgames.com/account");
        let then = match grant {
            Grant::AuthorizationCode => "sign in again",
            Grant::RefreshToken => "refresh the library again",
        };
        return eyre::format_err!(
            "Epic needs you to confirm something on your account first ({message}). Open {url} \
             in a browser, then {then}."
        );
    }
    // Epic's own code names what was refused (`…oauth.authorization_code_not_found`,
    // `…auth_token.invalid_refresh_token`). Where it does not, the OAuth name says the grant
    // that was sent is no longer good.
    let invalid_grant = error.error.as_deref() == Some("invalid_grant");
    if code.contains("authorization_code") || (invalid_grant && grant == Grant::AuthorizationCode) {
        return eyre::format_err!(
            "Epic did not accept the login code: it expired or was already used. Sign in again."
        );
    }
    if code.contains("refresh_token") || (invalid_grant && grant == Grant::RefreshToken) {
        return http::login_rejected("Your Epic sign-in has expired. Sign in again.");
    }
    let text = format!("Epic sign-in failed: {message} ({code})");
    if http::is_transient_status(status) {
        http::transient(text)
    } else {
        eyre::Report::msg(text)
    }
}

async fn request_token(
    client: &Client,
    grant: Grant,
    form: &[(&str, &str)],
) -> eyre::Result<TokenResponse> {
    let response = client
        .post(OAUTH_TOKEN_URL)
        .basic_auth(CLIENT_ID, Some(CLIENT_SECRET))
        .header(USER_AGENT, LAUNCHER_USER_AGENT)
        .form(form)
        .send()
        .await
        .map_err(http::request_failed)
        .wrap_err("Could not reach Epic's sign-in service")?;
    let status = response.status().as_u16();
    let body = response.text().await.map_err(http::request_failed)?;
    parse_token_response(grant, status, &body)
}

/// What BoilR keeps in `Session.data` for Epic. Earlier builds also wrote
/// `refresh_expires_at`, which nothing read; files that have it still load.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct EpicSession {
    access_token: String,
    /// Unix time in seconds.
    access_expires_at: u64,
    refresh_token: String,
    account_id: String,
    #[serde(default)]
    display_name: Option<String>,
}

impl EpicSession {
    fn from_tokens(tokens: TokenResponse, previous_name: Option<String>, now: u64) -> Self {
        Self {
            access_token: tokens.access_token,
            access_expires_at: now.saturating_add(tokens.expires_in),
            refresh_token: tokens.refresh_token,
            account_id: tokens.account_id,
            display_name: tokens
                .display_name
                .filter(|n| !n.trim().is_empty())
                .or(previous_name),
        }
    }

    fn needs_refresh(&self, now: u64) -> bool {
        now.saturating_add(REFRESH_MARGIN_SECS) >= self.access_expires_at
    }
}

/// Ends the session on Epic's side, as legendary does when it logs out. A login Epic no
/// longer accepts has no session left to end.
pub async fn sign_out(client: &Client, session: &Session) -> eyre::Result<()> {
    let mut data: EpicSession = serde_json::from_value(session.data.clone())
        .map_err(|_| eyre::format_err!("the saved login could not be read"))?;
    if data.needs_refresh(now_unix()) {
        match refresh(client, &mut data, &mut session.clone()).await {
            Err(err) if http::is_login_rejected(&err) => return Ok(()),
            refreshed => refreshed?,
        }
    }
    // The access token is part of the URL's path, so no error here may name the URL.
    let response = authorized(
        client.delete(format!("{SESSION_KILL_URL}{}", data.access_token)),
        &data.access_token,
    )
    .send()
    .await
    .map_err(|err| http::request_failed(err.without_url()))
    .wrap_err("Epic could not be reached")?;
    match session_end(response.status().as_u16()) {
        SessionEnd::Ended => Ok(()),
        SessionEnd::Failed(err) => Err(err),
    }
}

/// How Epic answered the request to end a session.
enum SessionEnd {
    /// The session is over: Epic ended it, or no longer knew the token.
    Ended,
    Failed(eyre::Report),
}

fn session_end(status: u16) -> SessionEnd {
    if (200..300).contains(&status) || status == 401 {
        return SessionEnd::Ended;
    }
    let status_text = reqwest::StatusCode::from_u16(status)
        .map(|s| s.to_string())
        .unwrap_or_else(|_| status.to_string());
    let text = format!("Epic answered {status_text}");
    SessionEnd::Failed(if http::is_transient_status(status) {
        http::transient(text)
    } else {
        eyre::Report::msg(text)
    })
}

/// Trades the refresh token for new tokens and writes them into the session.
async fn refresh(
    client: &Client,
    data: &mut EpicSession,
    session: &mut Session,
) -> eyre::Result<()> {
    let tokens = request_token(
        client,
        Grant::RefreshToken,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", &data.refresh_token),
            ("token_type", "eg1"),
        ],
    )
    .await?;
    let mut refreshed = EpicSession::from_tokens(tokens, data.display_name.clone(), now_unix());
    if refreshed.account_id.is_empty() {
        refreshed.account_id = data.account_id.clone();
    }
    *data = refreshed;
    session.data = serde_json::to_value(&*data)?;
    if data.display_name.is_some() {
        session.account_name = data.display_name.clone();
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct AccountResponse {
    #[serde(rename = "displayName", default)]
    display_name: Option<String>,
}

async fn fetch_display_name(client: &Client, data: &EpicSession) -> eyre::Result<Option<String>> {
    if data.account_id.is_empty() {
        return Ok(None);
    }
    let url = format!("{ACCOUNT_URL}{}", data.account_id);
    let account: AccountResponse =
        http::send_json(authorized(client.get(url), &data.access_token)).await?;
    Ok(account.display_name.filter(|n| !n.trim().is_empty()))
}

// ---------------------------------------------------------------------------------------------
// Library

fn authorized(request: RequestBuilder, access_token: &str) -> RequestBuilder {
    request
        .header(AUTHORIZATION, format!("bearer {access_token}"))
        .header(USER_AGENT, LAUNCHER_USER_AGENT)
}

fn library_request(client: &Client, access_token: &str, cursor: Option<&str>) -> RequestBuilder {
    // Playnite asks for the Windows platform; legendary sends no platform.
    let mut query = vec![("includeMetadata", "true"), ("platform", "Windows")];
    if let Some(cursor) = cursor {
        query.push(("cursor", cursor));
    }
    authorized(client.get(LIBRARY_ITEMS_URL), access_token).query(&query)
}

#[derive(Debug, Default, Deserialize)]
struct LibraryPage {
    #[serde(rename = "responseMetadata", default)]
    response_metadata: Option<ResponseMetadata>,
    #[serde(default)]
    records: Vec<LibraryRecord>,
}

impl LibraryPage {
    /// The page's records and the cursor of the next page. The last page has no cursor, or
    /// an empty one.
    fn into_parts(self) -> (Vec<LibraryRecord>, Option<String>) {
        let next = self
            .response_metadata
            .and_then(|m| m.next_cursor)
            .filter(|cursor| !cursor.is_empty());
        (self.records, next)
    }
}

#[derive(Debug, Default, Deserialize)]
struct ResponseMetadata {
    #[serde(rename = "nextCursor", default)]
    next_cursor: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct LibraryRecord {
    #[serde(rename = "appName", default)]
    app_name: Option<String>,
    #[serde(rename = "catalogItemId", default)]
    catalog_item_id: Option<String>,
    #[serde(default)]
    namespace: Option<String>,
    #[serde(rename = "sandboxType", default)]
    sandbox_type: Option<String>,
}

/// A library record worth looking up in the catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LibraryEntry {
    app_name: String,
    namespace: String,
    catalog_item_id: String,
}

impl LibraryEntry {
    fn catalog_key(&self) -> CatalogKey {
        (self.namespace.clone(), self.catalog_item_id.clone())
    }
}

/// Drops what Playnite and legendary skip before the catalog lookup: Unreal Engine
/// marketplace assets (namespace `ue`), private sandboxes (unreleased builds), records without
/// an app name, and legendary's broken app name "1". Keeps the first record per app name.
fn library_entries(records: Vec<LibraryRecord>) -> Vec<LibraryEntry> {
    let mut seen = BTreeSet::new();
    records
        .into_iter()
        .filter_map(|record| {
            let app_name = record.app_name.filter(|a| !a.is_empty() && a != "1")?;
            let namespace = record
                .namespace
                .filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case("ue"))?;
            let catalog_item_id = record.catalog_item_id.filter(|c| !c.is_empty())?;
            if record
                .sandbox_type
                .is_some_and(|s| s.eq_ignore_ascii_case("PRIVATE"))
            {
                return None;
            }
            Some(LibraryEntry {
                app_name,
                namespace,
                catalog_item_id,
            })
        })
        .filter(|entry| seen.insert(entry.app_name.clone()))
        .collect()
}

/// The previous list's game for a library entry: the same app under the same catalog item.
fn known_game<'a>(entry: &LibraryEntry, previous: &'a [OwnedGame]) -> Option<&'a OwnedGame> {
    previous.iter().find(|game| {
        game.id == entry.app_name
            && game.extra("namespace") == Some(entry.namespace.as_str())
            && game.extra("catalog_item_id") == Some(entry.catalog_item_id.as_str())
    })
}

/// The entries the catalog is asked about: those the previous list has no game for. Most
/// namespaces hold one game, so a lookup costs about one request per entry; a game that is
/// already listed is taken over instead. What is left is new games, and the entries that
/// are no games (DLC, extras), which no list remembers.
fn entries_to_look_up<'a>(
    entries: &'a [LibraryEntry],
    previous: &[OwnedGame],
) -> Vec<&'a LibraryEntry> {
    entries
        .iter()
        .filter(|entry| known_game(entry, previous).is_none())
        .collect()
}

/// A catalog request: a namespace and the item ids asked for in it.
type Batch = (String, Vec<String>);

/// Catalog lookups grouped per namespace (the bulk endpoint is per namespace), at most
/// [`CATALOG_BATCH_SIZE`] ids each.
fn catalog_batches<'a>(entries: impl IntoIterator<Item = &'a LibraryEntry>) -> Vec<Batch> {
    let mut per_namespace: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for entry in entries {
        per_namespace
            .entry(&entry.namespace)
            .or_default()
            .insert(&entry.catalog_item_id);
    }
    per_namespace
        .into_iter()
        .flat_map(|(namespace, ids)| {
            let ids: Vec<String> = ids.into_iter().map(str::to_string).collect();
            ids.chunks(CATALOG_BATCH_SIZE)
                .map(|chunk| (namespace.to_string(), chunk.to_vec()))
                .collect::<Vec<_>>()
        })
        .collect()
}

type CatalogKey = (String, String);

/// What a catalog lookup found, and which items it could not look up because a request
/// failed (as opposed to items the service does not know).
#[derive(Default)]
struct CatalogLookup {
    found: BTreeMap<CatalogKey, CatalogItem>,
    failed: BTreeSet<CatalogKey>,
    /// The first request error, reported when nothing could be looked up.
    error: Option<eyre::Report>,
}

impl CatalogLookup {
    /// Takes in the outcome of one request. The ids of a failed request count as failed,
    /// unless asking for them one by one may still work ([`worth_asking_one_by_one`]).
    fn record(
        &mut self,
        (namespace, ids): Batch,
        outcome: eyre::Result<BTreeMap<String, CatalogItem>>,
    ) {
        match outcome {
            Ok(items) => {
                for (id, item) in items {
                    self.found.insert((namespace.clone(), id), item);
                }
            }
            Err(err) if worth_asking_one_by_one(ids.len(), &err) => {
                self.error.get_or_insert(err);
            }
            Err(err) => self.fail((namespace, ids), err),
        }
    }

    fn fail(&mut self, (namespace, ids): Batch, err: eyre::Report) {
        self.failed
            .extend(ids.into_iter().map(|id| (namespace.clone(), id)));
        self.error.get_or_insert(err);
    }
}

/// Whether the ids of a failed request are worth asking for one at a time: only when there
/// were several and the service refused the request itself (one bad id, a URL it does not
/// take). An outage or a refused token would fail the same way again.
fn worth_asking_one_by_one(ids: usize, err: &eyre::Report) -> bool {
    ids > 1 && !http::is_transient(err) && !http::is_login_rejected(err)
}

/// The requests to make one id at a time after the batched ones: the ids of a batch of
/// several that it neither answered nor failed for good. So the lookup works whether or not
/// the service honours several `id` parameters in one request, and one bad id does not hide
/// the other games of its namespace. A request for a single id is never repeated.
fn single_lookups(batches: &[Batch], first: &CatalogLookup) -> Vec<Batch> {
    batches
        .iter()
        .filter(|(_, ids)| ids.len() > 1)
        .flat_map(|(namespace, ids)| ids.iter().map(move |id| (namespace.clone(), id.clone())))
        .filter(|key| !first.found.contains_key(key) && !first.failed.contains(key))
        .map(|(namespace, id)| (namespace, vec![id]))
        .collect()
}

/// Looks up catalog items, [`CATALOG_CONCURRENCY`] requests at a time, in batches first and
/// then one by one where that may help ([`single_lookups`]).
async fn fetch_catalog(client: &Client, access_token: &str, batches: Vec<Batch>) -> CatalogLookup {
    let mut lookup = run_catalog_requests(client, access_token, batches.clone()).await;
    let singles = single_lookups(&batches, &lookup);
    if !singles.is_empty() {
        let second = run_catalog_requests(client, access_token, singles).await;
        lookup.found.extend(second.found);
        lookup.failed.extend(second.failed);
        lookup.error = lookup.error.or(second.error);
    }
    lookup
}

async fn run_catalog_requests(
    client: &Client,
    access_token: &str,
    batches: Vec<Batch>,
) -> CatalogLookup {
    let mut lookup = CatalogLookup::default();
    let mut pending = batches.into_iter();
    let mut tasks = JoinSet::new();
    // What each running task asks for, so a task that ends without an answer still counts.
    let mut running: BTreeMap<tokio::task::Id, Batch> = BTreeMap::new();
    loop {
        while tasks.len() < CATALOG_CONCURRENCY {
            let Some(batch) = pending.next() else {
                break;
            };
            let client = client.clone();
            let access_token = access_token.to_string();
            let (namespace, ids) = batch.clone();
            let task = tasks.spawn(async move {
                fetch_catalog_batch(&client, &access_token, &namespace, &ids).await
            });
            running.insert(task.id(), batch);
        }
        let Some(joined) = tasks.join_next_with_id().await else {
            break;
        };
        match joined {
            Ok((task, outcome)) => {
                if let Some(batch) = running.remove(&task) {
                    lookup.record(batch, outcome);
                }
            }
            Err(stopped) => {
                if let Some(batch) = running.remove(&stopped.id()) {
                    let err = eyre::format_err!("Looking up Epic games stopped early: {stopped}");
                    lookup.fail(batch, err);
                }
            }
        }
    }
    lookup
}

async fn fetch_catalog_batch(
    client: &Client,
    access_token: &str,
    namespace: &str,
    ids: &[String],
) -> eyre::Result<BTreeMap<String, CatalogItem>> {
    let mut url = url::Url::parse(CATALOG_URL)?;
    url.path_segments_mut()
        .map_err(|_| eyre::format_err!("Bad Epic catalog URL"))?
        .pop_if_empty()
        .push(namespace)
        .push("bulk")
        .push("items");
    let mut query: Vec<(&str, &str)> = ids.iter().map(|id| ("id", id.as_str())).collect();
    query.extend([
        ("includeDLCDetails", "false"),
        ("includeMainGameDetails", "true"),
        ("country", "US"),
        ("locale", "en-US"),
    ]);
    let raw: BTreeMap<String, serde_json::Value> =
        http::send_json(authorized(client.get(url), access_token).query(&query)).await?;
    Ok(parse_catalog(raw))
}

#[derive(Clone, Debug, Default, Deserialize)]
struct CatalogItem {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    categories: Option<Vec<Category>>,
    #[serde(rename = "mainGameItem", default)]
    main_game_item: Option<serde_json::Value>,
    #[serde(rename = "customAttributes", default)]
    custom_attributes: Option<BTreeMap<String, CustomAttribute>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct Category {
    #[serde(default)]
    path: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct CustomAttribute {
    #[serde(default)]
    value: Option<String>,
}

impl CatalogItem {
    fn has_category(&self, path: &str) -> bool {
        self.categories
            .iter()
            .flatten()
            .any(|c| c.path.eq_ignore_ascii_case(path))
    }

    fn attribute(&self, name: &str) -> Option<&str> {
        self.custom_attributes
            .as_ref()?
            .get(name)?
            .value
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
    }

    /// The last part of the game's store page, `store.epicgames.com/p/<slug>`. The attribute
    /// is written for the store's older addresses and can end in `/home`. A value that is
    /// not a plain slug after that is not used: it ends up in a link.
    fn product_slug(&self) -> Option<&str> {
        let value = self
            .attribute(PRODUCT_SLUG_ATTRIBUTE)?
            .trim_end_matches('/');
        let slug = value
            .strip_suffix("/home")
            .unwrap_or(value)
            .trim_matches('/');
        let plain = !slug.is_empty()
            && slug
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        plain.then_some(slug)
    }
}

/// Keeps the items that have the catalog's shape; one odd item must not hide the others.
fn parse_catalog(raw: BTreeMap<String, serde_json::Value>) -> BTreeMap<String, CatalogItem> {
    raw.into_iter()
        .filter_map(|(id, value)| Some((id, serde_json::from_value(value).ok()?)))
        .collect()
}

/// Turns a library entry into a game, skipping what Playnite's `GetLibraryGames` skips: items
/// that are not applications, DLC that cannot be launched on its own, digital extras
/// (soundtracks, art books) and engine plugins. legendary also skips mods.
fn owned_game(entry: &LibraryEntry, item: &CatalogItem) -> Option<OwnedGame> {
    if !item.has_category("applications") {
        return None;
    }
    if item.main_game_item.as_ref().is_some_and(|m| !m.is_null())
        && !item.has_category("addons/launchable")
    {
        return None;
    }
    if ["digitalextras", "plugins", "plugins/engine", "mods"]
        .iter()
        .any(|c| item.has_category(c))
    {
        return None;
    }
    let title = clean_title(item.title.as_deref()?);
    if title.is_empty() {
        return None;
    }
    let game = OwnedGame::new(StoreId::Epic, entry.app_name.clone(), title)
        .with_extra("namespace", entry.namespace.clone())
        .with_extra("catalog_item_id", entry.catalog_item_id.clone());
    Some(match item.product_slug() {
        Some(slug) => game.with_extra("product_slug", slug),
        None => game,
    })
}

/// The games of a library, and how many of its entries are left out because they could not
/// be looked up.
#[derive(Default)]
struct Listed {
    games: Vec<OwnedGame>,
    missing: usize,
}

/// Puts the list together, entry by entry: a game the previous list holds is taken over as
/// it is; one the catalog was asked about is listed when the catalog says it is a game; one
/// whose lookup failed stays as the previous list had it under the same app name, so a
/// passing service error never removes it from Steam, and counts as missing when the
/// previous list does not know it.
fn list_games(entries: &[LibraryEntry], previous: &[OwnedGame], lookup: &CatalogLookup) -> Listed {
    let mut listed = Listed::default();
    for entry in entries {
        if let Some(known) = known_game(entry, previous) {
            listed.games.push(known.clone());
            continue;
        }
        let key = entry.catalog_key();
        if let Some(item) = lookup.found.get(&key) {
            listed.games.extend(owned_game(entry, item));
        } else if lookup.failed.contains(&key) {
            match previous.iter().find(|game| game.id == entry.app_name) {
                Some(before) => listed.games.push(before.clone()),
                None => listed.missing += 1,
            }
        }
    }
    listed
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = include_str!("../testdata/epic/token_response.json");
    const TOKEN_CODE_USED: &str = include_str!("../testdata/epic/token_error_code_used.json");
    const TOKEN_CORRECTIVE: &str = include_str!("../testdata/epic/token_error_corrective.json");
    const REDIRECT_IN: &str = include_str!("../testdata/epic/redirect_signed_in.json");
    const REDIRECT_OUT: &str = include_str!("../testdata/epic/redirect_signed_out.json");
    const LIBRARY_1: &str = include_str!("../testdata/epic/library_page1.json");
    const LIBRARY_2: &str = include_str!("../testdata/epic/library_page2.json");
    const CATALOG: &str = include_str!("../testdata/epic/catalog_bulk.json");

    const CODE: &str = "0a1b2c3d4e5f60718293a4b5c6d7e8f9";

    fn entry(app: &str, item: &str) -> LibraryEntry {
        LibraryEntry {
            app_name: app.to_string(),
            namespace: "ns".to_string(),
            catalog_item_id: item.to_string(),
        }
    }

    fn key(item: &str) -> CatalogKey {
        ("ns".to_string(), item.to_string())
    }

    /// A game as an earlier list holds it for [`entry`]`(app, item)`.
    fn listed_before(app: &str, item: &str, title: &str) -> OwnedGame {
        OwnedGame::new(StoreId::Epic, app, title)
            .with_extra("namespace", "ns")
            .with_extra("catalog_item_id", item)
    }

    fn game_item(title: &str) -> eyre::Result<CatalogItem> {
        Ok(serde_json::from_value(serde_json::json!({
            "title": title,
            "categories": [{"path": "games"}, {"path": "applications"}],
        }))?)
    }

    fn status(status: u16) -> eyre::Report {
        eyre::Report::new(http::HttpStatus {
            status,
            url: "https://catalog.example/bulk/items".into(),
            body: String::new(),
        })
    }

    fn titles(games: &[OwnedGame]) -> Vec<&str> {
        games.iter().map(|g| g.title.as_str()).collect()
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
    fn login_urls_never_match_done_prefixes() -> eyre::Result<()> {
        let pending = begin_login();
        let manual_url = match &pending.manual {
            ManualLogin::Paste { url, .. } => url.clone(),
            ManualLogin::Unsupported => String::new(),
        };
        assert!(!manual_url.is_empty());
        for url in [&pending.start_url, &manual_url] {
            assert!(pending
                .done_url_prefixes
                .iter()
                .all(|p| !url.starts_with(p)));
        }
        assert_eq!(
            http::query_param(&pending.start_url, "redirectUrl"),
            Some(redirect_url())
        );
        assert!(redirect_url().starts_with(REDIRECT_URL));
        assert!(redirect_url().contains(CLIENT_ID));
        Ok(())
    }

    #[test]
    fn the_login_window_loads_the_redirect_page_and_nothing_else() {
        let pending = begin_login();
        // The page with the code is loaded, so its text can be handed over.
        assert!(pending
            .page_text_url_prefixes
            .iter()
            .any(|p| redirect_url().starts_with(p)));
        // Only done pages are loaded for their text.
        assert!(pending
            .page_text_url_prefixes
            .iter()
            .all(|p| pending.done_url_prefixes.contains(p)));
        // The launcher's callback has the code in its URL and no page behind it.
        assert!(pending
            .page_text_url_prefixes
            .iter()
            .all(|p| !p.contains("localhost")));
        assert!(pending
            .done_url_prefixes
            .iter()
            .any(|p| p.contains("localhost")));
    }

    #[test]
    fn the_cookie_replay_uses_the_login_windows_user_agent() -> eyre::Result<()> {
        let pending = begin_login();
        assert_eq!(pending.user_agent.as_deref(), Some(LOGIN_USER_AGENT));
        let request = redirect_page_request(&Client::new(), "a=1".to_string()).build()?;
        let sent = request
            .headers()
            .get(USER_AGENT)
            .and_then(|v| v.to_str().ok());
        assert_eq!(sent, pending.user_agent.as_deref());
        assert_eq!(request.url().as_str(), redirect_url());
        Ok(())
    }

    #[test]
    fn reads_code_from_redirect_page() -> eyre::Result<()> {
        assert_eq!(parse_redirect_response(REDIRECT_IN)?, CODE);
        let err = parse_redirect_response(REDIRECT_OUT)
            .err()
            .map(|e| e.to_string());
        assert!(err.is_some_and(|e| e.contains("not signed in")));
        assert!(parse_redirect_response("<html>").is_err());
        // A webview that shows the JSON through a viewer of its own.
        let viewer = format!("redirectUrl\t\"x\"\nauthorizationCode\t\"{CODE}\"\nsid\tnull");
        assert_eq!(parse_redirect_response(&viewer)?, CODE);
        Ok(())
    }

    #[test]
    fn a_login_window_hands_over_the_code_in_the_url_or_in_the_page_text() -> eyre::Result<()> {
        let capture = |final_url: &str, page_text: Option<&str>| LoginCapture {
            final_url: Some(final_url.to_string()),
            page_text: page_text.map(str::to_string),
            cookies: vec![],
            pasted: None,
        };
        let found = |capture: &LoginCapture| code_in_capture(capture).transpose();

        // The launcher's callback: the code is in the URL.
        let callback = format!("https://localhost/launcher/authorized?code={CODE}");
        assert_eq!(found(&capture(&callback, None))?.as_deref(), Some(CODE));
        // The redirect page, loaded by the window.
        let loaded = capture(&redirect_url(), Some(REDIRECT_IN));
        assert_eq!(found(&loaded)?.as_deref(), Some(CODE));
        // Loaded, but the window was not signed in: an error, not a second request.
        let signed_out = capture(&redirect_url(), Some(REDIRECT_OUT));
        assert!(code_in_capture(&signed_out).is_some_and(|code| code.is_err()));
        // No text (an older window, or one that could not read the page): the cookies are
        // replayed.
        assert!(code_in_capture(&capture(&redirect_url(), None)).is_none());
        assert!(code_in_capture(&capture(&redirect_url(), Some("  "))).is_none());
        assert!(code_in_capture(&LoginCapture::default()).is_none());
        Ok(())
    }

    #[test]
    fn accepts_every_pasted_form() -> eyre::Result<()> {
        let code = CODE;
        assert_eq!(parse_pasted_code(code)?, code);
        assert_eq!(parse_pasted_code(&format!("  \"{code}\"\n"))?, code);
        assert_eq!(parse_pasted_code(REDIRECT_IN)?, code);
        assert_eq!(
            parse_pasted_code(&format!("\"authorizationCode\":\"{code}\","))?,
            code
        );
        assert_eq!(
            parse_pasted_code(&format!(
                "https://localhost/launcher/authorized?code={code}"
            ))?,
            code
        );
        assert!(parse_pasted_code("not a code!").is_err());
        assert!(parse_pasted_code("abc").is_err());
        assert!(parse_pasted_code(REDIRECT_OUT).is_err());
        Ok(())
    }

    #[test]
    fn finds_the_pasted_code_whatever_surrounds_it() -> eyre::Result<()> {
        let code = CODE;
        // Firefox's JSON viewer copies a row as name, tab, quoted value.
        assert_eq!(
            parse_pasted_code(&format!("authorizationCode\t\"{code}\""))?,
            code
        );
        // And the whole page as rows.
        let rows = format!(
            "warning\t\"Do not share this code with any 3rd party service.\"\n\
             redirectUrl\t\"https://localhost/launcher/authorized?code={code}\"\n\
             authorizationCode\t\"{code}\"\nexchangeCode\tnull\nsid\tnull"
        );
        assert_eq!(parse_pasted_code(&rows)?, code);
        assert_eq!(
            parse_pasted_code(&format!("authorizationCode: {code}"))?,
            code
        );
        // JSON cut off while selecting.
        assert_eq!(
            parse_pasted_code(&format!(
                "{{\"warning\":\"x\",\"authorizationCode\":\"{code}"
            ))?,
            code
        );
        // No name at all, but one thing that is a code.
        assert_eq!(parse_pasted_code(&format!("the code is {code}."))?, code);
        assert_eq!(
            parse_pasted_code(&format!("see https://x.example/?a=1&code={code}#top ok"))?,
            code
        );

        // The rows of a browser that is not signed in.
        let signed_out = parse_pasted_code("redirectUrl\t\"x\"\nauthorizationCode\tnull")
            .err()
            .map(|e| e.to_string());
        assert!(signed_out.is_some_and(|e| e.contains("not signed in")));
        // Two different codes: no telling which one is meant.
        let other = "f".repeat(32);
        assert!(parse_pasted_code(&format!("{code} or {other}")).is_err());
        // Epic's own instructions, without a code.
        assert!(parse_pasted_code("a page of text containing authorizationCode. Copy it").is_err());
        Ok(())
    }

    #[test]
    fn parses_tokens_into_a_session() -> eyre::Result<()> {
        let tokens = parse_token_response(Grant::AuthorizationCode, 200, TOKEN)?;
        let session = EpicSession::from_tokens(tokens, None, 1_000);
        assert_eq!(session.access_token, "eg1~access");
        assert_eq!(session.refresh_token, "eg1~refresh");
        assert_eq!(session.access_expires_at, 8_200);
        assert_eq!(session.account_id, "0123456789abcdef0123456789abcdef");
        assert_eq!(session.display_name.as_deref(), Some("Player One"));

        assert!(!session.needs_refresh(1_000));
        assert!(session.needs_refresh(8_200 - REFRESH_MARGIN_SECS));
        assert!(session.needs_refresh(9_000));

        let stored: EpicSession = serde_json::from_value(serde_json::to_value(&session)?)?;
        assert!(stored == session);
        Ok(())
    }

    #[test]
    fn sessions_saved_by_earlier_builds_still_load() -> eyre::Result<()> {
        let saved = Session {
            account_name: Some("Player One".into()),
            data: serde_json::json!({
                "access_token": "eg1~access",
                "access_expires_at": 8_200,
                "refresh_token": "eg1~refresh",
                "refresh_expires_at": 1_988_200,
                "account_id": "0123456789abcdef0123456789abcdef",
                "display_name": "Player One",
            }),
        };
        let data: EpicSession = session_data(StoreId::Epic, &saved)?;
        assert_eq!(data.refresh_token, "eg1~refresh");
        assert_eq!(data.access_expires_at, 8_200);

        let broken = Session {
            account_name: None,
            data: serde_json::json!({ "access_token": "only this" }),
        };
        let unreadable = session_data::<EpicSession>(StoreId::Epic, &broken);
        assert!(unreadable.is_err_and(|e| http::is_login_rejected(&e)));
        Ok(())
    }

    #[test]
    fn refresh_keeps_the_known_name() -> eyre::Result<()> {
        let mut tokens = parse_token_response(Grant::RefreshToken, 200, TOKEN)?;
        tokens.display_name = None;
        let session = EpicSession::from_tokens(tokens, Some("Old Name".into()), 0);
        assert_eq!(session.display_name.as_deref(), Some("Old Name"));
        Ok(())
    }

    #[test]
    fn explains_token_errors() {
        let used = parse_token_response(Grant::AuthorizationCode, 400, TOKEN_CODE_USED).err();
        assert!(used.is_some_and(|e| {
            // About the code of this sign-in, not about a saved login.
            e.to_string().contains("expired or was already used") && !http::is_login_rejected(&e)
        }));

        let corrective = parse_token_response(Grant::AuthorizationCode, 400, TOKEN_CORRECTIVE)
            .err()
            .map(|e| e.to_string());
        assert!(corrective.is_some_and(|e| e.contains("continuation=abc")));
        let later = parse_token_response(Grant::RefreshToken, 400, TOKEN_CORRECTIVE).err();
        assert!(later.is_some_and(|e| {
            !http::is_login_rejected(&e) && e.to_string().contains("refresh the library again")
        }));
    }

    #[test]
    fn only_a_refused_refresh_token_is_a_rejected_login() {
        let refresh = |status: u16, body: &str| {
            parse_token_response(Grant::RefreshToken, status, body)
                .err()
                .unwrap_or_else(|| eyre::format_err!("no error"))
        };
        // What Epic sends for a refresh token that expired or was ended.
        let dead = refresh(
            400,
            r#"{"errorCode":"errors.com.epicgames.account.auth_token.invalid_refresh_token","errorMessage":"Sorry the refresh token is invalid"}"#,
        );
        assert!(http::is_login_rejected(&dead), "{dead}");
        assert!(dead.to_string().contains("Sign in again"));
        // Another code of Epic's, with the OAuth name next to it (as in the fixture).
        let invalid_grant = refresh(
            400,
            r#"{"errorCode":"errors.com.epicgames.account.oauth.something_else","error":"invalid_grant"}"#,
        );
        assert!(http::is_login_rejected(&invalid_grant), "{invalid_grant}");
        // The same answer to a pasted code is about that code.
        let code = parse_token_response(Grant::AuthorizationCode, 400, TOKEN_CODE_USED).err();
        assert!(code.is_some_and(|e| !http::is_login_rejected(&e)));

        // An outage is neither, with or without Epic's error body.
        let outage = refresh(503, "<html>Service Unavailable</html>");
        assert!(http::is_transient(&outage) && !http::is_login_rejected(&outage));
        assert!(
            outage.to_string().contains("Epic sign-in failed"),
            "{outage}"
        );
        let busy = refresh(
            500,
            r#"{"errorCode":"errors.com.epicgames.common.server_error","errorMessage":"Try later"}"#,
        );
        assert!(http::is_transient(&busy) && !http::is_login_rejected(&busy));
        let limited = refresh(429, "");
        assert!(http::is_transient(&limited) && !http::is_login_rejected(&limited));
        // Something Epic does not like about the request says nothing about the login.
        let other = refresh(
            400,
            r#"{"errorCode":"errors.com.epicgames.common.oauth.invalid_client","errorMessage":"No"}"#,
        );
        assert!(!http::is_transient(&other) && !http::is_login_rejected(&other));
        assert!(!http::is_login_rejected(&refresh(403, "blocked")));
    }

    #[test]
    fn a_session_epic_no_longer_knows_counts_as_ended() {
        assert!(matches!(session_end(204), SessionEnd::Ended));
        assert!(matches!(session_end(200), SessionEnd::Ended));
        assert!(matches!(session_end(401), SessionEnd::Ended));
        assert!(matches!(
            session_end(503),
            SessionEnd::Failed(err) if http::is_transient(&err)
        ));
        assert!(matches!(
            session_end(403),
            SessionEnd::Failed(err) if !http::is_transient(&err) && err.to_string().contains("403")
        ));
    }

    fn library() -> eyre::Result<Vec<LibraryEntry>> {
        let first: LibraryPage = serde_json::from_str(LIBRARY_1)?;
        let second: LibraryPage = serde_json::from_str(LIBRARY_2)?;
        let (mut records, next) = first.into_parts();
        assert_eq!(next.as_deref(), Some("cursor-page-2"));
        let (more, end) = second.into_parts();
        assert_eq!(end, None);
        records.extend(more);
        Ok(library_entries(records))
    }

    #[test]
    fn an_empty_cursor_ends_the_library() -> eyre::Result<()> {
        let page: LibraryPage =
            serde_json::from_str(r#"{"responseMetadata":{"nextCursor":""},"records":[{}]}"#)?;
        let (records, next) = page.into_parts();
        assert_eq!(records.len(), 1);
        assert_eq!(next, None);
        let bare: LibraryPage = serde_json::from_str("{}")?;
        assert_eq!(bare.into_parts().1, None);
        Ok(())
    }

    #[test]
    fn filters_library_records() -> eyre::Result<()> {
        let names: Vec<String> = library()?.into_iter().map(|e| e.app_name).collect();
        assert_eq!(
            names,
            [
                "Fortnite",
                "Calluna",
                "CallunaDLC1",
                "CallunaExpansionLauncher",
                "EAGame",
                "UbiGame",
                "Soundtrack",
                "EnginePlugin",
                "NotAnApplication",
            ]
        );
        Ok(())
    }

    #[test]
    fn batches_catalog_lookups_per_namespace() -> eyre::Result<()> {
        let batches = catalog_batches(&library()?);
        let control = batches.iter().find(|(ns, _)| ns == "ns-control");
        assert_eq!(
            control.map(|(_, ids)| ids.clone()),
            Some(vec![
                "cat-control".to_string(),
                "cat-control-dlc".to_string(),
                "cat-control-launchable".to_string(),
            ])
        );
        assert_eq!(batches.len(), 7);

        let many: Vec<LibraryEntry> = (0..120)
            .map(|i| LibraryEntry {
                app_name: format!("app{i}"),
                namespace: "ns".into(),
                catalog_item_id: format!("id{i}"),
            })
            .collect();
        let sizes: Vec<usize> = catalog_batches(&many)
            .iter()
            .map(|(_, ids)| ids.len())
            .collect();
        assert_eq!(sizes, [50, 50, 20]);
        Ok(())
    }

    #[test]
    fn only_entries_the_previous_list_lacks_are_looked_up() {
        let entries = vec![
            entry("Fortnite", "a"),
            entry("Rocket", "b"),
            entry("RocketDLC", "c"),
            entry("New", "d"),
            entry("Moved", "e"),
        ];
        let previous = vec![
            listed_before("Fortnite", "a", "Fortnite"),
            listed_before("Rocket", "b", "Rocket League"),
            // The same app, but under another catalog item than the library names now.
            listed_before("Moved", "old", "Moved"),
            // No longer in the library.
            listed_before("Gone", "z", "Gone"),
        ];
        let apps = |previous: &[OwnedGame]| -> Vec<String> {
            entries_to_look_up(&entries, previous)
                .into_iter()
                .map(|e| e.app_name.clone())
                .collect()
        };
        // DLC is in no list, so it is asked about each time, like a new game.
        assert_eq!(apps(&previous), ["RocketDLC", "New", "Moved"]);
        // A first fetch looks up everything.
        assert_eq!(apps(&[]).len(), entries.len());
        // And the requests are made for those entries only.
        let batches = catalog_batches(entries_to_look_up(&entries, &previous));
        assert_eq!(
            batches,
            [(
                "ns".to_string(),
                vec!["c".to_string(), "d".to_string(), "e".to_string()]
            )]
        );
    }

    #[test]
    fn known_games_are_taken_over_and_failed_lookups_are_rescued_or_counted() -> eyre::Result<()> {
        let entries = vec![
            entry("Fortnite", "a"),
            entry("Moved", "b"),
            entry("New", "c"),
            entry("Lost", "d"),
            entry("Dlc", "e"),
            entry("Unknown", "f"),
        ];
        let previous = vec![
            // Listed before with a title and an extra of its own: taken over untouched.
            listed_before("Fortnite", "a", "Fortnite (as listed before)").with_extra("kept", "1"),
            listed_before("Moved", "old", "Moved"),
        ];
        let mut lookup = CatalogLookup::default();
        // Even an answer for a known game changes nothing: it was not asked for.
        lookup.found.insert(key("a"), game_item("Fortnite")?);
        lookup
            .found
            .insert(key("c"), game_item("New Game\u{2122}")?);
        let dlc: CatalogItem = serde_json::from_value(serde_json::json!({
            "title": "Some DLC",
            "categories": [{"path": "addons"}, {"path": "applications"}],
            "mainGameItem": {"id": "c"},
        }))?;
        lookup.found.insert(key("e"), dlc);
        lookup.failed.extend([key("b"), key("d")]);

        let listed = list_games(&entries, &previous, &lookup);

        assert_eq!(
            titles(&listed.games),
            ["Fortnite (as listed before)", "Moved", "New Game"]
        );
        assert_eq!(
            listed.games.first().and_then(|g| g.extra("kept")),
            Some("1")
        );
        // "Moved" failed but was listed before under its app name; "Lost" was not. The
        // catalog answered without "Unknown", which is not a failure.
        assert_eq!(listed.missing, 1);

        // Nothing failed: nothing is missing.
        lookup.failed.clear();
        assert_eq!(list_games(&entries, &previous, &lookup).missing, 0);
        Ok(())
    }

    #[test]
    fn failed_requests_count_their_ids_as_failed() -> eyre::Result<()> {
        let batch = |ids: &[&str]| -> Batch {
            (
                "ns".to_string(),
                ids.iter().map(|id| id.to_string()).collect(),
            )
        };
        let mut lookup = CatalogLookup::default();
        let answer: BTreeMap<String, CatalogItem> =
            [("a".to_string(), game_item("A")?)].into_iter().collect();
        lookup.record(batch(&["a", "b"]), Ok(answer));
        // An outage: every id of the request failed, and asking again would not help.
        lookup.record(batch(&["c", "d"]), Err(status(503)));
        // A single id the service refuses.
        lookup.record(batch(&["e"]), Err(status(404)));
        // A request for several ids the service refuses: maybe one of them is bad.
        lookup.record(batch(&["f", "g"]), Err(status(400)));
        // A task that ended without an answer.
        lookup.fail(batch(&["h", "i"]), eyre::format_err!("task stopped"));

        assert_eq!(lookup.found.keys().cloned().collect::<Vec<_>>(), [key("a")]);
        assert_eq!(
            lookup.failed.iter().cloned().collect::<Vec<_>>(),
            [key("c"), key("d"), key("e"), key("h"), key("i")]
        );
        // The first error is the one reported.
        assert!(lookup.error.as_ref().is_some_and(http::is_transient));

        // Asked one by one: what a batch of several left open. Never a single id again, and
        // never what an outage took.
        let batches = [
            batch(&["a", "b"]),
            batch(&["c", "d"]),
            batch(&["e"]),
            batch(&["f", "g"]),
            batch(&["h", "i"]),
            batch(&["unanswered"]),
        ];
        assert_eq!(
            single_lookups(&batches, &lookup),
            [batch(&["b"]), batch(&["f"]), batch(&["g"])]
        );
        Ok(())
    }

    #[test]
    fn only_a_refused_request_for_several_ids_is_split() {
        assert!(worth_asking_one_by_one(2, &status(400)));
        assert!(worth_asking_one_by_one(50, &status(404)));
        assert!(!worth_asking_one_by_one(1, &status(400)));
        assert!(!worth_asking_one_by_one(2, &status(503)));
        assert!(!worth_asking_one_by_one(2, &status(429)));
        assert!(!worth_asking_one_by_one(2, &status(401)));
        assert!(!worth_asking_one_by_one(2, &http::transient("timed out")));
    }

    #[test]
    fn turns_catalog_items_into_games() -> eyre::Result<()> {
        let raw: BTreeMap<String, serde_json::Value> = serde_json::from_str(CATALOG)?;
        let items = parse_catalog(raw);
        assert!(items.contains_key("cat-fortnite"));
        assert!(!items.contains_key("cat-broken"));

        let entries = library()?;
        let mut lookup = CatalogLookup::default();
        for e in &entries {
            if let Some(item) = items.get(&e.catalog_item_id) {
                lookup.found.insert(e.catalog_key(), item.clone());
            }
        }
        let listed = list_games(&entries, &[], &lookup);
        assert_eq!(listed.missing, 0);
        let games = listed.games;

        let summary: Vec<(&str, &str, Option<&str>)> = games
            .iter()
            .map(|g| (g.id.as_str(), g.title.as_str(), g.extra("product_slug")))
            .collect();
        assert_eq!(
            summary,
            [
                ("Fortnite", "Fortnite", Some("fortnite")),
                ("Calluna", "Control Ultimate Edition", Some("control")),
                (
                    "CallunaExpansionLauncher",
                    "Control Standalone Expansion",
                    None
                ),
                ("EAGame", "Star Wars Jedi: Fallen Order", None),
                ("UbiGame", "Assassin's Creed Valhalla", None),
            ]
        );

        let control = games.iter().find(|g| g.id == "Calluna");
        assert_eq!(
            control.and_then(|g| g.extra("namespace")),
            Some("ns-control")
        );
        assert_eq!(
            control.and_then(|g| g.extra("catalog_item_id")),
            Some("cat-control")
        );
        // Only the extras the install target reads are written.
        let extras: Vec<&str> = control
            .map(|g| g.extra.keys().map(String::as_str).collect())
            .unwrap_or_default();
        assert_eq!(extras, ["catalog_item_id", "namespace", "product_slug"]);
        assert!(games.iter().all(|g| g.store == StoreId::Epic));

        // The next refresh finds every one of them in the list and looks up the rest only.
        let again: Vec<&str> = entries_to_look_up(&entries, &games)
            .into_iter()
            .map(|e| e.app_name.as_str())
            .collect();
        assert_eq!(
            again,
            [
                "CallunaDLC1",
                "Soundtrack",
                "EnginePlugin",
                "NotAnApplication"
            ]
        );
        Ok(())
    }

    #[test]
    fn reads_the_store_page_slug() -> eyre::Result<()> {
        let slug_of = |value: serde_json::Value| -> eyre::Result<Option<String>> {
            let item: CatalogItem = serde_json::from_value(serde_json::json!({
                "customAttributes": { PRODUCT_SLUG_ATTRIBUTE: { "type": "STRING", "value": value } },
            }))?;
            Ok(item.product_slug().map(str::to_string))
        };
        assert_eq!(slug_of("fortnite".into())?.as_deref(), Some("fortnite"));
        // Written for the store's older addresses.
        assert_eq!(slug_of("control/home".into())?.as_deref(), Some("control"));
        assert_eq!(
            slug_of(" /a-plague-tale_2/home/ ".into())?.as_deref(),
            Some("a-plague-tale_2")
        );
        // Not a plain slug: left out rather than put in a link.
        assert_eq!(slug_of("bundles/some-bundle".into())?, None);
        assert_eq!(slug_of("x?redirect=https://evil.example".into())?, None);
        assert_eq!(slug_of("/home".into())?, None);
        assert_eq!(slug_of("".into())?, None);
        assert_eq!(slug_of(serde_json::Value::Null)?, None);
        let without: CatalogItem = serde_json::from_value(serde_json::json!({ "title": "x" }))?;
        assert_eq!(without.product_slug(), None);
        Ok(())
    }
}
