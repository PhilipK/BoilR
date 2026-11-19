//! Battle.net. Follows Playnite's BattleNetLibrary
//! (github.com/JosefNemec/PlayniteExtensions, `source/Libraries/BattleNetLibrary`).
//!
//! Login: the window starts the account site's OAuth sign-in
//! (`account.battle.net/oauth2/authorization/account-settings`). After the user signs in,
//! oauth.battle.net redirects to `account.battle.net/callback/oauth2/code/account-settings`;
//! by then the long-lived Battle.net sign-in cookies (on `.battle.net`) are set. BoilR keeps
//! the cookies a browser would send to the hosts that sign-in passes through
//! (`SESSION_HOSTS`) and none of Blizzard's other hosts. The account site's own session
//! lasts 30 minutes, so every refresh replays that OAuth sign-in with the kept cookies (as
//! Playnite does) before calling the account API.
//!
//! Library: `account.battle.net/api/games-and-subs` lists game accounts by Blizzard title id;
//! the product table (`PRODUCTS`) maps them to launcher product codes. A game the table lacks
//! is listed under the name the account site gives it, and opens the download page.
//! `api/classic-games` lists the CD-key classics (Warcraft III, Diablo II).
//!
//! Not supported: the China region (battlenet.com.cn), which has its own account site.
//!
//! [`OwnedGame::id`] is:
//! - for a game in the table, its launcher product code (`Pro`, `WoW`, `WTCG`), the code
//!   `Battle.net.exe --exec="launch <code>"` takes;
//! - for a classic game, `W3C`, `W3CX`, `D2` or `D2X`. These are not launcher codes (the
//!   launcher does not know the classics): they are the ids Playnite gives them;
//! - for a game the table lacks, the code its title id spells (see `code_of_title`), which
//!   is the launcher code for most games, or `title-<title id>` when it spells none.
//!
//! `extra` keys:
//! - `internal_id`: the launcher's internal id (`prometheus`), taken by
//!   `Battle.net.exe --game=<internal_id>` (opens the game's page, with its Install button).
//!   Absent for classic games, which are installed from Blizzard's site with a CD key, and
//!   for games the table lacks: without it a shortcut opens the download page.

use eyre::WrapErr;
use reqwest::{cookie::Jar, Client};
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};

use crate::{
    http::{self, cookie_jar},
    Cookie, Fetch, LoginCapture, ManualLogin, OwnedGame, PendingLogin, Session, StoreId,
};

/// Starts the account site's OAuth sign-in. Also replayed on every refresh.
const SIGN_IN_URL: &str = "https://account.battle.net/oauth2/authorization/account-settings";
/// Where oauth.battle.net sends the browser once the user is signed in.
const LOGIN_DONE_PREFIX: &str = "https://account.battle.net/callback/oauth2/code/";
const ACCOUNT_HOST: &str = "account.battle.net";
/// The hosts a sign-in passes through: the account site BoilR calls, the OAuth service it
/// redirects to, and the regional login hosts that service sends the browser to. The login
/// window is asked for their cookies, only cookies a browser would send to one of them are
/// kept, and renewed cookies are read back for each.
///
/// `battle.net` comes first, so that a cookie for the whole site is read back once, for
/// that domain, and not once per host.
const SESSION_HOSTS: [&str; 7] = [
    "battle.net",
    ACCOUNT_HOST,
    "oauth.battle.net",
    "us.account.battle.net",
    "eu.account.battle.net",
    "kr.account.battle.net",
    "tw.account.battle.net",
];
const API_STATUS_URL: &str = "https://account.battle.net/api/";
const GAMES_URL: &str = "https://account.battle.net/api/games-and-subs";
const CLASSIC_GAMES_URL: &str = "https://account.battle.net/api/classic-games";
const DETAILS_URL: &str = "https://account.battle.net/api/details";

/// A game the Battle.net account API can return. Ported from Playnite's `BattleNetGames.cs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Product {
    /// Launcher product code (`--exec="launch <code>"`); for a classic game, Playnite's id.
    code: &'static str,
    /// Launcher internal id (`--game=<internal_id>`). `None` for classic games.
    internal_id: Option<&'static str>,
    name: &'static str,
    /// Blizzard title id in `api/games-and-subs`. `None` for classic games.
    title_id: Option<u64>,
}

impl Product {
    /// A CD-key game from before the launcher, which neither installs nor starts it.
    fn is_classic(&self) -> bool {
        self.title_id.is_none()
    }
}

const fn launcher(
    code: &'static str,
    internal_id: &'static str,
    name: &'static str,
    title_id: u64,
) -> Product {
    Product {
        code,
        internal_id: Some(internal_id),
        name,
        title_id: Some(title_id),
    }
}

const fn classic(code: &'static str, name: &'static str) -> Product {
    Product {
        code,
        internal_id: None,
        name,
        title_id: None,
    }
}

/// Where a title id repeats, the account entry's name chooses ([`product_for_title`]).
const PRODUCTS: &[Product] = &[
    launcher("WoW", "wow", "World of Warcraft", 5730135),
    launcher("D3", "diablo3", "Diablo III", 17459),
    launcher("S2", "s2", "StarCraft II", 21298),
    launcher("S1", "s1", "StarCraft", 21297),
    launcher("WTCG", "hs_beta", "Hearthstone", 1465140039),
    launcher("Hero", "heroes", "Heroes of the Storm", 1214607983),
    launcher("Pro", "prometheus", "Overwatch 2", 5272175),
    classic("D2", "Diablo II"),
    classic("D2X", "Diablo II: Lord of Destruction"),
    launcher("VIPR", "viper", "Call of Duty: Black Ops 4", 1447645266),
    launcher("ODIN", "odin", "Call of Duty: Modern Warfare", 1329875278),
    classic("W3C", "Warcraft III: Reign of Chaos"),
    classic("W3CX", "Warcraft III: The Frozen Throne"),
    launcher("W3", "w3", "Warcraft III: Reforged", 22323),
    launcher(
        "LAZR",
        "lazarus",
        "Call of Duty: Modern Warfare 2 Campaign Remastered",
        1279351378,
    ),
    launcher(
        "ZEUS",
        "zeus",
        "Call of Duty: Black Ops Cold War",
        1514493267,
    ),
    launcher("WLBY", "wlby", "Crash Bandicoot 4", 1464615513),
    launcher("OSI", "osi", "Diablo II: Resurrected", 5198665),
    launcher("RTRO", "rtro", "Blizzard Arcade Collection", 1381257807),
    launcher("FORE", "fore", "Call of Duty: Vanguard", 1179603525),
    launcher("ANBS", "anbs", "Diablo Immortal", 1095647827),
    // Modern Warfare III (PNTA, below) has the same title id: both live in the Call of
    // Duty launcher entry.
    launcher(
        "AUKS",
        "auks",
        "Call of Duty: Modern Warfare II",
        1096108883,
    ),
    launcher("Fen", "Fen", "Diablo IV", 4613486),
    launcher("D1", "D1", "Diablo", 1146246220),
    launcher("W1R", "w1r", "Warcraft: Remastered", 5714258),
    launcher("W2R", "w2r", "Warcraft II: Remastered", 5714514),
    launcher("W1", "w1", "Warcraft: Orcs & Humans", 1463898673),
    launcher("W2", "w2", "Warcraft II: Battle.net Edition", 1462911566),
    launcher("GRY", "gryphon", "Warcraft Rumble", 4674137),
    launcher("ARIS", "aris", "Doom: The Dark Ages", 1095911763),
    launcher("SCOR", "scorpio", "Sea of Thieves", 1396920146),
    launcher("ARK", "arkansas", "The Outer Worlds 2", 4280907),
    launcher("LBRA", "libra", "Tony Hawk's Pro Skater 3 + 4", 1279414849),
    launcher(
        "PNTA",
        "pinta",
        "Call of Duty: Modern Warfare III",
        1096108883,
    ),
    launcher("AQUA", "aqua", "Avowed", 1095849281),
];

/// The product with this code (case-insensitive).
fn product(code: &str) -> Option<&'static Product> {
    PRODUCTS.iter().find(|p| p.code.eq_ignore_ascii_case(code))
}

/// The code a title id spells: read as bytes, most significant first, a title id is ASCII
/// text (5272175 is `Pro`, 1096108883 is `AUKS`). For most games that text is the launcher
/// product code. `None` when the bytes are not letters and digits.
fn code_of_title(title_id: u64) -> Option<String> {
    let bytes: Vec<u8> = title_id
        .to_be_bytes()
        .into_iter()
        .skip_while(|byte| *byte == 0)
        .collect();
    let spells_a_code = !bytes.is_empty() && bytes.iter().all(u8::is_ascii_alphanumeric);
    spells_a_code.then(|| bytes.into_iter().map(char::from).collect())
}

/// A name's words, lowercased, without trademark signs and punctuation.
fn words(name: &str) -> Vec<String> {
    crate::clean_title(name)
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

/// The product of a game account. Modern Warfare II and III share a title id, which alone
/// cannot tell them apart: of the products with this title id, the one whose every word is
/// in the name the account site gives the entry is taken, and the first one in the table
/// when the name does not help (as Playnite always does).
fn product_for_title(title_id: u64, localized_name: Option<&str>) -> Option<&'static Product> {
    let mut sharing = PRODUCTS.iter().filter(|p| p.title_id == Some(title_id));
    let Some(first) = sharing.next() else {
        // The table may know the game by the code this title id spells.
        return code_of_title(title_id)
            .and_then(|code| product(&code))
            .filter(|p| !p.is_classic());
    };
    let named = localized_name.map(words).unwrap_or_default();
    std::iter::once(first)
        .chain(sharing)
        .find(|p| {
            let name = words(p.name);
            !name.is_empty() && name.iter().all(|word| named.contains(word))
        })
        .or(Some(first))
}

/// What the session file holds.
#[derive(Clone, Default, Serialize, Deserialize)]
struct BattleNetSession {
    cookies: Vec<Cookie>,
}

pub fn begin_login() -> PendingLogin {
    PendingLogin {
        page_text_url_prefixes: vec![],
        store: StoreId::BattleNet,
        start_url: SIGN_IN_URL.to_string(),
        done_url_prefixes: vec![LOGIN_DONE_PREFIX.to_string()],
        cookie_urls: SESSION_HOSTS
            .iter()
            .map(|host| format!("https://{host}/"))
            .collect(),
        // The sign-in lives in HttpOnly cookies, which a user cannot reasonably copy.
        manual: ManualLogin::Unsupported,
        user_agent: Some(http::BROWSER_USER_AGENT.to_string()),
        state: serde_json::Value::Null,
    }
}

/// The cookies of a sign-in that BoilR keeps: the ones a browser would send to a host the
/// sign-in passes through. Blizzard's other hosts (its shop, its forums) are not contacted.
fn session_cookies(captured: &[Cookie]) -> Vec<Cookie> {
    cookie_jar::keep_for_hosts(captured, &SESSION_HOSTS)
}

pub async fn complete_login(
    _client: &Client,
    _pending: &PendingLogin,
    capture: &LoginCapture,
) -> eyre::Result<Session> {
    let mut cookies = session_cookies(&capture.cookies);
    if cookies.is_empty() {
        return Err(eyre::format_err!(
            "The Battle.net sign-in window handed over no battle.net cookies, so the sign-in \
             could not be kept."
        ));
    }
    let (client, jar) = cookie_jar::client_with(&cookies)?;
    sign_in_account_site(&client)
        .await
        .wrap_err("Signing in to Battle.net did not finish")?;
    let account_name = battletag(&client, &jar).await;
    cookie_jar::refresh(&mut cookies, &jar, &SESSION_HOSTS);
    Ok(Session {
        account_name,
        data: serde_json::to_value(BattleNetSession { cookies })?,
    })
}

pub async fn fetch_owned(
    _client: &Client,
    session: &mut Session,
    fetch: &mut Fetch<'_>,
) -> eyre::Result<Vec<OwnedGame>> {
    let mut data: BattleNetSession = crate::session_data(StoreId::BattleNet, session)?;
    let (client, jar) = cookie_jar::client_with(&data.cookies)?;
    let signed_in = sign_in_account_site(&client).await;
    // Battle.net may have renewed its sign-in cookies; keep them even when something fails.
    cookie_jar::refresh(&mut data.cookies, &jar, &SESSION_HOSTS);
    session.data = serde_json::to_value(&data)?;
    signed_in?;

    if let Some(tag) = battletag(&client, &jar).await {
        session.account_name = Some(tag);
    }
    let games: GamesAndSubs = api_get(&client, &jar, GAMES_URL).await?;
    // The classic CD-key games are a small extra: when only their list fails, the ones from
    // the previous list are kept instead of failing the whole library.
    let classic = match api_get::<ClassicGames>(&client, &jar, CLASSIC_GAMES_URL).await {
        Ok(classic) => Some(classic),
        Err(err) if http::is_login_rejected(&err) => return Err(err),
        Err(err) => {
            eprintln!(
                "Could not list the Battle.net classic games, keeping the ones from before: {}",
                http::redact(&format!("{err:#}"))
            );
            fetch.partial = true;
            None
        }
    };
    let owned = owned_games(&games, classic.as_ref().unwrap_or(&ClassicGames::default()))?;
    Ok(match classic {
        Some(_) => owned,
        None => with_previous_classic_games(owned, fetch.previous),
    })
}

fn with_previous_classic_games(
    mut owned: Vec<OwnedGame>,
    previous: &[OwnedGame],
) -> Vec<OwnedGame> {
    for game in previous {
        let Some(classic) = product(&game.id).filter(|p| p.is_classic()) else {
            continue;
        };
        if !owned.iter().any(|o| o.id == classic.code) {
            owned.push(to_owned_game(classic));
        }
    }
    owned
}

/// Replays the account site's OAuth sign-in with the saved cookies, which gives the client a
/// fresh account-site session, then checks that the account API sees a signed-in user.
async fn sign_in_account_site(client: &Client) -> eyre::Result<()> {
    // Ends on the account site when the saved sign-in is still good, on the login page when
    // not; both answer 200, so the status check below decides.
    let replay = client
        .get(SIGN_IN_URL)
        .send()
        .await
        .map_err(http::request_failed)?;
    let replay_failed = if replay.status().is_success() {
        None
    } else {
        Some(http::status_error(replay).await)
    };
    let response = client
        .get(API_STATUS_URL)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(http::request_failed)?;
    let status = response.status().as_u16();
    let body = response.text().await.map_err(http::request_failed)?;
    after_replay(replay_failed, signed_in(status, &body))
}

/// The account's BattleTag, if the account site hands it out. Never fails a refresh.
async fn battletag(client: &Client, jar: &Jar) -> Option<String> {
    let details: AccountDetails = api_get(client, jar, DETAILS_URL).await.ok()?;
    details.battle_tag.filter(|tag| !tag.trim().is_empty())
}

/// A GET on the account API, with the headers its own web page sends.
async fn api_get<T: DeserializeOwned>(client: &Client, jar: &Jar, url: &str) -> eyre::Result<T> {
    let mut request = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json");
    if let Some((_, xsrf)) = cookie_jar::pairs_for(jar, ACCOUNT_HOST)
        .into_iter()
        .find(|(name, _)| name == "XSRF-TOKEN")
    {
        request = request.header("X-XSRF-TOKEN", xsrf);
    }
    http::send_json(request).await
}

// ---- Reading answers (pure) ----

#[derive(Debug, Deserialize)]
struct ApiStatus {
    authenticated: Option<bool>,
}

/// Whether the account API's status answer says there is a signed-in user. `None` when the
/// answer does not say: it is no JSON, or JSON about something else (an error object).
fn parse_api_status(body: &str) -> Option<bool> {
    serde_json::from_str::<ApiStatus>(body).ok()?.authenticated
}

/// What the account API's status answer means for the saved sign-in. Without a sign-in the
/// API answers 401 and `authenticated: false`. An outage, a rate limit or a bot block says
/// nothing about the sign-in, so none of them asks the user to sign in again.
fn signed_in(status: u16, body: &str) -> eyre::Result<()> {
    let success = (200..300).contains(&status);
    match parse_api_status(body) {
        Some(true) => Ok(()),
        Some(false) if success => Err(sign_in_refused()),
        _ if status == 401 => Err(sign_in_refused()),
        _ if http::is_transient_status(status) => Err(eyre::Report::new(http::HttpStatus {
            status,
            url: API_STATUS_URL.to_string(),
            body: http::snippet(body),
        })),
        _ => Err(eyre::format_err!(
            "Unexpected answer from the Battle.net account site (HTTP {status}): {}",
            http::snippet(body)
        )),
    }
}

fn sign_in_refused() -> eyre::Report {
    http::login_rejected("Battle.net no longer accepts the saved sign-in. Sign in again.")
}

/// The outcome of a sign-in check, given how the replay before it went. When the replay
/// itself failed (an outage, a bot block), the account site has no session whatever the
/// saved sign-in is worth: a refusal then says nothing, and the replay's failure is reported.
fn after_replay(
    replay_failed: Option<eyre::Report>,
    checked: eyre::Result<()>,
) -> eyre::Result<()> {
    match (checked, replay_failed) {
        (Err(refused), Some(replay)) if http::is_login_rejected(&refused) => {
            Err(replay.wrap_err("The Battle.net sign-in could not be renewed"))
        }
        (checked, _) => checked,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountDetails {
    battle_tag: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GamesAndSubs {
    /// Must be there (`null` counts as empty): an answer without it is something else than
    /// the game list, and must not be read as an account without games.
    #[serde(deserialize_with = "list_or_null")]
    game_accounts: Vec<GameAccount>,
}

fn list_or_null<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameAccount {
    title_id: Option<u64>,
    /// The game's name in the account site's language, with trademark signs.
    localized_game_name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClassicGames {
    classic_games: Option<Vec<ClassicGame>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClassicGame {
    regional_game_franchise_icon_filename: Option<String>,
}

fn owned_games(games: &GamesAndSubs, classic: &ClassicGames) -> eyre::Result<Vec<OwnedGame>> {
    let accounts = &games.game_accounts;
    if !accounts.is_empty() && accounts.iter().all(|a| a.title_id.is_none()) {
        return Err(eyre::format_err!(
            "Unexpected answer from the Battle.net account site: none of its {} game accounts \
             has a title id",
            accounts.len()
        ));
    }

    let mut owned: Vec<OwnedGame> = Vec::new();
    for account in accounts {
        let Some(title_id) = account.title_id else {
            continue;
        };
        let name = account.localized_game_name.as_deref();
        let known = product_for_title(title_id, name);
        let Some(game) = known
            .map(to_owned_game)
            .or_else(|| unknown_game(title_id, name))
        else {
            eprintln!(
                "Battle.net lists a game BoilR does not know and gives it no name (title id \
                 {title_id}); it is skipped."
            );
            continue;
        };
        // Several game accounts (two WoW licences, say) are one game.
        if owned.iter().any(|o| o.id == game.id) {
            continue;
        }
        if known.is_none() {
            eprintln!(
                "Battle.net lists a game BoilR does not know (title id {title_id}); it is \
                 added as \"{}\" and opens the download page.",
                game.title
            );
        }
        owned.push(game);
    }

    // As Playnite: classic games only show up as CD keys, told apart by their icon. One
    // Warcraft III key is Reign of Chaos, a second one The Frozen Throne; likewise Diablo II
    // and Lord of Destruction.
    let icons: Vec<&str> = classic
        .classic_games
        .iter()
        .flatten()
        .filter_map(|g| g.regional_game_franchise_icon_filename.as_deref())
        .collect();
    let count = |franchise: &str| {
        icons
            .iter()
            .filter(|icon| is_franchise(icon, franchise))
            .count()
    };
    for (franchise, base, expansion) in
        [("warcraft-iii", "W3C", "W3CX"), ("diablo-ii", "D2", "D2X")]
    {
        for classic in [base, expansion]
            .into_iter()
            .take(count(franchise))
            .filter_map(product)
        {
            if !owned.iter().any(|o| o.id == classic.code) {
                owned.push(to_owned_game(classic));
            }
        }
    }

    Ok(owned)
}

/// Whether an icon file name belongs to a franchise, without `diablo-ii` matching
/// `diablo-iii`.
fn is_franchise(icon: &str, franchise: &str) -> bool {
    let icon = icon.to_ascii_lowercase();
    icon.match_indices(franchise).any(|(start, _)| {
        !icon
            .get(start + franchise.len()..)
            .is_some_and(|rest| rest.starts_with('i'))
    })
}

fn to_owned_game(product: &Product) -> OwnedGame {
    let game = OwnedGame::new(StoreId::BattleNet, product.code, product.name);
    match product.internal_id {
        Some(internal_id) => game.with_extra("internal_id", internal_id),
        None => game,
    }
}

/// A game the product table lacks (one Blizzard added since), from its account entry. It
/// has no `internal_id`, so its shortcut opens the download page. `None` when the entry has
/// no name to list it under.
fn unknown_game(title_id: u64, localized_name: Option<&str>) -> Option<OwnedGame> {
    let title = crate::clean_title(localized_name?);
    if title.is_empty() {
        return None;
    }
    let id = code_of_title(title_id)
        // A code the table has for another game (a classic one) is not this game's.
        .filter(|code| product(code).is_none())
        .unwrap_or_else(|| format!("title-{title_id}"));
    Some(OwnedGame::new(StoreId::BattleNet, id, title))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture<T: DeserializeOwned>(name: &str) -> eyre::Result<T> {
        Ok(serde_json::from_str(&fixture_text(name)?)?)
    }

    fn fixture_text(name: &str) -> eyre::Result<String> {
        let path = format!("{}/testdata/battlenet/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(&path).wrap_err(path)
    }

    fn cookie(name: &str, value: &str, domain: &str) -> Cookie {
        Cookie {
            name: name.into(),
            value: value.into(),
            domain: domain.into(),
        }
    }

    fn accounts(json: &str) -> eyre::Result<Vec<OwnedGame>> {
        owned_games(&serde_json::from_str(json)?, &ClassicGames::default())
    }

    fn ids(games: &[OwnedGame]) -> Vec<&str> {
        games.iter().map(|g| g.id.as_str()).collect()
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
        // The pages the sign-in goes through (seen 2026-11-08).
        assert!(!done("https://oauth.battle.net/authorize?response_type=code&client_id=x&redirect_uri=https://account.battle.net/callback/oauth2/code/account-settings"));
        assert!(!done("https://eu.account.battle.net/login/en-us/?ref=https://oauth.battle.net/authorize?redirect_uri%3Dhttps://account.battle.net/callback/oauth2/code/account-settings&app=bas"));
        assert!(!done("https://account.battle.net/overview"));
        assert!(done(
            "https://account.battle.net/callback/oauth2/code/account-settings?code=abc&state=x"
        ));
    }

    #[test]
    fn the_login_window_is_asked_for_the_cookies_of_every_host_in_the_sign_in() {
        let pending = begin_login();
        for host in [
            "account.battle.net",
            "oauth.battle.net",
            "eu.account.battle.net",
        ] {
            let url = format!("https://{host}/");
            assert!(pending.cookie_urls.contains(&url), "{url}");
        }
        assert_eq!(pending.cookie_urls.len(), SESSION_HOSTS.len());
        // The session is replayed under the user agent it was made with.
        assert_eq!(
            pending.user_agent.as_deref(),
            Some(http::BROWSER_USER_AGENT)
        );
    }

    #[test]
    fn only_cookies_a_browser_would_send_during_the_sign_in_are_kept() -> eyre::Result<()> {
        // Domains as a login window reports them: `.battle.net` comes as `battle.net`.
        let captured = vec![
            cookie("site", "1", "battle.net"),
            cookie("oauth", "2", "oauth.battle.net"),
            cookie("region", "3", "eu.battle.net"),
            cookie("session", "4", "account.battle.net"),
            cookie("login", "5", "eu.account.battle.net"),
            cookie("shop", "6", "shop.battle.net"),
            cookie("tracker", "7", "example.com"),
            cookie("lookalike", "8", "notbattle.net"),
        ];
        let kept = session_cookies(&captured);
        let names: Vec<&str> = kept.iter().map(|c| c.name.as_str()).collect();
        // `eu.battle.net` is not `eu.account.battle.net`: no host of the sign-in gets it.
        assert_eq!(names, vec!["site", "oauth", "session", "login"]);

        // The site-wide cookie reaches the OAuth service, where the sign-in is replayed.
        let (_client, jar) = cookie_jar::client_with(&kept)?;
        let mut sent: Vec<String> = cookie_jar::pairs_for(&jar, "oauth.battle.net")
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        sent.sort();
        assert_eq!(sent, vec!["oauth", "site"]);
        Ok(())
    }

    #[test]
    fn reads_sign_in_status() -> eyre::Result<()> {
        let anonymous = fixture_text("api_status_anonymous.json")?;
        let authenticated = fixture_text("api_status_authenticated.json")?;
        assert_eq!(parse_api_status(&anonymous), Some(false));
        assert_eq!(parse_api_status(&authenticated), Some(true));
        assert_eq!(parse_api_status("<!doctype html>"), None);
        // An error object says nothing about the sign-in.
        assert_eq!(
            parse_api_status(r#"{"error":"Internal Server Error"}"#),
            None
        );
        Ok(())
    }

    #[test]
    fn only_a_missing_sign_in_asks_to_sign_in_again() -> eyre::Result<()> {
        let anonymous = fixture_text("api_status_anonymous.json")?;
        let authenticated = fixture_text("api_status_authenticated.json")?;
        let error_object = r#"{"timestamp":1,"status":500,"error":"Internal Server Error"}"#;
        let refused = |status: u16, body: &str| {
            signed_in(status, body).is_err_and(|e| http::is_login_rejected(&e))
        };
        let outage = |status: u16, body: &str| {
            signed_in(status, body)
                .is_err_and(|e| http::is_transient(&e) && !http::is_login_rejected(&e))
        };
        let unexpected = |status: u16, body: &str| {
            signed_in(status, body)
                .is_err_and(|e| !http::is_transient(&e) && !http::is_login_rejected(&e))
        };

        assert!(signed_in(200, &authenticated).is_ok());
        // Without a sign-in the API answers 401 and `authenticated: false`.
        assert!(refused(401, &anonymous));
        assert!(refused(200, &anonymous));
        assert!(refused(401, ""));

        assert!(outage(500, error_object));
        assert!(outage(503, "<html>Service Unavailable</html>"));
        assert!(outage(429, ""));
        // An outage that also reports no sign-in is still an outage.
        assert!(outage(502, &anonymous));

        // A bot block, and JSON that is not the status.
        assert!(unexpected(403, "<html>Access Denied</html>"));
        assert!(unexpected(403, &anonymous));
        assert!(unexpected(200, "{}"));
        assert!(unexpected(200, "<!doctype html>"));
        assert!(unexpected(404, error_object));
        let message = signed_in(403, "<html>Access Denied</html>")
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(
            message.contains("403") && message.contains("Access Denied"),
            "{message}"
        );
        assert!(
            !message.to_lowercase().contains("sign in again"),
            "{message}"
        );
        Ok(())
    }

    #[test]
    fn a_refusal_after_a_failed_replay_reports_the_replay() {
        let replay = |status: u16| {
            Some(eyre::Report::new(http::HttpStatus {
                status,
                url: SIGN_IN_URL.to_string(),
                body: String::new(),
            }))
        };
        // The OAuth service was down or blocked BoilR: no session, whatever the cookies.
        let outage = after_replay(replay(503), Err(sign_in_refused()));
        assert!(outage.is_err_and(|e| http::is_transient(&e) && !http::is_login_rejected(&e)));
        let blocked = after_replay(replay(403), Err(sign_in_refused()));
        assert!(blocked.is_err_and(|e| !http::is_transient(&e) && !http::is_login_rejected(&e)));

        // A replay that went through leaves the check as it is.
        let refused = after_replay(None, Err(sign_in_refused()));
        assert!(refused.is_err_and(|e| http::is_login_rejected(&e)));
        assert!(after_replay(None, Ok(())).is_ok());
        // And a sign-in that is there is fine, whatever the replay's last page answered.
        assert!(after_replay(replay(404), Ok(())).is_ok());
        let other = after_replay(replay(404), Err(http::transient("timed out")));
        assert!(other.is_err_and(|e| http::is_transient(&e)));
    }

    #[test]
    fn maps_game_accounts_and_classic_keys_to_products() -> eyre::Result<()> {
        let games: GamesAndSubs = fixture("games_and_subs.json")?;
        let classic: ClassicGames = fixture("classic_games.json")?;
        let owned = owned_games(&games, &classic)?;
        // Two WoW accounts are one game; the game the table lacks is listed too; two
        // Warcraft III keys are the game and its expansion.
        assert_eq!(
            ids(&owned),
            vec![
                "WoW",
                "Pro",
                "WTCG",
                "Fen",
                "title-987654321",
                "W3C",
                "W3CX",
                "D2"
            ]
        );

        let overwatch = owned.get(1).ok_or_else(|| eyre::format_err!("missing"))?;
        assert_eq!(overwatch.store, StoreId::BattleNet);
        assert_eq!(overwatch.title, "Overwatch 2");
        assert_eq!(overwatch.extra("internal_id"), Some("prometheus"));
        assert_eq!(overwatch.extra.len(), 1);

        // Without an internal id, a shortcut opens the download page.
        let war3 = owned.get(5).ok_or_else(|| eyre::format_err!("missing"))?;
        assert_eq!(war3.title, "Warcraft III: Reign of Chaos");
        assert!(war3.extra.is_empty());
        Ok(())
    }

    #[test]
    fn a_game_the_table_lacks_is_listed_under_the_account_sites_name() -> eyre::Result<()> {
        let games: GamesAndSubs = fixture("games_and_subs.json")?;
        let owned = owned_games(&games, &ClassicGames::default())?;
        let unknown = owned.get(4).ok_or_else(|| eyre::format_err!("missing"))?;
        // 987654321 spells no code.
        assert_eq!(unknown.id, "title-987654321");
        assert_eq!(unknown.title, "A Game BoilR Does Not Know");
        assert!(unknown.extra.is_empty());

        // 1094861636 is "ABCD": the code a new game most likely has in the launcher.
        let new = accounts(
            r#"{"gameAccounts":[
                {"titleId":1094861636,"localizedGameName":"  Brand New® Game™ "},
                {"titleId":1094861636,"localizedGameName":"Brand New® Game™"},
                {"titleId":1094861637,"localizedGameName":null},
                {"titleId":1094861638,"localizedGameName":" ® "},
                {"titleId":1094861639}
            ]}"#,
        )?;
        // The second account of the same game adds nothing; without a name there is
        // nothing to list a game under.
        assert_eq!(ids(&new), vec!["ABCD"]);
        assert_eq!(
            new.first().map(|g| g.title.as_str()),
            Some("Brand New Game")
        );

        // A code the table knows is that product, also under a title id it does not list:
        // 1347310657 is "PNTA".
        let by_code = accounts(r#"{"gameAccounts":[{"titleId":1347310657}]}"#)?;
        assert_eq!(ids(&by_code), vec!["PNTA"]);
        assert_eq!(
            by_code.first().and_then(|g| g.extra("internal_id")),
            Some("pinta")
        );
        // But never a classic game's: 17458 is "D2", which here is some other game.
        let not_classic = accounts(
            r#"{"gameAccounts":[{"titleId":17458,"localizedGameName":"Something Else"}]}"#,
        )?;
        assert_eq!(ids(&not_classic), vec!["title-17458"]);
        Ok(())
    }

    #[test]
    fn title_ids_spell_product_codes() {
        assert_eq!(code_of_title(5272175).as_deref(), Some("Pro"));
        assert_eq!(code_of_title(1096108883).as_deref(), Some("AUKS"));
        assert_eq!(code_of_title(17459).as_deref(), Some("D3"));
        assert_eq!(code_of_title(987654321), None);
        assert_eq!(code_of_title(0), None);
        // "A B" and "A,B": only letters and digits make a code.
        assert_eq!(code_of_title(0x41_20_42), None);
        assert_eq!(code_of_title(0x41_2C_42), None);
        assert_eq!(code_of_title(u64::MAX), None);
    }

    #[test]
    fn the_account_sites_name_chooses_among_products_that_share_a_title_id() -> eyre::Result<()> {
        let code = |name: Option<&str>| product_for_title(1096108883, name).map(|p| p.code);
        assert_eq!(
            code(Some("Call of Duty®: Modern Warfare® III")),
            Some("PNTA")
        );
        assert_eq!(
            code(Some("Call of Duty®: Modern Warfare® II")),
            Some("AUKS")
        );
        assert_eq!(
            code(Some("CALL OF DUTY: MODERN WARFARE III - Vault Edition")),
            Some("PNTA")
        );
        // A name that does not help: the first one, as Playnite does.
        assert_eq!(code(Some("Call of Duty®")), Some("AUKS"));
        assert_eq!(code(Some("")), Some("AUKS"));
        assert_eq!(code(None), Some("AUKS"));
        // Nothing to choose where a title id has one product.
        assert_eq!(
            product_for_title(5272175, Some("Something else")).map(|p| p.code),
            Some("Pro")
        );
        assert_eq!(product_for_title(987654321, Some("Overwatch 2")), None);

        // An account with only Modern Warfare III gets that game, one with both gets both.
        let mw3 = accounts(
            r#"{"gameAccounts":[
                {"titleId":1096108883,"localizedGameName":"Call of Duty®: Modern Warfare® III"}
            ]}"#,
        )?;
        assert_eq!(ids(&mw3), vec!["PNTA"]);
        assert_eq!(
            mw3.first().map(|g| g.title.as_str()),
            Some("Call of Duty: Modern Warfare III")
        );
        let both = accounts(
            r#"{"gameAccounts":[
                {"titleId":1096108883,"localizedGameName":"Call of Duty®: Modern Warfare® III"},
                {"titleId":1096108883,"localizedGameName":"Call of Duty®: Modern Warfare® II"}
            ]}"#,
        )?;
        assert_eq!(ids(&both), vec!["PNTA", "AUKS"]);
        Ok(())
    }

    #[test]
    fn a_failed_classic_list_keeps_the_classic_games_from_before() -> eyre::Result<()> {
        let games: GamesAndSubs = fixture("games_and_subs.json")?;
        let classic: ClassicGames = fixture("classic_games.json")?;
        let mut before = owned_games(&games, &classic)?;
        // A list saved by an earlier version marks classic games with an extra.
        for game in &mut before {
            if game.id == "D2" {
                game.extra.insert("launcher".into(), "false".into());
            }
        }
        let now =
            with_previous_classic_games(owned_games(&games, &ClassicGames::default())?, &before);
        assert_eq!(
            ids(&now),
            vec![
                "WoW",
                "Pro",
                "WTCG",
                "Fen",
                "title-987654321",
                "W3C",
                "W3CX",
                "D2"
            ]
        );
        // Classic games come back as this version lists them.
        assert!(now.iter().skip(5).all(|g| g.extra.is_empty()));
        Ok(())
    }

    #[test]
    fn empty_lists_give_no_games() -> eyre::Result<()> {
        assert!(accounts(r#"{"gameAccounts":[]}"#)?.is_empty());
        let null_accounts: GamesAndSubs = serde_json::from_str(r#"{"gameAccounts":null}"#)?;
        let null_classic: ClassicGames = serde_json::from_str(r#"{"classicGames":null}"#)?;
        assert!(owned_games(&null_accounts, &null_classic)?.is_empty());
        let no_classic: ClassicGames = serde_json::from_str("{}")?;
        assert!(owned_games(&null_accounts, &no_classic)?.is_empty());
        Ok(())
    }

    #[test]
    fn an_answer_without_the_game_list_is_not_an_empty_library() {
        // Another shape, or an error object: not "no games".
        for body in [
            "{}",
            r#"{"games":[{"titleId":5272175}]}"#,
            r#"{"timestamp":1,"status":500,"error":"Internal Server Error"}"#,
            r#"{"gameAccounts":{"titleId":5272175}}"#,
        ] {
            let parsed = serde_json::from_str::<GamesAndSubs>(body);
            assert!(parsed.is_err(), "{body}");
        }
        // Entries that carry nothing BoilR knows them by.
        let renamed = accounts(r#"{"gameAccounts":[{"programId":5272175},{"programId":17459}]}"#);
        let message = renamed.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(message.contains("Unexpected answer"), "{message}");
        // One entry without a title id among good ones is passed over.
        let mixed = accounts(r#"{"gameAccounts":[{"titleId":5272175},{"gameAccountName":"x"}]}"#);
        assert!(mixed.is_ok_and(|games| ids(&games) == vec!["Pro"]));
    }

    #[test]
    fn reads_battletag() -> eyre::Result<()> {
        let details: AccountDetails = fixture("details.json")?;
        assert_eq!(details.battle_tag.as_deref(), Some("Player#2187"));
        Ok(())
    }

    #[test]
    fn franchise_icons_do_not_overlap() {
        assert!(is_franchise("diablo-ii.png", "diablo-ii"));
        assert!(is_franchise("D2-Diablo-II-LoD.png", "diablo-ii"));
        assert!(!is_franchise("diablo-iii.png", "diablo-ii"));
        assert!(is_franchise(
            "warcraft-iii-frozen-throne.png",
            "warcraft-iii"
        ));
    }

    #[test]
    fn product_table_is_consistent() {
        for (i, p) in PRODUCTS.iter().enumerate() {
            assert!(
                !PRODUCTS.iter().skip(i + 1).any(|q| q.code == p.code),
                "duplicate code {}",
                p.code
            );
            // The launcher knows a game, by its internal id, exactly when it is not classic.
            assert_eq!(p.internal_id.is_none(), p.is_classic(), "{}", p.code);
        }
        assert_eq!(
            product("pro").and_then(|p| p.internal_id),
            Some("prometheus")
        );
        assert!(product("W3C").is_some_and(|p| p.is_classic()));
        // Title ids spell the product code, but for a few games whose account entry goes by
        // another name (Diablo is "DRTL") and for Modern Warfare III.
        let spelled_otherwise: Vec<&str> = PRODUCTS
            .iter()
            .filter(|p| {
                p.title_id
                    .is_some_and(|title| code_of_title(title).as_deref() != Some(p.code))
            })
            .map(|p| p.code)
            .collect();
        assert_eq!(spelled_otherwise, vec!["D1", "W1", "W2", "PNTA"]);
    }
}
