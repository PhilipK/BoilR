//! Amazon Games (Prime Gaming).
//!
//! Signs in the way the Amazon Games launcher does, as Playnite's AmazonGamesLibrary
//! (`Services/AmazonAccountClient.cs`) and nile (`nile/api/authorization.py`) do: an
//! OpenID sign-in page on amazon.com with a PKCE S256 challenge and a per-device client id,
//! then `api.amazon.com/auth/register` turns the authorization code into a device
//! registration with bearer tokens. The library comes from the launcher's entitlements
//! service (`gaming.amazon.com/api/distribution/entitlements`, nile `nile/api/library.py`).
//!
//! [`OwnedGame::id`] is the entitlement's `product.id` (`amzn1.adg.product.<uuid>`): the
//! `Id` column of the Amazon Games launcher's `GameInstallInfo.sqlite` (read by BoilR's
//! Amazon platform) and the `id` in nile's `installed.json` (read by BoilR's Heroic platform).
//!
//! `extra` keys:
//! - `entitlement_id`: the entitlement's own id (nile's `GetGameDownload` and the
//!   `AMAZON_GAMES_FUEL_ENTITLEMENT_ID` launch variable use it).
//! - `sku`: the product SKU, when the service sends one.
//! - `icon_url`: square product art, when the service sends one.

use eyre::WrapErr;
use reqwest::{header, Client, RequestBuilder};
use serde::{Deserialize, Serialize};

use crate::{
    clean_title, http, session_data, Fetch, LoginCapture, ManualLogin, OwnedGame, PendingLogin,
    Session, StoreId,
};

const SIGN_IN_URL: &str = "https://www.amazon.com/ap/signin";
const RETURN_TO: &str = "https://www.amazon.com";
const REGISTER_URL: &str = "https://api.amazon.com/auth/register";
const TOKEN_URL: &str = "https://api.amazon.com/auth/token";
const DEREGISTER_URL: &str = "https://api.amazon.com/auth/deregister";
const ENTITLEMENTS_URL: &str = "https://gaming.amazon.com/api/distribution/entitlements";
const ENTITLEMENTS_TARGET: &str =
    "com.amazon.animusdistributionservice.entitlement.AnimusEntitlementsService.GetEntitlements";
/// Key id the launcher sends with GetEntitlements (nile `api/library.py`, Playnite
/// `AmazonAccountClient.cs`; taken from `Amazon.Fuel.Plugin.Entitlement.dll`).
const ENTITLEMENTS_KEY_ID: &str = "d5dc8b8b-86c8-4fc4-ae93-18c0def5314d";
/// Amazon's device type for the Amazon Games launcher on Windows.
const DEVICE_TYPE: &str = "A2UMVHOX7UP4V7";
const MARKETPLACE_ID: &str = "ATVPDKIKX0DER";
const ASSOC_HANDLE: &str = "amzn_sonic_games_launcher";
const APP_NAME: &str = "AGSLauncher for Windows";
const APP_VERSION: &str = "1.0.0";
const AUTH_USER_AGENT: &str = "AGSLauncher/1.0.0";
const LAUNCHER_USER_AGENT: &str = "com.amazon.agslauncher.win/3.0.9495.3";
const CODE_PARAM: &str = "openid.oa2.authorization_code";
/// Entitlements of this product line are in-game loot, not games (Playnite skips them).
const SKIPPED_PRODUCT_LINE: &str = "Twitch:FuelEntitlement";
/// Refresh this many seconds before the access token runs out.
const EXPIRY_MARGIN_SECS: u64 = 120;
/// More pages than this is taken as a `nextToken` that never ends: an error, not the end of
/// the list.
const MAX_PAGES: usize = 200;
/// The shortest pasted value that is taken as a bare authorization code. A guess on the
/// safe side: the codes in Login with Amazon's documentation are 20 characters long.
const MIN_CODE_CHARS: usize = 10;
/// Exception names Amazon's services answer a token they do not accept with, in the
/// `__type` of a JSON error body (`com.amazon.coral.service#UnrecognizedClientException`).
const AUTH_EXCEPTIONS: [&str; 5] = [
    "UnrecognizedClientException",
    "ExpiredTokenException",
    "InvalidTokenException",
    "NotAuthorizedException",
    "AccessDeniedException",
];

/// State carried from [`begin_login`] to [`complete_login`].
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct LoginState {
    code_verifier: String,
    device_serial: String,
    client_id: String,
}

// The PKCE verifier stays out of `Debug` output.
impl std::fmt::Debug for LoginState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginState")
            .field("code_verifier", &"<hidden>")
            .field("device_serial", &self.device_serial)
            .field("client_id", &self.client_id)
            .finish()
    }
}

/// What [`Session::data`] holds for Amazon. Sessions saved by earlier builds also hold a
/// `client_id` and a `user_id`; nothing reads them, so they are dropped on the next save.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmazonSession {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix time in seconds; 0 when Amazon did not say.
    #[serde(default)]
    pub expires_at: u64,
    /// The serial this device was registered with; also the base of `hardwareHash`.
    pub device_serial: String,
}

// The tokens stay out of `Debug` output, so a stray `{:?}` cannot put them in a log.
impl std::fmt::Debug for AmazonSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AmazonSession")
            .field("access_token", &"<hidden>")
            .field("refresh_token", &"<hidden>")
            .field("expires_at", &self.expires_at)
            .field("device_serial", &self.device_serial)
            .finish()
    }
}

impl AmazonSession {
    fn needs_refresh(&self, now: u64) -> bool {
        self.access_token.is_empty() || self.expires_at <= now.saturating_add(EXPIRY_MARGIN_SECS)
    }
}

pub fn begin_login() -> PendingLogin {
    let state = new_login_state();
    let start_url = sign_in_url(&state.client_id, &code_challenge(&state.code_verifier));
    PendingLogin {
        page_text_url_prefixes: vec![],
        store: StoreId::Amazon,
        start_url: start_url.clone(),
        done_url_prefixes: done_url_prefixes(),
        cookie_urls: vec![],
        manual: ManualLogin::Paste {
            url: start_url,
            instructions: "Sign in to Amazon in your browser. When it lands on the Amazon home \
                           page, copy the full address from the address bar and paste it here."
                .to_string(),
            placeholder:
                "https://www.amazon.com/?openid.assoc_handle=...&openid.oa2.authorization_code=..."
                    .to_string(),
        },
        user_agent: Some("Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) @amzn/aga-electron-platform/1.0.0 Chrome/78.0.3904.130 Electron/7.1.9 Safari/537.36".to_string()),
        state: serde_json::to_value(&state).unwrap_or_default(),
    }
}

pub async fn complete_login(
    client: &Client,
    pending: &PendingLogin,
    capture: &LoginCapture,
) -> eyre::Result<Session> {
    let state: LoginState = serde_json::from_value(pending.state.clone()).map_err(|err| {
        eyre::format_err!("The Amazon sign-in lost its state ({err}). Start the sign-in again.")
    })?;
    let code = authorization_code(capture)?;
    let body = register_request(&code, &state);
    let response: RegisterResponse = http::send_json(
        client
            .post(REGISTER_URL)
            .header(header::USER_AGENT, AUTH_USER_AGENT)
            .json(&body),
    )
    .await
    .wrap_err("Could not finish the Amazon sign-in")?;
    let (session, account_name) = session_from_registration(response, &state, crate::now_unix())?;
    Ok(Session {
        account_name,
        data: serde_json::to_value(&session)?,
    })
}

pub async fn fetch_owned(
    client: &Client,
    session: &mut Session,
    fetch: &mut Fetch<'_>,
) -> eyre::Result<Vec<OwnedGame>> {
    let mut amazon: AmazonSession = session_data(StoreId::Amazon, session)?;
    let mut renewed = false;
    if amazon.needs_refresh(crate::now_unix()) {
        renew(client, &mut amazon, session, fetch).await?;
        renewed = true;
    }
    match fetch_entitlements(client, &amazon).await {
        // Amazon can end a token before it expires: renew it once and try again.
        Err(err) if !renewed && http::is_login_rejected(&err) => {
            renew(client, &mut amazon, session, fetch).await?;
            fetch_entitlements(client, &amazon).await
        }
        result => result,
    }
}

/// Renews the access token and saves the session right away, so the renewal is not lost
/// when BoilR is closed while the games are listed.
async fn renew(
    client: &Client,
    amazon: &mut AmazonSession,
    session: &mut Session,
    fetch: &mut Fetch<'_>,
) -> eyre::Result<()> {
    refresh(client, amazon).await?;
    session.data = serde_json::to_value(&*amazon)?;
    fetch.checkpoint(session)
}

async fn fetch_entitlements(
    client: &Client,
    amazon: &AmazonSession,
) -> eyre::Result<Vec<OwnedGame>> {
    let hardware_hash = hardware_hash(&amazon.device_serial);
    let hardware_hash = hardware_hash.as_str();
    // The cursor is the `nextToken` of the page before; the first page has none.
    let first_page: Option<String> = None;
    let entitlements = http::paginate("Amazon", first_page, MAX_PAGES, |next_token| async move {
        let body = entitlements_request(hardware_hash, next_token.as_deref());
        let response = send_entitlements_request(
            client
                .post(ENTITLEMENTS_URL)
                .header(header::USER_AGENT, LAUNCHER_USER_AGENT)
                .header("X-Amz-Target", ENTITLEMENTS_TARGET)
                .header("x-amzn-token", &amazon.access_token)
                .header(header::CONTENT_ENCODING, "amz-1.0")
                .json(&body),
        )
        .await?;
        let page = entitlements_page(response)?;
        Ok((page.entitlements, page.next_token.map(Some)))
    })
    .await?;
    Ok(owned_games(entitlements))
}

/// Sends a GetEntitlements request. A failed one is looked at as a whole (status and body),
/// because Amazon answers a token it does not accept with 400 or 403 as well as with 401.
async fn send_entitlements_request(request: RequestBuilder) -> eyre::Result<EntitlementsResponse> {
    let response = request.send().await.map_err(http::request_failed)?;
    let status = response.status();
    if status.is_success() {
        return http::read_json(response).await;
    }
    let url = http::without_query(response.url());
    // The whole body, not the snippet of an error message: that one may be cut off.
    let body = response.text().await.unwrap_or_default();
    Err(entitlements_failure(status.as_u16(), url, &body))
}

/// Removes BoilR from the account's registered devices, as nile does when it logs out.
pub async fn sign_out(client: &Client, session: &Session) -> eyre::Result<()> {
    let mut amazon: AmazonSession = session_data(StoreId::Amazon, session)?;
    if amazon.needs_refresh(crate::now_unix()) {
        refresh(client, &mut amazon).await?;
    }
    http::send(
        client
            .post(DEREGISTER_URL)
            .header(header::USER_AGENT, AUTH_USER_AGENT)
            .bearer_auth(&amazon.access_token)
            .json(&serde_json::json!({
                "request_metadata": { "app_name": APP_NAME, "app_version": APP_VERSION }
            })),
    )
    .await?;
    Ok(())
}

async fn refresh(client: &Client, session: &mut AmazonSession) -> eyre::Result<()> {
    if session.refresh_token.is_empty() {
        return Err(http::login_rejected(
            "The saved Amazon login has no refresh token. Sign in again.",
        ));
    }
    let body = RefreshRequest {
        source_token: &session.refresh_token,
        source_token_type: "refresh_token",
        requested_token_type: "access_token",
        app_name: APP_NAME,
        app_version: APP_VERSION,
    };
    let response: RefreshResponse = http::send_json(
        client
            .post(TOKEN_URL)
            .header(header::USER_AGENT, AUTH_USER_AGENT)
            .json(&body),
    )
    .await
    .map_err(refresh_failure)?;
    apply_refresh(session, response, crate::now_unix())
}

// ---------------------------------------------------------------------------------------
// Which answers mean "the login was refused" (pure)
// ---------------------------------------------------------------------------------------

/// Whether a failed token renewal means Amazon refused the refresh token. The token service
/// answers one it no longer accepts the OAuth way, with HTTP 400 (`invalid_grant`) or 401.
/// Anything else (no connection, a timeout, a rate limit, 5xx) says nothing about the login.
fn refresh_was_refused(status: Option<u16>) -> bool {
    matches!(status, Some(400 | 401))
}

/// A failed token renewal as the error to report: a rejected login only when Amazon refused
/// the refresh token, otherwise the error as it is, with context.
fn refresh_failure(err: eyre::Report) -> eyre::Report {
    if refresh_was_refused(http::status_of(&err)) {
        http::login_rejected(format!(
            "Amazon no longer accepts the saved login. Sign in again. ({err})"
        ))
    } else {
        err.wrap_err("Could not refresh the Amazon login")
    }
}

/// Whether an error's `__type` or `message` names one of the [`AUTH_EXCEPTIONS`].
fn names_auth_exception(error_type: Option<&str>, message: Option<&str>) -> bool {
    [error_type, message].into_iter().flatten().any(|text| {
        let text = text.to_ascii_lowercase();
        AUTH_EXCEPTIONS
            .iter()
            .any(|name| text.contains(&name.to_ascii_lowercase()))
    })
}

/// Whether a failed response of Amazon's game services means the access token was refused.
///
/// 401 always does. Amazon's services also answer a bad or expired token with 400 or 403,
/// and then a JSON body names an auth exception. A 400 or 403 without one (a bot block, a
/// captive portal, a web page) says nothing about the login, and neither does any other
/// status.
fn login_was_refused(status: u16, body: &str) -> bool {
    match status {
        401 => true,
        400 | 403 => serde_json::from_str::<serde_json::Value>(body).is_ok_and(|error| {
            let text = |name: &str| error.get(name).and_then(|value| value.as_str());
            names_auth_exception(text("__type"), text("message").or(text("Message")))
        }),
        _ => false,
    }
}

/// The error for a failed GetEntitlements response: a rejected login when Amazon refused
/// the token (so the token is renewed once and the request tried again), otherwise the
/// plain HTTP error with its status.
fn entitlements_failure(status: u16, url: String, body: &str) -> eyre::Report {
    let failure = eyre::Report::new(http::HttpStatus {
        status,
        url,
        body: http::snippet(body),
    });
    // 401 is a rejected login as it is.
    if status != 401 && login_was_refused(status, body) {
        http::login_rejected(format!(
            "Amazon did not accept the saved login. Sign in again. ({failure})"
        ))
    } else {
        failure
    }
}

// ---------------------------------------------------------------------------------------
// Login pieces (pure)
// ---------------------------------------------------------------------------------------

fn new_login_state() -> LoginState {
    use rand::RngCore;
    let mut rng = rand::thread_rng();
    let mut verifier_bytes = [0u8; 32];
    rng.fill_bytes(&mut verifier_bytes);
    let mut serial_bytes = [0u8; 16];
    rng.fill_bytes(&mut serial_bytes);
    let device_serial = hex::encode_upper(serial_bytes);
    LoginState {
        code_verifier: code_verifier_from_bytes(&verifier_bytes),
        client_id: client_id(&device_serial),
        device_serial,
    }
}

/// PKCE verifier: 32 random bytes, base64url without padding (43 characters), like nile.
fn code_verifier_from_bytes(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// PKCE S256 challenge: base64url (no padding) of the SHA-256 of the verifier.
fn code_challenge(verifier: &str) -> String {
    use base64::Engine;
    use sha2::Digest;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier))
}

/// The OAuth client id of a device: hex of `<serial>#<device type>` (nile `generate_client_id`).
fn client_id(device_serial: &str) -> String {
    hex::encode(format!("{device_serial}#{DEVICE_TYPE}"))
}

/// `hardwareHash` for GetEntitlements: upper-case hex SHA-256 of the device serial (nile).
fn hardware_hash(device_serial: &str) -> String {
    use sha2::Digest;
    hex::encode_upper(sha2::Sha256::digest(device_serial))
}

fn sign_in_url(client_id: &str, challenge: &str) -> String {
    let device_client_id = format!("device:{client_id}");
    let params = [
        ("openid.ns", "http://specs.openid.net/auth/2.0"),
        (
            "openid.claimed_id",
            "http://specs.openid.net/auth/2.0/identifier_select",
        ),
        (
            "openid.identity",
            "http://specs.openid.net/auth/2.0/identifier_select",
        ),
        ("openid.mode", "checkid_setup"),
        ("openid.oa2.scope", "device_auth_access"),
        ("openid.ns.oa2", "http://www.amazon.com/ap/ext/oauth/2"),
        ("openid.oa2.response_type", "code"),
        ("openid.oa2.code_challenge_method", "S256"),
        ("openid.oa2.client_id", device_client_id.as_str()),
        ("language", "en_US"),
        ("marketPlaceId", MARKETPLACE_ID),
        ("openid.return_to", RETURN_TO),
        ("openid.pape.max_auth_age", "0"),
        ("openid.assoc_handle", ASSOC_HANDLE),
        ("pageId", ASSOC_HANDLE),
        ("openid.oa2.code_challenge", challenge),
    ];
    match url::Url::parse_with_params(SIGN_IN_URL, params) {
        Ok(url) => url.to_string(),
        // SIGN_IN_URL is a valid constant, so this does not happen.
        Err(_) => SIGN_IN_URL.to_string(),
    }
}

/// Amazon sends the browser back to `openid.return_to` with the OpenID response appended:
/// `https://www.amazon.com/?openid.assoc_handle=amzn_sonic_games_launcher&...`. Any query on
/// the home page counts, whichever parameter comes first; [`authorization_code`] finds the
/// code wherever it is. The sign-in pages themselves live under `/ap/`, so they never match,
/// and neither does the home page without a query.
fn done_url_prefixes() -> Vec<String> {
    vec![
        "https://www.amazon.com/?".to_string(),
        "https://amazon.com/?".to_string(),
    ]
}

/// The authorization code from the redirect URL (webview) or from what the user pasted:
/// the address Amazon sent the browser to, with or without its `https://`, the query of
/// that address, or the bare code.
fn authorization_code(capture: &LoginCapture) -> eyre::Result<String> {
    let raw = capture
        .final_url
        .as_deref()
        .or(capture.pasted.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| eyre::format_err!("The Amazon sign-in did not return anything."))?;

    let query = match raw.split_once('?') {
        Some((_, query)) => Some(query),
        None if raw.contains('=') => Some(raw),
        None => None,
    };
    let Some(query) = query else {
        if looks_like_code(raw) {
            return Ok(raw.to_string());
        }
        return Err(eyre::format_err!(
            "That does not look like the address Amazon sent you to. Sign in, then paste the \
             full address from the browser's address bar."
        ));
    };
    let query = query.split('#').next().unwrap_or_default();
    let param = |name: &str| {
        url::form_urlencoded::parse(query.as_bytes())
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
            .filter(|value| !value.is_empty())
    };
    if let Some(code) = param(CODE_PARAM) {
        return Ok(code);
    }
    if param("openid.mode").as_deref() == Some("cancel") {
        return Err(eyre::format_err!("The Amazon sign-in was cancelled."));
    }
    Err(eyre::format_err!(
        "That address has no Amazon sign-in code ({CODE_PARAM}). Sign in again and copy \
         the full address the browser lands on after signing in."
    ))
}

/// Whether a pasted value without a query can be an authorization code on its own: long
/// enough, and made of the characters a code has. An address without its query
/// (`www.amazon.com/`) has dots or slashes and is not one.
fn looks_like_code(value: &str) -> bool {
    value.len() >= MIN_CODE_CHARS
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

#[derive(Serialize)]
struct RegisterRequest<'a> {
    auth_data: RegisterAuthData<'a>,
    registration_data: RegistrationData<'a>,
    requested_extensions: [&'a str; 2],
    requested_token_type: [&'a str; 2],
    user_context_map: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize)]
struct RegisterAuthData<'a> {
    authorization_code: &'a str,
    client_domain: &'a str,
    client_id: &'a str,
    code_algorithm: &'a str,
    code_verifier: &'a str,
    use_global_authentication: bool,
}

#[derive(Serialize)]
struct RegistrationData<'a> {
    app_name: &'a str,
    app_version: &'a str,
    device_model: &'a str,
    device_name: Option<&'a str>,
    device_serial: &'a str,
    device_type: &'a str,
    domain: &'a str,
    os_version: &'a str,
}

fn register_request<'a>(code: &'a str, state: &'a LoginState) -> RegisterRequest<'a> {
    RegisterRequest {
        auth_data: RegisterAuthData {
            authorization_code: code,
            client_domain: "DeviceLegacy",
            client_id: &state.client_id,
            code_algorithm: "SHA-256",
            code_verifier: &state.code_verifier,
            use_global_authentication: false,
        },
        registration_data: RegistrationData {
            app_name: APP_NAME,
            app_version: APP_VERSION,
            device_model: "Windows",
            device_name: None,
            device_serial: &state.device_serial,
            device_type: DEVICE_TYPE,
            domain: "Device",
            os_version: "10.0.19044.0",
        },
        requested_extensions: ["customer_info", "device_info"],
        requested_token_type: ["bearer", "mac_dms"],
        user_context_map: serde_json::Map::new(),
    }
}

// The registration response holds tokens, so the types it is read into have no `Debug`.
#[derive(Deserialize)]
struct RegisterResponse {
    response: Option<RegisterResponseBody>,
}

#[derive(Deserialize)]
struct RegisterResponseBody {
    success: Option<RegisterSuccess>,
    error: Option<AmazonError>,
}

#[derive(Debug, Deserialize)]
struct AmazonError {
    code: Option<String>,
    message: Option<String>,
}

#[derive(Deserialize)]
struct RegisterSuccess {
    tokens: RegisterTokens,
    #[serde(default)]
    extensions: Option<RegisterExtensions>,
}

#[derive(Deserialize)]
struct RegisterTokens {
    bearer: BearerToken,
}

#[derive(Deserialize)]
struct BearerToken {
    access_token: String,
    refresh_token: String,
    #[serde(default)]
    expires_in: Option<Seconds>,
}

#[derive(Debug, Deserialize)]
struct RegisterExtensions {
    #[serde(default)]
    customer_info: Option<CustomerInfo>,
    #[serde(default)]
    device_info: Option<DeviceInfo>,
}

#[derive(Debug, Deserialize)]
struct CustomerInfo {
    name: Option<String>,
    given_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeviceInfo {
    device_serial_number: Option<String>,
}

/// Amazon sends `expires_in` as a number in some responses and as a string in others.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Seconds {
    Number(u64),
    Text(String),
}

impl Seconds {
    fn value(&self) -> Option<u64> {
        match self {
            Seconds::Number(n) => Some(*n),
            Seconds::Text(s) => s.trim().parse().ok(),
        }
    }
}

fn expires_at(expires_in: Option<&Seconds>, now: u64) -> u64 {
    expires_in
        .and_then(Seconds::value)
        .map(|secs| now.saturating_add(secs))
        .unwrap_or_default()
}

/// Turns the device registration into the stored session and the account name.
fn session_from_registration(
    response: RegisterResponse,
    state: &LoginState,
    now: u64,
) -> eyre::Result<(AmazonSession, Option<String>)> {
    let body = response
        .response
        .ok_or_else(|| eyre::format_err!("Amazon's device registration returned no response."))?;
    let success = match (body.success, body.error) {
        (Some(success), _) => success,
        (None, Some(error)) => {
            return Err(eyre::format_err!(
                "Amazon refused to register BoilR: {} ({}). Sign in again.",
                error.message.unwrap_or_else(|| "no reason given".into()),
                error.code.unwrap_or_else(|| "no code".into())
            ))
        }
        (None, None) => {
            return Err(eyre::format_err!(
                "Amazon's device registration returned neither tokens nor an error."
            ))
        }
    };
    let bearer = success.tokens.bearer;
    if bearer.access_token.is_empty() || bearer.refresh_token.is_empty() {
        return Err(eyre::format_err!(
            "Amazon's device registration returned empty tokens."
        ));
    }
    let extensions = success.extensions;
    let (customer, device) = match extensions {
        Some(ext) => (ext.customer_info, ext.device_info),
        None => (None, None),
    };
    let device_serial = device
        .and_then(|d| d.device_serial_number)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| state.device_serial.clone());
    let account_name = customer.and_then(|c| {
        c.name
            .filter(|n| !n.trim().is_empty())
            .or(c.given_name.filter(|n| !n.trim().is_empty()))
    });
    let session = AmazonSession {
        expires_at: expires_at(bearer.expires_in.as_ref(), now),
        access_token: bearer.access_token,
        refresh_token: bearer.refresh_token,
        device_serial,
    };
    Ok((session, account_name))
}

#[derive(Serialize)]
struct RefreshRequest<'a> {
    source_token: &'a str,
    source_token_type: &'a str,
    requested_token_type: &'a str,
    app_name: &'a str,
    app_version: &'a str,
}

/// Holds a token, so it has no `Debug`.
#[derive(Deserialize)]
struct RefreshResponse {
    access_token: Option<String>,
    #[serde(default)]
    expires_in: Option<Seconds>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

fn apply_refresh(
    session: &mut AmazonSession,
    response: RefreshResponse,
    now: u64,
) -> eyre::Result<()> {
    if let Some(token) = response.access_token.filter(|t| !t.is_empty()) {
        session.access_token = token;
        session.expires_at = expires_at(response.expires_in.as_ref(), now);
        return Ok(());
    }
    match response.error_description.or(response.error) {
        // Amazon said why it gives no token: it refused the refresh token.
        Some(reason) => Err(http::login_rejected(format!(
            "Amazon did not refresh the login ({reason}). Sign in again."
        ))),
        // An answer that is neither a token nor a refusal says nothing about the login.
        None => Err(eyre::format_err!(
            "Unexpected answer from Amazon when refreshing the login: no access token in it."
        )),
    }
}

// ---------------------------------------------------------------------------------------
// Library pieces (pure)
// ---------------------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EntitlementsRequest<'a> {
    #[serde(rename = "Operation")]
    operation: &'a str,
    client_id: &'a str,
    sync_point: Option<u64>,
    next_token: Option<&'a str>,
    max_results: u32,
    product_id_filter: Option<&'a str>,
    key_id: &'a str,
    hardware_hash: &'a str,
}

fn entitlements_request<'a>(
    hardware_hash: &'a str,
    next_token: Option<&'a str>,
) -> EntitlementsRequest<'a> {
    EntitlementsRequest {
        operation: "GetEntitlements",
        client_id: "Sonic",
        sync_point: None,
        next_token,
        max_results: 50,
        product_id_filter: None,
        key_id: ENTITLEMENTS_KEY_ID,
        hardware_hash,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntitlementsResponse {
    entitlements: Option<Vec<Entitlement>>,
    next_token: Option<String>,
    #[serde(rename = "__type")]
    error_type: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entitlement {
    id: Option<String>,
    product: Option<Product>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Product {
    id: Option<String>,
    title: Option<String>,
    product_line: Option<String>,
    sku: Option<String>,
    product_detail: Option<ProductDetail>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProductDetail {
    icon_url: Option<String>,
}

struct EntitlementsPage {
    entitlements: Vec<Entitlement>,
    next_token: Option<String>,
}

fn entitlements_page(response: EntitlementsResponse) -> eyre::Result<EntitlementsPage> {
    let Some(entitlements) = response.entitlements else {
        let refused =
            names_auth_exception(response.error_type.as_deref(), response.message.as_deref());
        let reason = format!(
            "{}{}",
            response
                .error_type
                .map(|t| format!("{t}: "))
                .unwrap_or_default(),
            response.message.unwrap_or_else(|| "no reason given".into())
        );
        return Err(if refused {
            http::login_rejected(format!(
                "Amazon did not accept the saved login ({reason}). Sign in again."
            ))
        } else {
            eyre::format_err!("Amazon did not send the list of games: {reason}")
        });
    };
    Ok(EntitlementsPage {
        entitlements,
        next_token: response.next_token.filter(|t| !t.is_empty()),
    })
}

/// Games from entitlements: skips in-game loot and entries without a product id or title,
/// and keeps the first entitlement of each product.
fn owned_games(entitlements: Vec<Entitlement>) -> Vec<OwnedGame> {
    let mut seen = std::collections::HashSet::new();
    let mut games = vec![];
    for entitlement in entitlements {
        let Some(product) = entitlement.product else {
            continue;
        };
        if product.product_line.as_deref() == Some(SKIPPED_PRODUCT_LINE) {
            continue;
        }
        let Some(id) = product.id.filter(|id| !id.trim().is_empty()) else {
            continue;
        };
        let Some(title) = product
            .title
            .map(|t| clean_title(&t))
            .filter(|t| !t.is_empty())
        else {
            continue;
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        let mut game = OwnedGame::new(StoreId::Amazon, id, title);
        if let Some(entitlement_id) = entitlement.id.filter(|s| !s.is_empty()) {
            game = game.with_extra("entitlement_id", entitlement_id);
        }
        if let Some(sku) = product.sku.filter(|s| !s.is_empty()) {
            game = game.with_extra("sku", sku);
        }
        if let Some(icon) = product
            .product_detail
            .and_then(|d| d.icon_url)
            .filter(|s| !s.is_empty())
        {
            game = game.with_extra("icon_url", icon);
        }
        games.push(game);
    }
    games
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> LoginState {
        LoginState {
            code_verifier: "verifier".into(),
            device_serial: "0123456789ABCDEF0123456789ABCDEF".into(),
            client_id: client_id("0123456789ABCDEF0123456789ABCDEF"),
        }
    }

    fn capture_url(url: &str) -> LoginCapture {
        LoginCapture {
            final_url: Some(url.into()),
            ..Default::default()
        }
    }

    fn capture_paste(text: &str) -> LoginCapture {
        LoginCapture {
            pasted: Some(text.into()),
            ..Default::default()
        }
    }

    /// An error body as Amazon's services send it.
    fn exception(name: &str) -> String {
        format!(r#"{{"__type":"com.amazon.coral.service#{name}","message":"Invalid token"}}"#)
    }

    fn status(status: u16) -> eyre::Report {
        eyre::Report::new(http::HttpStatus {
            status,
            url: TOKEN_URL.into(),
            body: r#"{"error":"invalid_grant"}"#.into(),
        })
    }

    #[test]
    fn pkce_matches_rfc_7636_example() {
        // RFC 7636 appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            code_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn login_state_has_the_expected_shape() {
        let state = new_login_state();
        assert_eq!(state.code_verifier.len(), 43);
        assert!(state
            .code_verifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_eq!(state.device_serial.len(), 32);
        assert!(state
            .device_serial
            .chars()
            .all(|c| c.is_ascii_digit() || c.is_ascii_uppercase()));
        assert_eq!(state.client_id, client_id(&state.device_serial));
        let challenge = code_challenge(&state.code_verifier);
        assert_eq!(challenge.len(), 43);
        assert!(!challenge.contains('='));
        assert_ne!(new_login_state(), state);
    }

    #[test]
    fn client_id_is_hex_of_serial_and_device_type() {
        // Playnite's fixed client id decodes to this serial.
        assert_eq!(
            client_id("73db8d286c2a924d2e77e1af67766645"),
            "3733646238643238366332613932346432653737653161663637373636363435234132554d56484f58375550345637"
        );
    }

    #[test]
    fn hardware_hash_is_upper_hex_sha256() {
        assert_eq!(
            hardware_hash("abc"),
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        );
    }

    #[test]
    fn sign_in_url_carries_client_and_challenge() {
        let pending = begin_login();
        let state: LoginState =
            serde_json::from_value(pending.state.clone()).unwrap_or_else(|_| LoginState {
                code_verifier: String::new(),
                device_serial: String::new(),
                client_id: String::new(),
            });
        assert!(!state.code_verifier.is_empty());
        let url = &pending.start_url;
        assert!(url.starts_with("https://www.amazon.com/ap/signin?"));
        assert_eq!(
            http::query_param(url, "openid.oa2.client_id"),
            Some(format!("device:{}", state.client_id))
        );
        assert_eq!(
            http::query_param(url, "openid.oa2.code_challenge"),
            Some(code_challenge(&state.code_verifier))
        );
        assert_eq!(
            http::query_param(url, "openid.oa2.code_challenge_method").as_deref(),
            Some("S256")
        );
        assert_eq!(
            http::query_param(url, "openid.assoc_handle").as_deref(),
            Some("amzn_sonic_games_launcher")
        );
        assert!(matches!(pending.manual, ManualLogin::Paste { .. }));
    }

    #[test]
    fn done_prefixes_match_only_the_final_redirect() {
        let pending = begin_login();
        let matches = |url: &str| {
            pending
                .done_url_prefixes
                .iter()
                .any(|p| url.starts_with(p.as_str()))
        };
        assert!(!matches(&pending.start_url));
        assert!(!matches("https://www.amazon.com/ap/signin?openid.ns=x"));
        assert!(!matches("https://www.amazon.com/ap/mfa?arb=x"));
        assert!(!matches("https://www.amazon.com/ap/cvf/request?arb=x"));
        assert!(!matches("https://www.amazon.com/"));
        assert!(!matches(
            "https://www.amazon.com/gp/help/customer/display.html?nodeId=1"
        ));
        assert!(!matches(
            "https://www.amazon.com.example/?openid.mode=id_res"
        ));
        assert!(matches(
            "https://www.amazon.com/?openid.assoc_handle=amzn_sonic_games_launcher&openid.oa2.authorization_code=ANabc"
        ));
        // Whichever parameter Amazon puts first.
        assert!(matches(
            "https://www.amazon.com/?ref_=nav_signin&openid.oa2.authorization_code=ANabc"
        ));
        assert!(matches(
            "https://amazon.com/?language=en_US&openid.oa2.authorization_code=ANabc"
        ));
    }

    #[test]
    fn reads_the_code_from_the_redirect() -> eyre::Result<()> {
        let url = "https://www.amazon.com/?openid.assoc_handle=amzn_sonic_games_launcher\
                   &openid.mode=id_res&openid.oa2.authorization_code=ANcodeXYZ&openid.ns=x";
        assert_eq!(authorization_code(&capture_url(url))?, "ANcodeXYZ");
        assert_eq!(
            authorization_code(&capture_paste(&format!("  {url}\n")))?,
            "ANcodeXYZ"
        );
        // The code is found wherever it is in the query.
        let other_first =
            "https://www.amazon.com/?ref_=nav&openid.oa2.authorization_code=ANcodeXYZ#top";
        assert_eq!(authorization_code(&capture_url(other_first))?, "ANcodeXYZ");
        Ok(())
    }

    #[test]
    fn reads_the_code_from_a_pasted_address_without_its_scheme() -> eyre::Result<()> {
        // Browsers hide the scheme in the address bar, and some copy the address without it.
        for pasted in [
            "www.amazon.com/?openid.mode=id_res&openid.oa2.authorization_code=ANcodeXYZ",
            "amazon.com/?openid.oa2.authorization_code=ANcodeXYZ&openid.ns=x",
            "?openid.mode=id_res&openid.oa2.authorization_code=ANcodeXYZ",
            "openid.mode=id_res&openid.oa2.authorization_code=ANcodeXYZ",
        ] {
            assert_eq!(
                authorization_code(&capture_paste(pasted))?,
                "ANcodeXYZ",
                "{pasted}"
            );
        }
        Ok(())
    }

    #[test]
    fn a_bare_value_is_a_code_only_when_it_looks_like_one() -> eyre::Result<()> {
        // The length and shape of the codes in Amazon's examples.
        let code = "ANdNAVhyhqirUelHGEHA";
        assert_eq!(authorization_code(&capture_paste(code))?, code);
        assert_eq!(
            authorization_code(&capture_paste(&format!(" {code}\n")))?,
            code
        );
        assert!(looks_like_code("AN-dNAVhy_hqir"));

        for not_a_code in [
            // An address without its query, with or without its scheme.
            "www.amazon.com/",
            "www.amazon.com",
            "amazon.com",
            "https://www.amazon.com/",
            "https://www.amazon.com/ap/signin",
            // Too short, or with characters no code has.
            "ANcode",
            "not a code at all",
            "ANdNAVhy|hqirUelHGEHA",
            "ANdNAVhy/hqirUelHGEHA",
            "ANdNAVhy.hqirUelHGEHA",
        ] {
            let result = authorization_code(&capture_paste(not_a_code));
            assert!(
                result.is_err_and(|e| e.to_string().contains("paste the full address")),
                "{not_a_code}"
            );
        }
        Ok(())
    }

    #[test]
    fn rejects_redirects_without_a_code() {
        let cancelled = capture_url("https://www.amazon.com/?openid.mode=cancel");
        let err = authorization_code(&cancelled).map_err(|e| e.to_string());
        assert_eq!(err, Err("The Amazon sign-in was cancelled.".to_string()));
        let no_code =
            authorization_code(&capture_url("https://www.amazon.com/?openid.mode=id_res"));
        assert!(no_code.is_err_and(|e| e.to_string().contains("no Amazon sign-in code")));
        let empty_code = authorization_code(&capture_paste(
            "www.amazon.com/?openid.oa2.authorization_code=&openid.mode=id_res",
        ));
        assert!(empty_code.is_err_and(|e| e.to_string().contains("no Amazon sign-in code")));
        assert!(authorization_code(&LoginCapture::default()).is_err());
        assert!(authorization_code(&capture_paste("   ")).is_err());
    }

    #[test]
    fn register_request_has_launcher_shape() -> eyre::Result<()> {
        let state = state();
        let json = serde_json::to_value(register_request("ANcode", &state))?;
        let at = |pointer: &str| json.pointer(pointer).cloned().unwrap_or_default();
        assert_eq!(at("/auth_data/authorization_code"), "ANcode");
        assert_eq!(at("/auth_data/client_id"), state.client_id.as_str());
        assert_eq!(at("/auth_data/code_verifier"), "verifier");
        assert_eq!(at("/auth_data/code_algorithm"), "SHA-256");
        assert_eq!(at("/auth_data/client_domain"), "DeviceLegacy");
        assert_eq!(at("/registration_data/device_type"), "A2UMVHOX7UP4V7");
        assert_eq!(
            at("/registration_data/device_serial"),
            state.device_serial.as_str()
        );
        assert_eq!(
            at("/requested_token_type"),
            serde_json::json!(["bearer", "mac_dms"])
        );
        assert_eq!(at("/user_context_map"), serde_json::json!({}));
        Ok(())
    }

    #[test]
    fn parses_a_device_registration() -> eyre::Result<()> {
        let response: RegisterResponse =
            serde_json::from_str(include_str!("../testdata/amazon/register_success.json"))?;
        let (session, name) = session_from_registration(response, &state(), 1000)?;
        assert_eq!(name.as_deref(), Some("Jane Player"));
        assert_eq!(session.access_token, "Atna|access");
        assert_eq!(session.refresh_token, "Atnr|refresh");
        assert_eq!(session.expires_at, 4600);
        assert_eq!(session.device_serial, "SERIALFROMAMAZON");
        assert!(!session.needs_refresh(1000));
        assert!(session.needs_refresh(4500));
        Ok(())
    }

    #[test]
    fn reports_a_refused_registration() -> eyre::Result<()> {
        let response: RegisterResponse =
            serde_json::from_str(include_str!("../testdata/amazon/register_error.json"))?;
        let err = session_from_registration(response, &state(), 0)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(err.contains("Authorization code is invalid"), "{err}");
        assert!(err.contains("InvalidValue"), "{err}");
        Ok(())
    }

    #[test]
    fn applies_a_token_refresh() -> eyre::Result<()> {
        let response: RegisterResponse =
            serde_json::from_str(include_str!("../testdata/amazon/register_success.json"))?;
        let (mut session, _) = session_from_registration(response, &state(), 0)?;
        let refresh: RefreshResponse =
            serde_json::from_str(include_str!("../testdata/amazon/refresh.json"))?;
        apply_refresh(&mut session, refresh, 10_000)?;
        assert_eq!(session.access_token, "Atna|fresh");
        assert_eq!(session.refresh_token, "Atnr|refresh");
        assert_eq!(session.expires_at, 13_600);

        let failed: RefreshResponse = serde_json::from_str(
            r#"{"error":"invalid_grant","error_description":"The request has an invalid grant parameter"}"#,
        )?;
        let refused = apply_refresh(&mut session, failed, 0);
        assert!(refused.is_err_and(|e| {
            http::is_login_rejected(&e) && e.to_string().contains("invalid grant")
        }));
        assert_eq!(session.access_token, "Atna|fresh");

        // An answer with neither a token nor a refusal is not a reason to sign in again.
        for body in ["{}", r#"{"access_token":""}"#, r#"{"token_type":"bearer"}"#] {
            let odd: RefreshResponse = serde_json::from_str(body)?;
            let result = apply_refresh(&mut session, odd, 0);
            assert!(
                result.is_err_and(|e| {
                    !http::is_login_rejected(&e) && e.to_string().contains("Unexpected answer")
                }),
                "{body}"
            );
        }
        assert_eq!(session.access_token, "Atna|fresh");
        Ok(())
    }

    #[test]
    fn only_a_refused_refresh_token_is_a_rejected_login() {
        assert!(refresh_was_refused(Some(400)));
        assert!(refresh_was_refused(Some(401)));
        for other in [403, 404, 408, 429, 500, 502, 503] {
            assert!(!refresh_was_refused(Some(other)), "{other}");
        }
        assert!(!refresh_was_refused(None));

        for refused in [400, 401] {
            let err = refresh_failure(status(refused));
            assert!(http::is_login_rejected(&err), "{refused}");
            assert!(err.to_string().contains("Sign in again"), "{err}");
        }
        // An outage stays an outage, with the status still there to read.
        let outage = refresh_failure(status(503));
        assert!(http::is_transient(&outage) && !http::is_login_rejected(&outage));
        assert_eq!(http::status_of(&outage), Some(503));
        assert!(format!("{outage:#}").contains("Could not refresh the Amazon login"));
        let offline = refresh_failure(http::transient("no connection"));
        assert!(http::is_transient(&offline) && !http::is_login_rejected(&offline));
        let blocked = refresh_failure(status(403));
        assert!(!http::is_login_rejected(&blocked) && !http::is_transient(&blocked));
    }

    #[test]
    fn a_refused_token_is_told_from_other_failures() {
        // 401 is a refused token whatever the body.
        assert!(login_was_refused(401, ""));
        assert!(login_was_refused(401, "<html>Unauthorized</html>"));
        // Amazon's services answer a bad or expired token with 400 or 403 and name why.
        for name in AUTH_EXCEPTIONS {
            for status in [400, 403] {
                assert!(
                    login_was_refused(status, &exception(name)),
                    "{status} {name}"
                );
            }
        }
        // The exception can be named without its namespace, in any case, or in the message.
        assert!(login_was_refused(
            400,
            r#"{"__type":"ExpiredTokenException"}"#
        ));
        assert!(login_was_refused(
            403,
            r#"{"__type":"x#accessdeniedexception"}"#
        ));
        assert!(login_was_refused(
            400,
            r#"{"message":"com.amazon.coral.service#InvalidTokenException"}"#
        ));
        assert!(login_was_refused(
            403,
            r#"{"Message":"NotAuthorizedException: token expired"}"#
        ));

        // Other errors of the service are not about the login.
        assert!(!login_was_refused(400, &exception("ValidationException")));
        assert!(!login_was_refused(400, &exception("ThrottlingException")));
        assert!(!login_was_refused(403, r#"{"message":"Forbidden"}"#));
        // A bot block, a captive portal or an empty answer is not either.
        assert!(!login_was_refused(403, "<html>Request blocked</html>"));
        assert!(!login_was_refused(403, ""));
        assert!(!login_was_refused(400, r#"{"__type": 3}"#));
        assert!(!login_was_refused(
            400,
            r#"["UnrecognizedClientException"]"#
        ));
        // No other status is, whatever the body says.
        let named = exception("UnrecognizedClientException");
        for other in [200, 404, 408, 429, 500, 502, 503] {
            assert!(!login_was_refused(other, &named), "{other}");
        }
    }

    #[test]
    fn a_refused_token_fails_the_entitlements_call_as_a_rejected_login() {
        let failure = |status: u16, body: &str| {
            entitlements_failure(status, ENTITLEMENTS_URL.to_string(), body)
        };
        // These make the token be renewed once and the request tried again.
        for status in [400, 401, 403] {
            let err = failure(status, &exception("UnrecognizedClientException"));
            assert!(http::is_login_rejected(&err), "{status}");
            assert!(err.to_string().contains("Sign in again"), "{err}");
            assert!(err.to_string().contains(ENTITLEMENTS_URL), "{err}");
        }
        assert!(http::is_login_rejected(&failure(401, "")));

        // These do not, and keep their status.
        let blocked = failure(403, "<html>Request blocked</html>");
        assert!(!http::is_login_rejected(&blocked) && !http::is_transient(&blocked));
        assert_eq!(http::status_of(&blocked), Some(403));
        let bad_request = failure(400, &exception("ValidationException"));
        assert!(!http::is_login_rejected(&bad_request));
        for outage in [429, 500, 503] {
            let err = failure(outage, &exception("UnrecognizedClientException"));
            assert!(
                http::is_transient(&err) && !http::is_login_rejected(&err),
                "{outage}"
            );
        }
        // The body in the message has its secrets taken out.
        let leaky = failure(
            403,
            r#"{"__type":"AccessDeniedException","message":"denied","token":"SECRET"}"#,
        );
        assert!(http::is_login_rejected(&leaky));
        assert!(!leaky.to_string().contains("SECRET"), "{leaky}");
    }

    #[test]
    fn entitlements_request_has_launcher_shape() -> eyre::Result<()> {
        let json = serde_json::to_value(entitlements_request("HASH", Some("next")))?;
        assert_eq!(
            json,
            serde_json::json!({
                "Operation": "GetEntitlements",
                "clientId": "Sonic",
                "syncPoint": null,
                "nextToken": "next",
                "maxResults": 50,
                "productIdFilter": null,
                "keyId": "d5dc8b8b-86c8-4fc4-ae93-18c0def5314d",
                "hardwareHash": "HASH"
            })
        );
        Ok(())
    }

    #[test]
    fn parses_entitlement_pages_into_games() -> eyre::Result<()> {
        let first: EntitlementsResponse =
            serde_json::from_str(include_str!("../testdata/amazon/entitlements_page1.json"))?;
        let second: EntitlementsResponse =
            serde_json::from_str(include_str!("../testdata/amazon/entitlements_page2.json"))?;
        let first = entitlements_page(first)?;
        let second = entitlements_page(second)?;
        assert_eq!(first.next_token.as_deref(), Some("page-2-token"));
        assert_eq!(second.next_token, None);

        let mut all = first.entitlements;
        all.extend(second.entitlements);
        let games = owned_games(all);
        let ids: Vec<&str> = games.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "amzn1.adg.product.11111111-1111-1111-1111-111111111111",
                "amzn1.adg.product.33333333-3333-3333-3333-333333333333",
            ]
        );
        let [fallout, other] = games.as_slice() else {
            eyre::bail!("expected two games, got {}", games.len());
        };
        assert_eq!(fallout.store, StoreId::Amazon);
        assert_eq!(fallout.title, "Fallout: New Vegas Ultimate Edition");
        assert_eq!(
            fallout.extra("entitlement_id"),
            Some("aaaaaaaa-0000-0000-0000-000000000001")
        );
        assert_eq!(fallout.extra("sku"), Some("FNV_SKU"));
        assert_eq!(
            fallout.extra("icon_url"),
            Some("https://example.invalid/fnv-icon.jpg")
        );
        assert_eq!(other.title, "Second Game");
        assert_eq!(other.extra("sku"), None);
        Ok(())
    }

    #[test]
    fn titles_lose_trademark_signs_and_extra_spaces() -> eyre::Result<()> {
        let response: EntitlementsResponse = serde_json::from_str(
            r#"{"entitlements":[
                {"id":"e1","product":{"id":"p1","title":"  STAR WARS\u2122  Battlefront\u00ae "}},
                {"id":"e2","product":{"id":"p2","title":" \u2122 "}}
            ]}"#,
        )?;
        let games = owned_games(entitlements_page(response)?.entitlements);
        let titles: Vec<&str> = games.iter().map(|g| g.title.as_str()).collect();
        // A title that is only a sign is no title.
        assert_eq!(titles, vec!["STAR WARS Battlefront"]);
        Ok(())
    }

    #[test]
    fn reports_an_entitlements_error() -> eyre::Result<()> {
        // Amazon names an auth exception: the login was refused.
        let response: EntitlementsResponse =
            serde_json::from_str(&exception("UnrecognizedClientException"))?;
        let Err(refused) = entitlements_page(response) else {
            eyre::bail!("an error answer was read as a page of games");
        };
        assert!(http::is_login_rejected(&refused));
        let message = refused.to_string();
        assert!(
            message.contains("UnrecognizedClientException: Invalid token"),
            "{message}"
        );

        // Any other answer without the list is an error that is not about the login.
        for body in [
            r#"{"__type":"com.amazon.coral.service#InternalFailure","message":"Try again"}"#,
            r#"{"message":"Service unavailable"}"#,
            "{}",
        ] {
            let response: EntitlementsResponse = serde_json::from_str(body)?;
            let result = entitlements_page(response);
            assert!(
                result.is_err_and(|e| {
                    !http::is_login_rejected(&e)
                        && e.to_string().contains("did not send the list of games")
                }),
                "{body}"
            );
        }
        Ok(())
    }

    #[test]
    fn session_round_trips_through_json() -> eyre::Result<()> {
        let session = AmazonSession {
            access_token: "a".into(),
            refresh_token: "r".into(),
            expires_at: 5,
            device_serial: "S".into(),
        };
        let value = serde_json::to_value(&session)?;
        assert_eq!(serde_json::from_value::<AmazonSession>(value)?, session);
        Ok(())
    }

    #[test]
    fn sessions_saved_by_earlier_builds_are_still_read() -> eyre::Result<()> {
        // Earlier builds also saved a client id and a user id.
        let saved = Session {
            account_name: Some("Jane Player".into()),
            data: serde_json::json!({
                "access_token": "Atna|access",
                "refresh_token": "Atnr|refresh",
                "expires_at": 4600,
                "device_serial": "SERIAL",
                "client_id": "53455249414c234132554d56484f58375550345637",
                "user_id": "amzn1.account.TEST"
            }),
        };
        let session: AmazonSession = session_data(StoreId::Amazon, &saved)?;
        assert_eq!(session.access_token, "Atna|access");
        assert_eq!(session.refresh_token, "Atnr|refresh");
        assert_eq!(session.expires_at, 4600);
        assert_eq!(session.device_serial, "SERIAL");

        let unreadable = Session {
            account_name: None,
            data: serde_json::json!({ "cookies": [] }),
        };
        let result = session_data::<AmazonSession>(StoreId::Amazon, &unreadable);
        assert!(result.is_err_and(|e| http::is_login_rejected(&e)));
        Ok(())
    }

    #[test]
    fn debug_output_hides_tokens_and_the_verifier() {
        let session = AmazonSession {
            access_token: "Atna|SECRET".into(),
            refresh_token: "Atnr|SECRET".into(),
            expires_at: 5,
            device_serial: "SERIAL".into(),
        };
        let state = LoginState {
            code_verifier: "SECRET-verifier".into(),
            ..state()
        };
        let shown = format!("{session:?} {state:?}");
        assert!(!shown.contains("SECRET"), "{shown}");
        assert!(shown.contains("SERIAL"), "{shown}");
    }
}
