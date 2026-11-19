//! itch.io: every game the account holds a download key for (purchases, claims, bundles,
//! gifts), installed or not.
//!
//! Uses itch's server-side API (https://itch.io/docs/api/serverside) with an API key the user
//! creates on their account settings page: `GET https://api.itch.io/profile` for the account
//! and `GET https://api.itch.io/profile/owned-keys?page=N` for the keys, each with an embedded
//! game. Playnite's ItchioLibrary reads the same data through the itch app's butler
//! (`Fetch.ProfileOwnedKeys`); this needs no itch app.
//!
//! What is assumed about the API, not verified against it: pages are numbered from 1, an
//! empty `owned_keys` list means there are no more pages, and errors come as an `errors`
//! (or `error`) field, also with a success status. Nothing here depends on the size of a
//! page.
//!
//! Like Playnite's default settings, only items classified `game` or `tool` are listed;
//! assets, soundtracks, books, comics, mods, physical games and "other" are skipped. Free
//! games the user only added to a collection have no key and are not listed (Playnite's
//! default too). A game that names its platforms and has neither Windows nor Linux among
//! them (Android only, macOS only) is skipped: BoilR runs on those two. A game that names
//! no platform (a browser game, an upload without tags) is listed.
//!
//! `OwnedGame.id` is the itch game id as a decimal string. `extra` keys:
//! - `url`: the game's page on itch.io.

use eyre::WrapErr;
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    clean_title, http, session_data, Fetch, LoginCapture, ManualLogin, OwnedGame, PendingLogin,
    Session, StoreId,
};

const API_KEYS_URL: &str = "https://itch.io/user/settings/api-keys";
const PROFILE_URL: &str = "https://api.itch.io/profile";
const OWNED_KEYS_URL: &str = "https://api.itch.io/profile/owned-keys";
/// Asked for in the hope of fewer requests. Whether the API honours `per_page`, and up to
/// what size, is not verified; the paging works with any page size.
const KEYS_PER_PAGE: &str = "500";
/// A stop for a server that never returns an empty page. Reaching it is an error.
const MAX_PAGES: usize = 1000;
/// Playnite's default `ImportGameClassification`.
const LISTED_CLASSIFICATIONS: [&str; 2] = ["game", "tool"];
/// The traits that say which platforms a game has downloads for.
const PLATFORM_TRAITS: [&str; 4] = ["p_windows", "p_linux", "p_osx", "p_android"];
/// The platforms BoilR runs on. Linux also runs Windows games, through Proton.
const PLATFORM_TRAITS_HERE: [&str; 2] = ["p_windows", "p_linux"];

/// What BoilR stores for an itch.io account. Earlier builds also wrote `user_id`, which
/// nothing read; files that have it still load. No `Debug`: the key is the login.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ItchSession {
    api_key: String,
}

pub fn begin_login() -> PendingLogin {
    PendingLogin {
        page_text_url_prefixes: vec![],
        store: StoreId::Itch,
        start_url: API_KEYS_URL.to_string(),
        // No login window: itch.io sign-in goes through an API key.
        done_url_prefixes: vec![],
        cookie_urls: vec![],
        manual: ManualLogin::Paste {
            url: API_KEYS_URL.to_string(),
            instructions: "Sign in to itch.io in your browser and open your API keys page \
                           (Settings > API keys). Click \"Generate new API key\", then \
                           \"View\" next to the new key, and paste the key here. You can \
                           revoke it on the same page at any time."
                .to_string(),
            placeholder: "itch.io API key".to_string(),
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
    let api_key = capture
        .pasted
        .as_deref()
        .and_then(parse_pasted_key)
        .ok_or_else(|| {
            eyre::format_err!(
                "That is not an itch.io API key. Copy the key from {API_KEYS_URL} and paste it."
            )
        })?;
    let user = fetch_profile(client, &api_key).await?;
    Ok(Session {
        account_name: user.account_name(),
        data: serde_json::to_value(ItchSession { api_key })?,
    })
}

pub async fn fetch_owned(
    client: &Client,
    session: &mut Session,
    _fetch: &mut Fetch<'_>,
) -> eyre::Result<Vec<OwnedGame>> {
    let data: ItchSession = session_data(StoreId::Itch, session)?;
    let api_key = data.api_key.as_str();
    let keys = http::paginate("itch.io", Page::first(), MAX_PAGES, |page| async move {
        let number = page.number.to_string();
        let request = client
            .get(OWNED_KEYS_URL)
            .query(&[("page", number.as_str()), ("per_page", KEYS_PER_PAGE)])
            .bearer_auth(api_key);
        let response = http::send(request).await?;
        let text = response.text().await.map_err(http::request_failed)?;
        read_page(&text, &page)
    })
    .await
    .wrap_err("Could not list the itch.io games")?;
    Ok(owned_games(keys))
}

async fn fetch_profile(client: &Client, api_key: &str) -> eyre::Result<User> {
    let response = http::send(client.get(PROFILE_URL).bearer_auth(api_key)).await?;
    parse_profile(&response.text().await.map_err(http::request_failed)?)
}

/// Trims the pasted key. API keys are a single token of letters and digits.
fn parse_pasted_key(pasted: &str) -> Option<String> {
    let key = pasted.trim();
    let valid = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    valid.then(|| key.to_string())
}

#[derive(Debug, Default, Deserialize)]
struct User {
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
}

impl User {
    fn account_name(&self) -> Option<String> {
        [&self.username, &self.display_name]
            .into_iter()
            .flatten()
            .map(|n| n.trim())
            .find(|n| !n.is_empty())
            .map(str::to_string)
    }
}

#[derive(Deserialize)]
struct ProfileResponse {
    user: User,
}

#[derive(Deserialize)]
struct OwnedKeysResponse {
    #[serde(default, deserialize_with = "lua_list")]
    owned_keys: Option<Vec<OwnedKey>>,
}

/// itch.io's API is written in Lua, where an empty list and an empty object are the same
/// table, so an empty list can arrive as `{}`. Accepts a list, an object of items keyed by
/// position (`{}` included), or `null`.
fn lua_list<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: DeserializeOwned,
{
    use serde::de::Error;
    use serde_json::Value;
    let items = match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Array(items)) => items,
        Some(Value::Object(map)) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by_key(|(key, _)| key.parse::<u64>().unwrap_or(u64::MAX));
            entries.into_iter().map(|(_, value)| value).collect()
        }
        Some(other) => return Err(D::Error::custom(format!("expected a list, found {other}"))),
    };
    items
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<Vec<T>, _>>()
        .map(Some)
        .map_err(D::Error::custom)
}

#[derive(Debug, Deserialize)]
struct OwnedKey {
    /// The id of the key itself.
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    game_id: Option<u64>,
    #[serde(default)]
    game: Option<Game>,
}

#[derive(Debug, Default, Deserialize)]
struct Game {
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    classification: Option<String>,
    /// `p_windows`, `p_linux`, `p_osx`, `p_android`, `can_be_bought`, `has_demo`, ...
    #[serde(default, deserialize_with = "lua_list")]
    traits: Option<Vec<String>>,
}

fn parse_profile(text: &str) -> eyre::Result<User> {
    let response: ProfileResponse = parse_api(text, "profile")?;
    Ok(response.user)
}

/// The keys on a page of `profile/owned-keys`, or `None` when the answer has no list.
fn parse_owned_keys(text: &str) -> eyre::Result<Option<Vec<OwnedKey>>> {
    let response: OwnedKeysResponse = parse_api(text, "owned games")?;
    Ok(response.owned_keys)
}

/// Where the paging of the owned keys is: the page to ask for, and what the page before it
/// held, to notice a server that answers every page number with the same page.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Page {
    number: u32,
    before: Vec<KeyId>,
}

/// What tells one key from another: the key's id and its game's id.
type KeyId = (Option<u64>, Option<u64>);

impl Page {
    fn first() -> Self {
        Self {
            number: 1,
            before: vec![],
        }
    }
}

/// Reads the answer for `page` and says which page to ask for next, or `None` after the
/// last one.
///
/// A list cut short would be saved as the whole library, and the games it lacks would be
/// removed from Steam. So only an empty list ends the paging (a short page does not: no page
/// size is assumed), or a later page without a list. An answer that is an error
/// ([`parse_api`]), a first page without a list, and a page that repeats the one before it
/// are errors.
fn read_page(text: &str, page: &Page) -> eyre::Result<(Vec<OwnedKey>, Option<Page>)> {
    let keys = match parse_owned_keys(text)? {
        Some(keys) => keys,
        None if page.number == 1 => {
            return Err(eyre::format_err!(
                "Unexpected owned games response from itch.io: it holds no list of keys. \
                 Body starts with: {}",
                http::snippet(text)
            ))
        }
        None => vec![],
    };
    if keys.is_empty() {
        return Ok((keys, None));
    }
    let ids: Vec<KeyId> = keys.iter().map(|key| (key.id, key.game_id)).collect();
    if ids == page.before {
        return Err(eyre::format_err!(
            "itch.io answered page {} with the keys of page {} again, so the list is \
             incomplete and was not saved",
            page.number,
            page.number.saturating_sub(1)
        ));
    }
    let next = Page {
        number: page.number.saturating_add(1),
        before: ids,
    };
    Ok((keys, Some(next)))
}

/// Decodes an api.itch.io response. Errors come as `{"errors": ["invalid key"]}`, which may
/// arrive with a success status. An answer with an `errors` or `error` field is an error
/// whatever the field holds (a list, a text, an object), and whatever else the answer holds.
fn parse_api<T: DeserializeOwned>(text: &str, what: &str) -> eyre::Result<T> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|err| {
        eyre::format_err!(
            "Unexpected {what} response from itch.io: {err}. Body starts with: {}",
            http::snippet(text)
        )
    })?;
    if let Some(errors) = api_errors(&value) {
        let joined = errors.messages.join("; ");
        if errors.about_the_key {
            return Err(http::login_rejected(format!(
                "itch.io rejected the API key ({joined}). Create a new key at {API_KEYS_URL} and sign in again."
            )));
        }
        return Err(eyre::format_err!("itch.io returned an error: {joined}"));
    }
    serde_json::from_value(value).map_err(|err| {
        eyre::format_err!(
            "Unexpected {what} response from itch.io: {err}. Body starts with: {}",
            http::snippet(text)
        )
    })
}

/// What an answer's `errors` and `error` fields say.
struct ApiErrors {
    messages: Vec<String>,
    /// A message, in itch's own words, is about the API key: the login is refused. Anything
    /// else (an outage, a rate limit, an error of a shape not seen before) is not.
    about_the_key: bool,
}

/// The errors of an answer, or `None` when it has neither an `errors` nor an `error` field
/// (a `null` field counts as none).
fn api_errors(answer: &serde_json::Value) -> Option<ApiErrors> {
    use serde_json::Value;
    let fields: Vec<&Value> = ["errors", "error"]
        .into_iter()
        .filter_map(|field| answer.get(field))
        .filter(|value| !value.is_null())
        .collect();
    if fields.is_empty() {
        return None;
    }
    let mut about_the_key = false;
    let mut messages = Vec::new();
    let items = fields.into_iter().flat_map(|field| match field {
        Value::Array(items) => items.iter().collect(),
        other => vec![other],
    });
    for item in items {
        let message = match item {
            Value::String(message) => {
                about_the_key |= ["key", "auth", "scope"]
                    .iter()
                    .any(|word| message.contains(word));
                message.clone()
            }
            other => http::snippet(&other.to_string()),
        };
        if !message.trim().is_empty() {
            messages.push(message);
        }
    }
    if messages.is_empty() {
        messages.push("no details given".to_string());
    }
    Some(ApiErrors {
        messages,
        about_the_key,
    })
}

/// One `OwnedGame` per listed game, in key order, without duplicates.
fn owned_games(keys: Vec<OwnedKey>) -> Vec<OwnedGame> {
    let mut seen = std::collections::BTreeSet::new();
    let mut games = Vec::new();
    for key in keys {
        let Some(game) = key.game else {
            continue;
        };
        let Some(id) = game.id.or(key.game_id) else {
            continue;
        };
        let title = game.title.as_deref().map(clean_title).unwrap_or_default();
        if title.is_empty()
            || !is_listed(game.classification.as_deref())
            || !runs_here(game.traits.as_deref().unwrap_or_default())
            || !seen.insert(id)
        {
            continue;
        }
        let mut owned = OwnedGame::new(StoreId::Itch, id.to_string(), title);
        if let Some(url) = game.url.as_deref().filter(|u| !u.is_empty()) {
            owned = owned.with_extra("url", url);
        }
        games.push(owned);
    }
    games
}

/// A missing classification is taken as a game; unknown ones are skipped.
fn is_listed(classification: Option<&str>) -> bool {
    classification.is_none_or(|c| LISTED_CLASSIFICATIONS.contains(&c))
}

/// Whether a game with these traits can run where BoilR runs: it has a Windows or a Linux
/// download, or it names no platform at all (a browser game, an upload without tags), which
/// says nothing against it.
fn runs_here(traits: &[String]) -> bool {
    let mut platforms = traits
        .iter()
        .filter(|t| PLATFORM_TRAITS.contains(&t.as_str()))
        .peekable();
    platforms.peek().is_none() || platforms.any(|t| PLATFORM_TRAITS_HERE.contains(&t.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = include_str!("../testdata/itch/profile.json");
    const OWNED_KEYS: &str = include_str!("../testdata/itch/owned_keys_page1.json");
    const OWNED_KEYS_EMPTY: &str = include_str!("../testdata/itch/owned_keys_empty.json");
    const INVALID_KEY: &str = include_str!("../testdata/itch/invalid_key.json");

    fn keys(text: &str) -> eyre::Result<Vec<OwnedKey>> {
        Ok(parse_owned_keys(text)?.unwrap_or_default())
    }

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
    fn has_no_login_window() {
        let pending = begin_login();
        assert!(pending.done_url_prefixes.is_empty());
        assert!(matches!(pending.manual, ManualLogin::Paste { .. }));
    }

    #[test]
    fn reads_profile() -> eyre::Result<()> {
        let user = parse_profile(PROFILE)?;
        assert_eq!(user.account_name(), Some("leafo".to_string()));
        Ok(())
    }

    #[test]
    fn sessions_saved_by_earlier_builds_still_load() -> eyre::Result<()> {
        let saved = Session {
            account_name: Some("leafo".into()),
            data: serde_json::json!({ "api_key": "AbC123", "user_id": 3423 }),
        };
        let data: ItchSession = session_data(StoreId::Itch, &saved)?;
        assert_eq!(data.api_key, "AbC123");
        // What is written now has the key only.
        assert_eq!(
            serde_json::to_value(&data)?,
            serde_json::json!({ "api_key": "AbC123" })
        );

        let broken = Session {
            account_name: None,
            data: serde_json::json!({ "user_id": 3423 }),
        };
        let unreadable = session_data::<ItchSession>(StoreId::Itch, &broken);
        assert!(unreadable.is_err_and(|e| http::is_login_rejected(&e)));
        Ok(())
    }

    #[test]
    fn invalid_key_is_a_readable_error() {
        let err = parse_profile(INVALID_KEY).err();
        assert!(err.is_some_and(|e| {
            http::is_login_rejected(&e) && e.to_string().contains("rejected the API key")
        }));
    }

    #[test]
    fn an_answer_with_an_error_field_is_an_error_whatever_its_shape() {
        let error_of = |text: &str| keys(text).err().map(|e| e.to_string());
        // A text instead of a list, and the singular name.
        assert!(
            error_of(r#"{"errors": "rate limited"}"#).is_some_and(|e| e.contains("rate limited"))
        );
        assert!(error_of(r#"{"error": "server busy"}"#).is_some_and(|e| e.contains("server busy")));
        // An object, a list of objects, an empty list.
        assert!(error_of(r#"{"errors": {"code": 500}}"#).is_some_and(|e| e.contains("500")));
        assert!(error_of(r#"{"errors": [{"message": "broken"}]}"#)
            .is_some_and(|e| e.contains("broken")));
        assert!(error_of(r#"{"errors": []}"#).is_some_and(|e| e.contains("no details")));
        assert!(error_of(r#"{"error": true}"#).is_some());
        // Also next to a list of keys: an answer does not get to be both.
        assert!(error_of(r#"{"errors": ["failed"], "owned_keys": []}"#).is_some());

        // These say nothing about the API key.
        let busy = keys(r#"{"errors": "rate limited"}"#).err();
        assert!(busy.is_some_and(|e| !http::is_login_rejected(&e)));
        // This one does.
        let revoked = keys(r#"{"error": "invalid key"}"#).err();
        assert!(revoked.is_some_and(|e| http::is_login_rejected(&e)));
        // An error of an unknown shape is not taken for a refused key, whatever its fields
        // are called.
        let odd = keys(r#"{"errors": {"api_key": "x", "reason": "maintenance"}}"#).err();
        assert!(odd.is_some_and(|e| !http::is_login_rejected(&e)));

        // A null field is no error.
        assert!(keys(r#"{"errors": null, "owned_keys": []}"#).is_ok());
    }

    #[test]
    fn lists_games_and_tools_and_skips_other_classifications() -> eyre::Result<()> {
        let games = owned_games(keys(OWNED_KEYS)?);
        // 999 is a second key for 284880, 555 an asset pack, 556 a soundtrack, 557 has no game.
        assert_eq!(ids(&games), vec!["284880", "123456", "777"]);

        let pikuniku = games.first();
        assert_eq!(pikuniku.map(|g| g.title.as_str()), Some("Pikuniku"));
        assert_eq!(
            pikuniku.and_then(|g| g.extra("url")),
            Some("https://sectordub.itch.io/pikuniku")
        );
        // Only the extra the install target reads is written.
        let extras: Vec<&str> = pikuniku
            .map(|g| g.extra.keys().map(String::as_str).collect())
            .unwrap_or_default();
        assert_eq!(extras, ["url"]);

        let tool = games.get(1);
        assert_eq!(tool.map(|g| g.title.as_str()), Some("Pixel Tool"));
        // A browser game names no platform and stays.
        let web = games.get(2);
        assert_eq!(web.map(|g| g.title.as_str()), Some("Browser Jam Game"));
        Ok(())
    }

    #[test]
    fn games_for_other_platforms_only_are_skipped() -> eyre::Result<()> {
        let traits =
            |traits: &[&str]| -> Vec<String> { traits.iter().map(|t| t.to_string()).collect() };
        assert!(runs_here(&traits(&["p_windows"])));
        assert!(runs_here(&traits(&["p_linux", "has_demo"])));
        assert!(runs_here(&traits(&["p_osx", "p_android", "p_windows"])));
        assert!(!runs_here(&traits(&["p_android"])));
        assert!(!runs_here(&traits(&["p_osx", "can_be_bought"])));
        assert!(!runs_here(&traits(&["p_osx", "p_android"])));
        // No platform named: a browser game, or an upload nobody tagged.
        assert!(runs_here(&traits(&[])));
        assert!(runs_here(&traits(&["can_be_bought", "has_demo"])));

        let games = owned_games(keys(
            r#"{"owned_keys": [
                {"id": 1, "game_id": 1, "game": {"id": 1, "title": "Phone Game", "traits": ["p_android"]}},
                {"id": 2, "game_id": 2, "game": {"id": 2, "title": "Mac Game", "traits": ["p_osx"]}},
                {"id": 3, "game_id": 3, "game": {"id": 3, "title": "Penguin Game", "traits": ["p_linux"]}},
                {"id": 4, "game_id": 4, "game": {"id": 4, "title": "Untagged", "traits": {}}},
                {"id": 5, "game_id": 5, "game": {"id": 5, "title": "No Traits"}}
            ]}"#,
        )?);
        assert_eq!(ids(&games), vec!["3", "4", "5"]);
        Ok(())
    }

    #[test]
    fn reads_lists_the_way_lua_writes_them() -> eyre::Result<()> {
        // Lua sends an empty list as an empty object.
        assert!(keys("{\"page\": 3, \"owned_keys\": {}}")?.is_empty());
        assert!(parse_owned_keys("{\"page\": 3, \"owned_keys\": {}}")?.is_some());
        assert!(parse_owned_keys("{\"page\": 3, \"per_page\": 50}")?.is_none());
        assert!(parse_owned_keys("{\"owned_keys\": null}")?.is_none());
        let keys = keys(
            r#"{"owned_keys": {"2": {"game_id": 2, "game": {"id": 2, "title": "B", "traits": {}}},
                               "1": {"game_id": 1, "game": {"id": 1, "title": "A", "traits": ["p_windows"]}}}}"#,
        )?;
        let ids: Vec<Option<u64>> = keys.iter().map(|k| k.game_id).collect();
        assert_eq!(ids, vec![Some(1), Some(2)]);
        let traits = keys
            .last()
            .and_then(|k| k.game.as_ref())
            .and_then(|g| g.traits.clone());
        assert_eq!(traits, Some(vec![]));
        Ok(())
    }

    /// A page of keys with these key ids, each for the game with the same id.
    fn page_of(ids: &[u64]) -> String {
        let keys: Vec<serde_json::Value> = ids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "id": id,
                    "game_id": id,
                    "game": { "id": id, "title": format!("Game {id}") },
                })
            })
            .collect();
        serde_json::json!({ "owned_keys": keys }).to_string()
    }

    #[test]
    fn only_an_empty_page_ends_the_paging() -> eyre::Result<()> {
        let first = Page::first();
        assert_eq!(first.number, 1);

        // A page with keys is followed by the next number, however short it is.
        let (keys, next) = read_page(&page_of(&[1]), &first)?;
        assert_eq!(keys.len(), 1);
        let second =
            next.ok_or_else(|| eyre::format_err!("the paging ended on a page with keys"))?;
        assert_eq!(second.number, 2);

        // An empty list ends it: as a list, as Lua's empty object, as the fixture.
        for empty in [
            r#"{"owned_keys": []}"#,
            r#"{"owned_keys": {}}"#,
            OWNED_KEYS_EMPTY,
        ] {
            let (keys, next) = read_page(empty, &second)?;
            assert!(keys.is_empty());
            assert_eq!(next, None);
        }
        // An account without keys.
        let (none, next) = read_page(r#"{"owned_keys": []}"#, &first)?;
        assert!(none.is_empty() && next.is_none());

        // A later page without a list ends it too.
        let (keys, next) = read_page(r#"{"page": 2, "per_page": 500}"#, &second)?;
        assert!(keys.is_empty() && next.is_none());
        Ok(())
    }

    #[test]
    fn an_answer_that_is_no_page_of_keys_fails_the_list() -> eyre::Result<()> {
        let first = Page::first();
        let (_, next) = read_page(&page_of(&[1, 2]), &first)?;
        let second =
            next.ok_or_else(|| eyre::format_err!("the paging ended on a page with keys"))?;

        // The first page must hold a list: an answer without one is not an empty account.
        let no_list = read_page(r#"{"page": 1, "per_page": 500}"#, &first);
        assert!(no_list.is_err_and(|e| e.to_string().contains("no list of keys")));
        assert!(read_page("{}", &first).is_err());

        // An error in the middle of the list is not its end.
        for answer in [
            r#"{"error": "server busy"}"#,
            r#"{"errors": "rate limited"}"#,
            r#"{"errors": ["something broke"]}"#,
        ] {
            assert!(read_page(answer, &second).is_err(), "{answer}");
            assert!(read_page(answer, &first).is_err(), "{answer}");
        }
        assert!(read_page("<html>Bad gateway</html>", &second).is_err());
        Ok(())
    }

    #[test]
    fn a_page_that_repeats_the_one_before_fails_the_list() -> eyre::Result<()> {
        let (_, next) = read_page(&page_of(&[1, 2]), &Page::first())?;
        let second =
            next.ok_or_else(|| eyre::format_err!("the paging ended on a page with keys"))?;

        // A server that ignores `page` answers every number with the first page.
        let repeated = read_page(&page_of(&[1, 2]), &second);
        assert!(repeated.is_err_and(|e| {
            let message = e.to_string();
            message.contains("page 2")
                && message.contains("page 1")
                && message.contains("incomplete")
        }));

        // Other keys are another page, also when it starts with the same key.
        let (keys, next) = read_page(&page_of(&[1, 3]), &second)?;
        assert_eq!(keys.len(), 2);
        assert_eq!(next.map(|page| page.number), Some(3));
        Ok(())
    }

    #[test]
    fn reads_pasted_key() {
        assert_eq!(parse_pasted_key("  AbC123xyz \n"), Some("AbC123xyz".into()));
        assert_eq!(parse_pasted_key(""), None);
        assert_eq!(parse_pasted_key("not a key"), None);
    }
}
