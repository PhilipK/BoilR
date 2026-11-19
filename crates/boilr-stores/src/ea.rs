//! EA app. Follows Jeshibu's EaLibrary for Playnite
//! (github.com/Jeshibu/PlayniteExtensions, `source/EaLibrary`: `Services/EaWebsite.cs` and
//! `EaLibraryDataGatherer.cs`).
//!
//! Login: the user signs in on www.ea.com. EA's site sends the browser through
//! accounts.ea.com and signin.ea.com and back to `https://www.ea.com/login_check?code=…`;
//! by then accounts.ea.com has set its session cookies (`sid`, `remid`). BoilR keeps the
//! cookies a browser would send to accounts.ea.com (those two, and what EA sets for all of
//! ea.com) and none of EA's other hosts.
//!
//! Library: every refresh trades those cookies for a fresh, short-lived access token
//! (`accounts.ea.com/connect/auth?client_id=ORIGIN_JS_SDK&…&prompt=none`), lists the owned
//! games with the GraphQL query the EA website uses (`getPreloadedOwnedGames`), then looks up
//! each offer's content id in EA's catalog (`legacyOffers`).
//!
//! The game list is asked for by the hash of a query EA's website registered (a persisted
//! query); BoilR does not have the query's text. When EA's website moves to another query,
//! the hash stops working and BoilR needs an update.
//!
//! Not listed:
//! - games the account only has through a linked Steam or Epic account (see
//!   `only_through_linked_store`);
//! - open trials the account holds nothing else of (see `is_unowned_trial`).
//!
//! [`OwnedGame::id`] is the offer id (`Origin.OFR.50.0002487`, `OFB-EAST:52735`, `DR:225064100`).
//! `extra` keys:
//! - `content_id`: catalog content id. The EA app's install/launch link takes it:
//!   `origin2://game/launch/?offerIds=<content_id>` (EaLibrary `EaControllerHelper.cs`).
//!   Absent when the catalog lookup failed and no earlier list had it.

use std::{collections::HashMap, future::Future};

use eyre::WrapErr;
use reqwest::Client;
use serde::{de::IgnoredAny, Deserialize, Serialize};

use crate::{
    http::{self, cookie_jar},
    Cookie, Fetch, LoginCapture, ManualLogin, OwnedGame, PendingLogin, Session, StoreId,
};

const LOGIN_URL: &str = "https://www.ea.com/login";
/// EA's site redirects here (with `?code=…`) once accounts.ea.com accepted the sign-in.
const LOGIN_DONE_PREFIX: &str = "https://www.ea.com/login_check";
/// The host whose cookies carry the EA sign-in.
const ACCOUNTS_HOST: &str = "accounts.ea.com";
/// Where a refresh gets its access token. A cookie EA sets here without a `Path` is scoped
/// to `/connect`, and is not handed out for the host's root URL: the login window is asked
/// for this URL's cookies too, and renewed cookies are read back for it.
const AUTH_URL: &str = "https://accounts.ea.com/connect/auth";
const TOKEN_QUERY: &str =
    "client_id=ORIGIN_JS_SDK&response_type=token&redirect_uri=nucleus:rest&prompt=none";
/// Cookies set for all of ea.com come with these too.
const COOKIE_URLS: [&str; 2] = ["https://accounts.ea.com/", AUTH_URL];
const GRAPHQL_URL: &str = "https://service-aggregation-layer.juno.ea.com/graphql";
const OWNED_GAMES_QUERY_HASH: &str =
    "779f1cd1355699752e20c0b3877847f4e3010ef5de131c248e98f8eff84f0718";
const PAGE_SIZE: u32 = 500;
const FIRST_PAGE: &str = "0";
const MAX_PAGES: usize = 50;
const LEGACY_OFFER_BATCH: usize = 100;

/// Every way an account can have a game, as EaLibrary asks for them (`EaWebsite.GetGamesUrl`).
/// The `STEAM…` and `EPIC…` ones come from a linked Steam or Epic account
/// ([`is_linked_store`]); `VAULT` is EA Play and `XGP_VAULT` Xbox Game Pass.
const OWNERSHIP_METHODS: [&str; 16] = [
    "UNKNOWN",
    "ASSOCIATION",
    "PURCHASE",
    "REDEMPTION",
    "GIFT_RECEIPT",
    GRANT,
    "DIRECT_ENTITLEMENT",
    "PRE_ORDER_PURCHASE",
    "VAULT",
    "XGP_VAULT",
    "STEAM",
    "STEAM_VAULT",
    "STEAM_SUBSCRIPTION",
    "EPIC",
    "EPIC_VAULT",
    "EPIC_SUBSCRIPTION",
];
/// EA's own entitlement record. Most games bought on EA carry it, but so do games bought on
/// Steam (EaLibrary's sample answer has `["STEAM", "ENTITLEMENT_GRANT"]`), so on its own it
/// does not say where a game comes from.
const GRANT: &str = "ENTITLEMENT_GRANT";

/// What the session file holds.
#[derive(Clone, Default, Serialize, Deserialize)]
struct EaSession {
    cookies: Vec<Cookie>,
}

pub fn begin_login() -> PendingLogin {
    PendingLogin {
        page_text_url_prefixes: vec![],
        store: StoreId::Ea,
        start_url: LOGIN_URL.to_string(),
        done_url_prefixes: vec![LOGIN_DONE_PREFIX.to_string()],
        cookie_urls: COOKIE_URLS.iter().map(|u| u.to_string()).collect(),
        // The sign-in lives in HttpOnly cookies, which a user cannot reasonably copy.
        manual: ManualLogin::Unsupported,
        user_agent: Some(http::BROWSER_USER_AGENT.to_string()),
        state: serde_json::Value::Null,
    }
}

/// The cookies of a sign-in that BoilR keeps: the ones a browser would send to
/// accounts.ea.com, the only host that is asked for a token.
fn session_cookies(captured: &[Cookie]) -> Vec<Cookie> {
    cookie_jar::keep_for_hosts(captured, &[ACCOUNTS_HOST])
}

pub async fn complete_login(
    _client: &Client,
    _pending: &PendingLogin,
    capture: &LoginCapture,
) -> eyre::Result<Session> {
    let mut cookies = session_cookies(&capture.cookies);
    if cookies.is_empty() {
        return Err(eyre::format_err!(
            "The EA sign-in window handed over no cookies for {ACCOUNTS_HOST}, so the sign-in \
             could not be kept."
        ));
    }
    let (client, jar) = cookie_jar::client_with(&cookies)?;
    let token = access_token(&client)
        .await
        .wrap_err("Signing in to EA did not finish")?;
    let account_name = player_name(&client, &token).await;
    cookie_jar::refresh(&mut cookies, &jar, &[AUTH_URL]);
    Ok(Session {
        account_name,
        data: serde_json::to_value(EaSession { cookies })?,
    })
}

pub async fn fetch_owned(
    _client: &Client,
    session: &mut Session,
    fetch: &mut Fetch<'_>,
) -> eyre::Result<Vec<OwnedGame>> {
    let mut data: EaSession = crate::session_data(StoreId::Ea, session)?;
    let (client, jar) = cookie_jar::client_with(&data.cookies)?;
    let token = access_token(&client).await;
    // EA may have renewed `sid`; keep the renewed cookies even when something below fails.
    cookie_jar::refresh(&mut data.cookies, &jar, &[AUTH_URL]);
    session.data = serde_json::to_value(&data)?;
    let token = token?;

    if session.account_name.is_none() {
        session.account_name = player_name(&client, &token).await;
    }

    let (client, token) = (&client, token.as_str());
    let games = http::paginate(
        "EA's game list",
        FIRST_PAGE.to_string(),
        MAX_PAGES,
        |cursor| async move {
            let url = owned_games_url(&cursor)?;
            let page = graphql_get(client, token, &url, parse_owned_games_page).await?;
            Ok((page.games, page.next))
        },
    )
    .await?;

    // Content ids only improve the install link and matching installed copies; without
    // them the offer id is used, so a failed lookup never fails the library. Ids the lookup
    // did not return come from the previous list, so one failed lookup does not unmatch
    // installed games until the next refresh. (Like EaLibrary, the catalog lookup is sent
    // without a token.)
    let offer_ids: Vec<String> = games.iter().map(|g| g.id.clone()).collect();
    let (content_ids, failed) = lookup_in_batches(&offer_ids, LEGACY_OFFER_BATCH, |batch| {
        legacy_content_ids(client, batch)
    })
    .await;
    let games = with_content_ids(games, &content_ids, fetch.previous);
    if let Some(err) = failed {
        eprintln!(
            "Could not look up EA content ids, using the ones from before: {}",
            http::redact(&format!("{err:#}"))
        );
        if games.iter().any(|g| g.extra("content_id").is_none()) {
            fetch.partial = true;
        }
    }
    Ok(games)
}

/// Looks ids up `size` at a time. The first batch that fails ends the lookup: with the
/// catalog down, every further batch would only wait for its own timeout. Returns what was
/// found until then, and the failure.
async fn lookup_in_batches<'a, F, Fut>(
    ids: &'a [String],
    size: usize,
    mut lookup: F,
) -> (HashMap<String, String>, Option<eyre::Report>)
where
    F: FnMut(&'a [String]) -> Fut,
    Fut: Future<Output = eyre::Result<HashMap<String, String>>>,
{
    let mut found = HashMap::new();
    for batch in ids.chunks(size.max(1)) {
        match lookup(batch).await {
            Ok(ids) => found.extend(ids),
            Err(err) => return (found, Some(err)),
        }
    }
    (found, None)
}

fn with_content_ids(
    games: Vec<OwnedGame>,
    content_ids: &HashMap<String, String>,
    previous: &[OwnedGame],
) -> Vec<OwnedGame> {
    games
        .into_iter()
        .map(|game| {
            let content_id = content_ids.get(&game.id).cloned().or_else(|| {
                previous
                    .iter()
                    .find(|p| p.id == game.id)
                    .and_then(|p| p.extra("content_id"))
                    .map(str::to_string)
            });
            match content_id {
                Some(content_id) => game.with_extra("content_id", content_id),
                None => game,
            }
        })
        .collect()
}

/// The ids in the `id=` value of an installed EA game's `.mfst` manifest (in
/// `ProgramData/Origin/LocalContent`), as BoilR's installed EA platform reads it: URL-encoded,
/// possibly several offer ids joined by commas (`OFB-EAST%3a52735`,
/// `Origin.OFR.50.0002487%2cOrigin.OFR.50.0002412`). The account lists the game under one of
/// these, or under its content id.
pub fn manifest_ids(manifest_id: &str) -> Vec<String> {
    let decoded = url::form_urlencoded::parse(format!("id={manifest_id}").as_bytes())
        .next()
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| manifest_id.to_string());
    decoded
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect()
}

async fn access_token(client: &Client) -> eyre::Result<String> {
    let response = client
        .get(format!("{AUTH_URL}?{TOKEN_QUERY}"))
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(http::request_failed)?;
    let status = response.status().as_u16();
    let body = response.text().await.map_err(http::request_failed)?;
    token_from_answer(status, &body)
}

/// The display name of the signed-in account, if EA hands it out. Never fails a login.
async fn player_name(client: &Client, token: &str) -> Option<String> {
    let body = serde_json::json!({ "query": "query { me { player { displayName } } }" });
    let response = client
        .post(GRAPHQL_URL)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .ok()?;
    parse_player_name(&response.text().await.ok()?)
}

/// GETs a GraphQL URL and reads the answer with `parse`; see [`graphql_answer`].
async fn graphql_get<T>(
    client: &Client,
    token: &str,
    url: &str,
    parse: fn(&str) -> eyre::Result<T>,
) -> eyre::Result<T> {
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(http::request_failed)?;
    let status = response.status().as_u16();
    let body = response.text().await.map_err(http::request_failed)?;
    graphql_answer(status, &body, parse)
}

async fn legacy_content_ids(
    client: &Client,
    offer_ids: &[String],
) -> eyre::Result<HashMap<String, String>> {
    // The same query EaLibrary sends (getLegacyCatalogDefs), cut down to the fields we use.
    let body = serde_json::json!({
        "query": "query getLegacyCatalogDefs($offerIds: [String!]!, $locale: Locale) { legacyOffers(offerIds: $offerIds, locale: $locale) { offerId: id contentId } }",
        "operationName": "getLegacyCatalogDefs",
        "variables": { "locale": "DEFAULT", "offerIds": offer_ids },
    });
    let response = http::send(client.post(GRAPHQL_URL).json(&body)).await?;
    let answer = response.text().await.map_err(http::request_failed)?;
    parse_legacy_offers(&answer)
}

fn owned_games_url(cursor: &str) -> eyre::Result<String> {
    // The variables and persisted-query hash EaLibrary uses (EaWebsite.GetGamesUrl).
    let variables = serde_json::json!({
        "isMac": false,
        "addFieldsToPreloadGames": true,
        "locale": "en",
        "limit": PAGE_SIZE,
        "next": cursor,
        "type": ["DIGITAL_FULL_GAME", "PACKAGED_FULL_GAME"],
        "entitlementEnabled": true,
        "storefronts": ["EA", "STEAM", "EPIC"],
        "ownershipMethods": OWNERSHIP_METHODS,
        "platforms": ["PC"],
    });
    let extensions = serde_json::json!({
        "persistedQuery": { "version": 1, "sha256Hash": OWNED_GAMES_QUERY_HASH }
    });
    let mut url = url::Url::parse(GRAPHQL_URL)?;
    url.query_pairs_mut()
        .append_pair("operationName", "getPreloadedOwnedGames")
        .append_pair("variables", &variables.to_string())
        .append_pair("extensions", &extensions.to_string());
    Ok(url.into())
}

// ---- Reading answers (pure) ----

#[derive(Default, Deserialize)]
struct TokenAnswer {
    access_token: Option<String>,
    error: Option<String>,
    error_code: Option<String>,
}

fn failed_with(status: u16, url: &str, body: &str) -> eyre::Report {
    eyre::Report::new(http::HttpStatus {
        status,
        url: url.to_string(),
        body: http::snippet(body),
    })
}

/// The access token in EA's answer to the token request, or why there is none. Only EA
/// saying that a login is required counts as a refused login: an outage or a rate limit is
/// transient, and anything else is reported as it is.
fn token_from_answer(status: u16, body: &str) -> eyre::Result<String> {
    let answer: TokenAnswer = serde_json::from_str(body).unwrap_or_default();
    let success = (200..300).contains(&status);
    if let Some(token) = answer.access_token.filter(|t| success && !t.is_empty()) {
        return Ok(token);
    }
    if http::is_transient_status(status) {
        return Err(failed_with(status, AUTH_URL, body));
    }
    let errors = [answer.error, answer.error_code];
    let mut errors = errors.iter().flatten();
    if errors.clone().any(|error| error == "login_required") {
        return Err(http::login_rejected(
            "EA no longer accepts the saved sign-in (it answered \"login_required\"). Sign in again.",
        ));
    }
    match errors.next() {
        Some(error) => Err(eyre::format_err!(
            "EA did not hand out an access token: it answered \"{error}\" (HTTP {status})"
        )),
        None if !success => Err(failed_with(status, AUTH_URL, body)),
        None => Err(eyre::format_err!(
            "Unexpected answer from EA's sign-in service, without an access token: {}",
            http::snippet(body)
        )),
    }
}

/// Whether a body is a GraphQL answer: an object with `data` or a list of `errors`.
fn is_graphql(body: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(body).is_ok_and(|answer| {
        answer.get("data").is_some() || answer.get("errors").is_some_and(|e| e.is_array())
    })
}

/// Reads an answer of EA's GraphQL service with `parse`.
///
/// A failure status is always an error. When its body is a GraphQL answer, EA's own error
/// in it is reported, because it says more than the status: EA answers a rejected token
/// with 401 and an `UNAUTHENTICATED` error, and a persisted query it no longer knows with
/// an error too. That holds unless the status says "sign in again" (401) or "not now" (429,
/// 5xx) and EA's error does not: then the status is reported, so the failure keeps its kind.
fn graphql_answer<T>(
    status: u16,
    body: &str,
    parse: impl FnOnce(&str) -> eyre::Result<T>,
) -> eyre::Result<T> {
    if (200..300).contains(&status) {
        return parse(body);
    }
    if is_graphql(body) {
        if let Err(err) = parse(body) {
            let status_says_more = status == 401 || http::is_transient_status(status);
            if http::is_login_rejected(&err) || !status_says_more {
                return Err(err);
            }
        }
    }
    Err(failed_with(status, GRAPHQL_URL, body))
}

#[derive(Debug, Deserialize)]
struct GraphQlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    message: Option<String>,
    extensions: Option<GraphQlErrorExtensions>,
}

#[derive(Debug, Deserialize)]
struct GraphQlErrorExtensions {
    code: Option<String>,
}

impl GraphQlError {
    fn code(&self) -> Option<&str> {
        self.extensions.as_ref()?.code.as_deref()
    }

    /// Apollo's answer to the hash of a persisted query it does not know (any more).
    fn is_unknown_persisted_query(&self) -> bool {
        matches!(
            self.code(),
            Some("PERSISTED_QUERY_NOT_FOUND" | "PERSISTED_QUERY_NOT_SUPPORTED")
        ) || matches!(
            self.message.as_deref(),
            Some("PersistedQueryNotFound" | "PersistedQueryNotSupported")
        )
    }
}

impl<T> GraphQlResponse<T> {
    /// What `pick` finds in the data, or a readable error built from `errors`. Partial data
    /// (data plus errors on some fields) is kept, unless EA says the sign-in is not valid.
    fn into_data<U>(self, what: &str, pick: impl FnOnce(T) -> Option<U>) -> eyre::Result<U> {
        let errors = self.errors.unwrap_or_default();
        if errors.iter().any(|e| e.code() == Some("UNAUTHENTICATED")) {
            return Err(http::login_rejected(format!(
                "EA no longer accepts the saved sign-in (it answered \"not authenticated\" \
                 while listing {what}). Sign in again."
            )));
        }
        if errors.iter().any(GraphQlError::is_unknown_persisted_query) {
            return Err(eyre::format_err!(
                "EA changed how its website lists {what}, and BoilR needs an update to read \
                 them. (EA no longer knows the request BoilR sends.)"
            ));
        }
        if let Some(found) = self.data.and_then(pick) {
            return Ok(found);
        }
        let messages: Vec<&str> = errors.iter().filter_map(|e| e.message.as_deref()).collect();
        Err(eyre::format_err!(
            "EA could not list {what}: {}",
            if messages.is_empty() {
                "no data in the answer".to_string()
            } else {
                messages.join("; ")
            }
        ))
    }
}

#[derive(Debug, Deserialize)]
struct OwnedGamesData {
    me: Option<OwnedGamesUser>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OwnedGamesUser {
    owned_game_products: Option<OwnedGamesPage>,
}

#[derive(Debug, Deserialize)]
struct OwnedGamesPage {
    next: Option<String>,
    items: Option<Vec<OwnedGameProduct>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OwnedGameProduct {
    id: Option<String>,
    origin_offer_id: Option<String>,
    product: Option<Product>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Product {
    name: Option<String>,
    /// The product is itself an open trial, not the game.
    is_ungated_trial: Option<bool>,
    base_item: Option<BaseItem>,
    game_product_user: Option<GameProductUser>,
}

#[derive(Debug, Deserialize)]
struct BaseItem {
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameProductUser {
    ownership_methods: Option<Vec<String>>,
    /// The account's trial of this product (its remaining time), when it started one.
    game_product_user_trial: Option<IgnoredAny>,
}

#[derive(Debug)]
struct ParsedPage {
    games: Vec<OwnedGame>,
    next: Option<String>,
}

fn parse_owned_games_page(body: &str) -> eyre::Result<ParsedPage> {
    let response: GraphQlResponse<OwnedGamesData> = serde_json::from_str(body).map_err(|err| {
        eyre::format_err!(
            "Unexpected answer from EA's library service: {err}. Body starts with: {}",
            http::snippet(body)
        )
    })?;
    let page = response.into_data("your games", |data| data.me?.owned_game_products)?;
    Ok(ParsedPage {
        games: page
            .items
            .unwrap_or_default()
            .into_iter()
            .filter_map(to_owned_game)
            .collect(),
        next: page.next.filter(|next| !next.is_empty()),
    })
}

fn to_owned_game(item: OwnedGameProduct) -> Option<OwnedGame> {
    let offer_id = item
        .origin_offer_id
        .filter(|id| !id.is_empty())
        .or(item.id)
        .filter(|id| !id.is_empty())?;
    let product = item.product?;
    let user = product.game_product_user;
    let trial_started = user
        .as_ref()
        .is_some_and(|u| u.game_product_user_trial.is_some());
    let methods = user.and_then(|u| u.ownership_methods).unwrap_or_default();
    let trial_product = product.is_ungated_trial == Some(true);
    if only_through_linked_store(&methods)
        || is_unowned_trial(trial_product, trial_started, &methods)
    {
        return None;
    }
    let title = product
        .name
        .as_deref()
        .map(crate::clean_title)
        .filter(|t| !t.is_empty())
        .or_else(|| {
            product
                .base_item
                .as_ref()
                .and_then(|b| b.title.as_deref())
                .map(crate::clean_title)
        })
        .filter(|t| !t.is_empty())?;
    Some(OwnedGame::new(StoreId::Ea, offer_id, title))
}

/// An ownership method that comes from a linked Steam or Epic account: a purchase there
/// (`STEAM`, `EPIC`) or an EA Play subscription taken out there (`STEAM_VAULT`,
/// `STEAM_SUBSCRIPTION`, `EPIC_VAULT`, `EPIC_SUBSCRIPTION`).
fn is_linked_store(method: &str) -> bool {
    method.starts_with("STEAM") || method.starts_with("EPIC")
}

/// Whether nothing but a linked Steam or Epic account gives the EA account this game. Such
/// a game belongs to that store (Steam has it already, the Epic account adds it), and the
/// EA app cannot install it, so it is not listed.
///
/// Anything EA itself gives wins: a purchase or a code redeemed on EA, EA Play, Game Pass,
/// and methods BoilR does not know. The exception is `ENTITLEMENT_GRANT` ([`GRANT`]), which
/// comes with Steam purchases too and so proves nothing next to a linked store.
fn only_through_linked_store(methods: &[String]) -> bool {
    methods.iter().any(|m| is_linked_store(m))
        && methods.iter().all(|m| is_linked_store(m) || m == GRANT)
}

/// Whether an entry is a trial of a game the account does not have.
///
/// EA's answer carries several trial fields, and most of them say nothing about ownership:
/// in EaLibrary's sample answer a bought game (`A Way Out`, with its free Friend's Pass)
/// has both `trialDetails` and a `gameProductUserTrial`. So an entry is only skipped when
/// everything agrees: EA marks the product itself as an open trial (`isUngatedTrial`), the
/// account started that trial, and nothing but the grant a free trial is claimed with gives
/// the account the product. In every other case the game is kept.
fn is_unowned_trial(trial_product: bool, trial_started: bool, methods: &[String]) -> bool {
    trial_product && trial_started && methods.iter().all(|m| m == GRANT)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyOffersData {
    legacy_offers: Option<Vec<Option<LegacyOffer>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyOffer {
    offer_id: Option<String>,
    content_id: Option<String>,
}

/// Offer id to content id.
fn parse_legacy_offers(body: &str) -> eyre::Result<HashMap<String, String>> {
    let response: GraphQlResponse<LegacyOffersData> =
        serde_json::from_str(body).map_err(|err| {
            eyre::format_err!(
                "Unexpected answer from EA's catalog: {err}. Body starts with: {}",
                http::snippet(body)
            )
        })?;
    Ok(response
        .into_data("game details", |data| {
            Some(data.legacy_offers.unwrap_or_default())
        })?
        .into_iter()
        .flatten()
        .filter_map(|o| match (o.offer_id, o.content_id) {
            (Some(offer), Some(content)) if !offer.is_empty() && !content.is_empty() => {
                Some((offer, content))
            }
            _ => None,
        })
        .collect())
}

#[derive(Debug, Deserialize)]
struct PlayerData {
    me: Option<PlayerUser>,
}

#[derive(Debug, Deserialize)]
struct PlayerUser {
    player: Option<Player>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Player {
    display_name: Option<String>,
}

fn parse_player_name(body: &str) -> Option<String> {
    serde_json::from_str::<GraphQlResponse<PlayerData>>(body)
        .ok()?
        .data?
        .me?
        .player?
        .display_name
        .filter(|n| !n.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> eyre::Result<String> {
        let path = format!("{}/testdata/ea/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(&path).wrap_err(path)
    }

    fn cookie(name: &str, value: &str, domain: &str) -> Cookie {
        Cookie {
            name: name.into(),
            value: value.into(),
            domain: domain.into(),
        }
    }

    fn methods(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    fn run<T>(future: impl Future<Output = T>) -> eyre::Result<T> {
        Ok(tokio::runtime::Builder::new_current_thread()
            .build()?
            .block_on(future))
    }

    /// The failure, when it is neither a refused login nor a transient one.
    fn plain_error<T>(result: eyre::Result<T>) -> Option<String> {
        result
            .err()
            .filter(|e| !http::is_login_rejected(e) && !http::is_transient(e))
            .map(|e| e.to_string())
    }

    #[test]
    fn login_urls_do_not_finish_early() {
        let pending = begin_login();
        let done = |url: &str| {
            pending
                .done_url_prefixes
                .iter()
                .any(|prefix| url.starts_with(prefix.as_str()))
        };
        assert!(!done(&pending.start_url));
        // The pages EA's sign-in goes through (seen 2026-11-08).
        assert!(!done("https://accounts.ea.com/connect/auth?response_type=code&redirect_uri=https%3A%2F%2Fwww.ea.com%2Flogin_check&state=x&locale=en_US&client_id=EADOTCOM-WEB-SERVER&display=junoWeb%2Flogin"));
        assert!(!done("https://signin.ea.com/p/juno/login?fid=abc"));
        assert!(done("https://www.ea.com/login_check?code=QUOx&state=x"));
    }

    #[test]
    fn the_login_window_is_asked_for_what_the_token_request_needs() {
        let pending = begin_login();
        // Cookies scoped to `/connect` are not handed out for the root URL.
        assert!(pending.cookie_urls.iter().any(|url| url == AUTH_URL));
        assert!(pending
            .cookie_urls
            .iter()
            .all(|url| url.starts_with(&format!("https://{ACCOUNTS_HOST}/"))));
        // The session is replayed under the user agent it was made with.
        assert_eq!(
            pending.user_agent.as_deref(),
            Some(http::BROWSER_USER_AGENT)
        );
    }

    #[test]
    fn only_cookies_for_the_accounts_host_are_kept() {
        // Domains as a login window reports them: without a leading dot.
        let captured = vec![
            cookie("sid", "s", "accounts.ea.com"),
            cookie("remid", "r", "accounts.ea.com"),
            cookie("site", "e", "ea.com"),
            cookie("signin", "x", "signin.ea.com"),
            cookie("shop", "w", "www.ea.com"),
            cookie("tracker", "t", "example.com"),
            cookie("lookalike", "l", "notea.com"),
        ];
        let kept = session_cookies(&captured);
        let names: Vec<&str> = kept.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["sid", "remid", "site"]);
        assert!(session_cookies(&[cookie("shop", "w", "www.ea.com")]).is_empty());
    }

    #[test]
    fn reads_access_token() -> eyre::Result<()> {
        assert_eq!(
            token_from_answer(200, &fixture("token_ok.json")?)?,
            "QVQwOjIuMDozLjA6NjA6dGVzdHRva2Vu"
        );
        Ok(())
    }

    #[test]
    fn only_a_required_login_asks_to_sign_in_again() -> eyre::Result<()> {
        let login_required = fixture("token_login_required.json")?;
        for status in [200, 400, 401] {
            let refused = token_from_answer(status, &login_required);
            assert!(
                refused.is_err_and(|e| http::is_login_rejected(&e)),
                "{status}"
            );
        }

        // An outage or a rate limit, whatever the body says.
        for (status, body) in [
            (503, "<html>Service Unavailable</html>"),
            (502, ""),
            (429, r#"{"error":"too_many_requests"}"#),
            (500, login_required.as_str()),
        ] {
            let outage = token_from_answer(status, body);
            assert!(
                outage.is_err_and(|e| http::is_transient(&e) && !http::is_login_rejected(&e)),
                "{status}"
            );
        }

        // Another error is reported with EA's own word for it.
        let other = plain_error(token_from_answer(400, r#"{"error":"invalid_request"}"#))
            .unwrap_or_default();
        assert!(other.contains("invalid_request"), "{other}");

        // A page where JSON was expected, a bot block, and a token on a failed request.
        let page = plain_error(token_from_answer(200, "<html>login</html>")).unwrap_or_default();
        assert!(page.contains("Unexpected answer"), "{page}");
        let blocked = token_from_answer(403, "<html>Access Denied</html>");
        assert!(plain_error(blocked).is_some());
        let failed = token_from_answer(404, &fixture("token_ok.json")?);
        assert!(plain_error(failed).is_some());
        let empty = token_from_answer(200, r#"{"access_token":""}"#);
        assert!(plain_error(empty).is_some());

        // A 401 without EA's reason is a refused login by its status.
        let unauthorized = token_from_answer(401, "");
        assert!(unauthorized.is_err_and(|e| http::is_login_rejected(&e)));
        Ok(())
    }

    #[test]
    fn parses_owned_games_pages() -> eyre::Result<()> {
        let first = parse_owned_games_page(&fixture("owned_games_page1.json")?)?;
        assert_eq!(first.next.as_deref(), Some("Mw=="));
        // A Way Out is only there through the linked Steam account. Crysis comes with Game
        // Pass, which the EA app installs.
        let titles: Vec<&str> = first.games.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(titles, vec!["Battlefield 3", "Crysis"]);

        let battlefield = first
            .games
            .first()
            .ok_or_else(|| eyre::format_err!("empty"))?;
        assert_eq!(battlefield.store, StoreId::Ea);
        assert_eq!(battlefield.id, "DR:225064100");
        // The content id is added later; nothing else is kept.
        assert!(battlefield.extra.is_empty());

        let second = parse_owned_games_page(&fixture("owned_games_page2.json")?)?;
        assert_eq!(second.next, None);
        let zuma = second
            .games
            .first()
            .ok_or_else(|| eyre::format_err!("empty"))?;
        assert_eq!(zuma.id, "OFB-EAST:52735");
        // Trademark signs are dropped.
        assert_eq!(zuma.title, "Zuma's Revenge");

        // An empty cursor ends the list like a missing one.
        let last = parse_owned_games_page(
            r#"{"data":{"me":{"ownedGameProducts":{"next":"","items":[]}}}}"#,
        )?;
        assert_eq!(last.next, None);
        Ok(())
    }

    #[test]
    fn games_of_a_linked_store_are_left_out_unless_ea_gives_them_too() {
        let linked_only = |names: &[&str]| only_through_linked_store(&methods(names));
        // Bought on Steam or Epic, or part of an EA Play subscription taken out there.
        for method in [
            "STEAM",
            "STEAM_VAULT",
            "STEAM_SUBSCRIPTION",
            "EPIC",
            "EPIC_VAULT",
            "EPIC_SUBSCRIPTION",
        ] {
            assert!(linked_only(&[method]), "{method}");
        }
        assert!(linked_only(&["STEAM", "EPIC"]));
        // Steam purchases carry EA's grant too (both orders are in EaLibrary's sample).
        assert!(linked_only(&["STEAM", "ENTITLEMENT_GRANT"]));
        assert!(linked_only(&["ENTITLEMENT_GRANT", "STEAM"]));
        assert!(linked_only(&["EPIC_VAULT", "ENTITLEMENT_GRANT"]));

        // Real EA ownership wins.
        assert!(!linked_only(&["STEAM", "PURCHASE", "ENTITLEMENT_GRANT"]));
        assert!(!linked_only(&["STEAM", "REDEMPTION", "ENTITLEMENT_GRANT"]));
        assert!(!linked_only(&["EPIC", "GIFT_RECEIPT"]));
        assert!(!linked_only(&["STEAM_VAULT", "PRE_ORDER_PURCHASE"]));
        assert!(!linked_only(&["EPIC_SUBSCRIPTION", "DIRECT_ENTITLEMENT"]));
        // So do the subscriptions the EA app installs, and a method BoilR does not know.
        assert!(!linked_only(&["STEAM", "XGP_VAULT"]));
        assert!(!linked_only(&["EPIC", "VAULT"]));
        assert!(!linked_only(&["STEAM", "SOMETHING_NEW"]));

        // Nothing from a linked store.
        assert!(!linked_only(&["PURCHASE", "ENTITLEMENT_GRANT"]));
        assert!(!linked_only(&["ENTITLEMENT_GRANT"]));
        assert!(!linked_only(&["VAULT"]));
        assert!(!linked_only(&["XGP_VAULT"]));
        assert!(!linked_only(&["UNKNOWN"]));
        assert!(!linked_only(&[]));
    }

    #[test]
    fn only_a_clear_trial_is_left_out() -> eyre::Result<()> {
        let page = parse_owned_games_page(&fixture("owned_games_trials.json")?)?;
        let titles: Vec<&str> = page.games.iter().map(|g| g.title.as_str()).collect();
        // A bought game with a trial on record stays, and so does a trial product the
        // account also bought. The open trial held by nothing but its grant goes.
        assert_eq!(titles, vec!["A Way Out", "Bought After The Trial"]);

        let grant = methods(&["ENTITLEMENT_GRANT"]);
        assert!(is_unowned_trial(true, true, &grant));
        assert!(is_unowned_trial(true, true, &[]));
        // Any doubt keeps the game.
        assert!(!is_unowned_trial(false, true, &grant));
        assert!(!is_unowned_trial(true, false, &grant));
        for owned in [
            &["PURCHASE", "ENTITLEMENT_GRANT"][..],
            &["VAULT"],
            &["XGP_VAULT"],
            &["UNKNOWN"],
        ] {
            assert!(!is_unowned_trial(true, true, &methods(owned)), "{owned:?}");
        }
        Ok(())
    }

    #[test]
    fn every_ownership_method_is_asked_for_once() -> eyre::Result<()> {
        let url = owned_games_url(FIRST_PAGE)?;
        let variables: serde_json::Value =
            serde_json::from_str(&http::query_param(&url, "variables").unwrap_or_default())?;
        assert_eq!(
            variables.get("ownershipMethods"),
            Some(&serde_json::json!(OWNERSHIP_METHODS))
        );
        for (i, method) in OWNERSHIP_METHODS.iter().enumerate() {
            assert!(
                !OWNERSHIP_METHODS.iter().skip(i + 1).any(|m| m == method),
                "{method}"
            );
        }
        assert_eq!(
            OWNERSHIP_METHODS
                .iter()
                .filter(|m| is_linked_store(m))
                .count(),
            6
        );
        Ok(())
    }

    #[test]
    fn an_unauthenticated_answer_is_a_refused_login() -> eyre::Result<()> {
        let body = fixture("not_authenticated.json")?;
        let refused = parse_owned_games_page(&body);
        assert!(refused.is_err_and(|e| http::is_login_rejected(&e)));
        // EA sends it with 401.
        let refused = graphql_answer(401, &body, parse_owned_games_page);
        assert!(refused.is_err_and(|e| http::is_login_rejected(&e)));
        Ok(())
    }

    #[test]
    fn a_failure_status_is_never_read_as_an_empty_answer() -> eyre::Result<()> {
        let read = |status: u16, body: &str| graphql_answer(status, body, parse_owned_games_page);

        // JSON that is not a GraphQL answer: the status decides.
        let unauthorized = read(401, r#"{"message":"Unauthorized"}"#);
        assert!(unauthorized
            .is_err_and(|e| http::is_login_rejected(&e) && http::status_of(&e) == Some(401)));
        let limited = read(429, r#"{"message":"Too many requests"}"#);
        assert!(limited.is_err_and(|e| http::is_transient(&e) && !http::is_login_rejected(&e)));
        let down = read(503, "<html>Service Unavailable</html>");
        assert!(down.is_err_and(|e| http::is_transient(&e)));
        let blocked = read(403, "<html>Access Denied</html>");
        assert!(blocked
            .is_err_and(|e| { http::status_of(&e) == Some(403) && !http::is_login_rejected(&e) }));

        // A GraphQL error with an outage status stays an outage, and with 401 a refused login.
        let upstream = r#"{"errors":[{"message":"upstream timed out"}],"data":null}"#;
        let outage = read(502, upstream);
        assert!(outage.is_err_and(|e| http::is_transient(&e) && !http::is_login_rejected(&e)));
        let refused = read(401, upstream);
        assert!(refused.is_err_and(|e| http::is_login_rejected(&e)));
        // With another status, EA's error is the message.
        let bad_request = plain_error(read(400, upstream)).unwrap_or_default();
        assert!(bad_request.contains("upstream timed out"), "{bad_request}");

        // Data that comes with a failure status is not taken as the game list.
        let page = fixture("owned_games_page1.json")?;
        assert!(read(500, &page).is_err_and(|e| http::is_transient(&e)));
        assert!(read(404, &page).is_err());
        assert_eq!(read(200, &page)?.games.len(), 2);
        Ok(())
    }

    #[test]
    fn an_unknown_persisted_query_asks_for_an_update_not_a_sign_in() {
        for body in [
            r#"{"errors":[{"message":"PersistedQueryNotFound","extensions":{"code":"PERSISTED_QUERY_NOT_FOUND"}}]}"#,
            r#"{"data":null,"errors":[{"message":"PersistedQueryNotFound"}]}"#,
            r#"{"errors":[{"message":"PersistedQueryNotSupported","extensions":{"code":"PERSISTED_QUERY_NOT_SUPPORTED"}}]}"#,
        ] {
            for status in [200, 400] {
                let message = plain_error(graphql_answer(status, body, parse_owned_games_page))
                    .unwrap_or_default();
                assert!(message.contains("BoilR needs an update"), "{message}");
                assert!(!message.to_lowercase().contains("sign in"), "{message}");
            }
        }
    }

    #[test]
    fn graphql_errors_are_reported_with_eas_message() {
        // No data at all.
        let none = plain_error(parse_owned_games_page(
            r#"{"data":null,"errors":[{"message":"Something broke"}]}"#,
        ))
        .unwrap_or_default();
        assert!(none.contains("Something broke"), "{none}");
        // Data without the account in it.
        let no_account = plain_error(parse_owned_games_page(
            r#"{"data":{"me":null},"errors":[{"message":"Account lookup failed"}]}"#,
        ))
        .unwrap_or_default();
        assert!(no_account.contains("Account lookup failed"), "{no_account}");
        let no_list =
            plain_error(parse_owned_games_page(r#"{"data":{"me":{}}}"#)).unwrap_or_default();
        assert!(no_list.contains("no data in the answer"), "{no_list}");
        // Not GraphQL at all.
        assert!(plain_error(parse_owned_games_page("<html></html>")).is_some());
    }

    #[test]
    fn parses_content_ids() -> eyre::Result<()> {
        let ids = parse_legacy_offers(&fixture("legacy_offers.json")?)?;
        assert_eq!(ids.len(), 4);
        assert_eq!(
            ids.get("Origin.OFR.50.0002487").map(String::as_str),
            Some("1028663")
        );
        Ok(())
    }

    #[test]
    fn content_ids_fall_back_to_the_previous_library() {
        let games = vec![
            OwnedGame::new(StoreId::Ea, "A", "A"),
            OwnedGame::new(StoreId::Ea, "B", "B"),
            OwnedGame::new(StoreId::Ea, "C", "C"),
        ];
        let found = HashMap::from([("A".to_string(), "1".to_string())]);
        let previous = vec![
            OwnedGame::new(StoreId::Ea, "A", "A").with_extra("content_id", "old"),
            OwnedGame::new(StoreId::Ea, "B", "B").with_extra("content_id", "2"),
        ];
        let ids: Vec<Option<String>> = with_content_ids(games, &found, &previous)
            .iter()
            .map(|g| g.extra("content_id").map(str::to_string))
            .collect();
        assert_eq!(ids, vec![Some("1".into()), Some("2".into()), None]);
    }

    #[test]
    fn a_failed_content_id_batch_ends_the_lookup() -> eyre::Result<()> {
        let ids: Vec<String> = (0..7).map(|i| i.to_string()).collect();
        let content_ids = |batch: &[String]| -> HashMap<String, String> {
            batch
                .iter()
                .map(|id| (id.clone(), format!("c{id}")))
                .collect()
        };

        let asked = std::cell::Cell::new(0);
        let (found, failed) = run(lookup_in_batches(&ids, 2, |batch| {
            asked.set(asked.get() + 1);
            let fails = asked.get() == 2;
            async move {
                if fails {
                    Err(http::transient("the catalog is down"))
                } else {
                    Ok(content_ids(batch))
                }
            }
        }))?;
        // The third and fourth batch were never asked for.
        assert_eq!(asked.get(), 2);
        assert_eq!(found.len(), 2);
        assert!(failed.is_some_and(|e| http::is_transient(&e)));

        let (found, failed) = run(lookup_in_batches(&ids, 2, |batch| async move {
            Ok(content_ids(batch))
        }))?;
        assert_eq!(found.len(), 7);
        assert!(failed.is_none());
        Ok(())
    }

    #[test]
    fn parses_player_name() -> eyre::Result<()> {
        assert_eq!(
            parse_player_name(&fixture("player.json")?),
            Some("PlayerOne".to_string())
        );
        assert_eq!(parse_player_name(&fixture("not_authenticated.json")?), None);
        Ok(())
    }

    #[test]
    fn owned_games_url_carries_cursor_and_query_hash() -> eyre::Result<()> {
        let url = owned_games_url("Mw==")?;
        assert_eq!(
            http::query_param(&url, "operationName").as_deref(),
            Some("getPreloadedOwnedGames")
        );
        let variables: serde_json::Value =
            serde_json::from_str(&http::query_param(&url, "variables").unwrap_or_default())?;
        assert_eq!(variables.get("next"), Some(&serde_json::json!("Mw==")));
        assert!(http::query_param(&url, "extensions")
            .unwrap_or_default()
            .contains(OWNED_GAMES_QUERY_HASH));
        Ok(())
    }

    #[test]
    fn reads_installed_manifest_ids() {
        assert_eq!(manifest_ids("OFB-EAST%3a52735"), vec!["OFB-EAST:52735"]);
        assert_eq!(
            manifest_ids("Origin.OFR.50.0000001%2cOFB-EAST%3a52735"),
            vec!["Origin.OFR.50.0000001", "OFB-EAST:52735"]
        );
        assert!(manifest_ids("").is_empty());
    }
}
