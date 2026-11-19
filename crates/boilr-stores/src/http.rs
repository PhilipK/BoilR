//! Shared HTTP helpers for the store clients.

use std::future::Future;

use reqwest::{Client, ClientBuilder, RequestBuilder, Response};
use serde::de::DeserializeOwned;

use crate::Cookie;

/// A browser-like user agent. Several store web endpoints reject unknown clients; store
/// modules override it per request where a launcher's own user agent is expected. The login
/// windows of the cookie-session stores use it too, so a session is replayed under the user
/// agent it was made with.
pub const BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

/// Every request ends after this long.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

fn builder() -> ClientBuilder {
    Client::builder()
        .user_agent(BROWSER_USER_AGENT)
        .timeout(REQUEST_TIMEOUT)
}

pub fn client() -> eyre::Result<Client> {
    Ok(builder().cookie_store(true).build()?)
}

/// A `Cookie` header value for the cookies whose domain matches `host` (a cookie domain of
/// `.example.com` matches `example.com` and every subdomain).
pub fn cookie_header(cookies: &[Cookie], host: &str) -> String {
    cookies
        .iter()
        .filter(|c| domain_matches(&c.domain, host))
        .map(|c| format!("{}={}", c.name, c.value))
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn domain_matches(cookie_domain: &str, host: &str) -> bool {
    let domain = cookie_domain.trim_start_matches('.').to_ascii_lowercase();
    let host = host.to_ascii_lowercase();
    host == domain || host.ends_with(&format!(".{domain}"))
}

/// The store refused the saved login: HTTP 401, an OAuth `invalid_grant`, a sign-in page
/// where data was expected, or a session the store reports as signed out. Only this kind of
/// failure asks the user to sign in again. Store modules check for it with
/// [`is_login_rejected`] to renew a token once and try again.
#[derive(Debug)]
pub struct LoginRejected(pub String);

impl std::fmt::Display for LoginRejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LoginRejected {}

pub fn login_rejected(message: impl Into<String>) -> eyre::Report {
    eyre::Report::new(LoginRejected(message.into()))
}

pub fn is_login_rejected(err: &eyre::Report) -> bool {
    err.chain().any(|cause| {
        cause.is::<LoginRejected>()
            || cause
                .downcast_ref::<HttpStatus>()
                .is_some_and(|http| http.status == 401)
    })
}

/// The store or the network is having trouble: no connection, a timeout, HTTP 429 or 5xx.
/// The saved login is probably fine and trying later may work.
#[derive(Debug)]
pub struct Transient(pub String);

impl std::fmt::Display for Transient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Transient {}

pub fn transient(message: impl Into<String>) -> eyre::Report {
    eyre::Report::new(Transient(message.into()))
}

pub fn is_transient(err: &eyre::Report) -> bool {
    err.chain().any(|cause| {
        cause.is::<Transient>()
            || cause
                .downcast_ref::<HttpStatus>()
                .is_some_and(|http| is_transient_status(http.status))
            || cause
                .downcast_ref::<reqwest::Error>()
                .is_some_and(is_transient_request_error)
    })
}

/// Statuses that say "not now" rather than "no".
pub fn is_transient_status(status: u16) -> bool {
    matches!(status, 408 | 425 | 429) || (500..600).contains(&status)
}

fn is_transient_request_error(err: &reqwest::Error) -> bool {
    !(err.is_builder() || err.is_redirect() || err.is_status())
}

/// A response with a non-success status. 401 counts as a rejected login
/// ([`is_login_rejected`]); 408, 425, 429 and 5xx count as transient ([`is_transient`]).
/// 403 is neither: stores answer it for bot blocks and region locks as well as for dead
/// tokens, so on its own it does not ask the user to sign in again.
#[derive(Debug)]
pub struct HttpStatus {
    pub status: u16,
    /// The URL without its query.
    pub url: String,
    /// The start of the response body, with secrets taken out.
    pub body: String,
}

impl std::fmt::Display for HttpStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = reqwest::StatusCode::from_u16(self.status)
            .map(|s| s.to_string())
            .unwrap_or_else(|_| self.status.to_string());
        if self.status == 401 {
            write!(
                f,
                "The store rejected the saved login ({status} from {}). Sign in again. {}",
                self.url, self.body
            )
        } else {
            write!(f, "{status} from {}: {}", self.url, self.body)
        }
    }
}

impl std::error::Error for HttpStatus {}

/// The HTTP status of the response that caused `err`, when a status did.
pub fn status_of(err: &eyre::Report) -> Option<u16> {
    http_status(err).map(|http| http.status)
}

/// The failed response behind `err`, when a status caused it.
pub fn http_status(err: &eyre::Report) -> Option<&HttpStatus> {
    err.chain()
        .find_map(|cause| cause.downcast_ref::<HttpStatus>())
}

/// Sends a request and decodes a JSON body, turning HTTP errors into readable messages that
/// include the start of the response body.
pub async fn send_json<T: DeserializeOwned>(request: RequestBuilder) -> eyre::Result<T> {
    read_json(send(request).await?).await
}

/// Decodes the JSON body of a response already received.
pub async fn read_json<T: DeserializeOwned>(response: Response) -> eyre::Result<T> {
    let url = without_query(response.url());
    let text = response.text().await.map_err(request_failed)?;
    serde_json::from_str(&text).map_err(|err| {
        eyre::format_err!(
            "Unexpected response from {url}: {err}. Body starts with: {}",
            snippet(&text)
        )
    })
}

/// Sends a request and fails on a non-success status, with an [`HttpStatus`] error.
pub async fn send(request: RequestBuilder) -> eyre::Result<Response> {
    let response = request.send().await.map_err(request_failed)?;
    if response.status().is_success() {
        return Ok(response);
    }
    Err(status_error(response).await)
}

/// The [`HttpStatus`] error for a response, reading its body for the message.
pub async fn status_error(response: Response) -> eyre::Report {
    let status = response.status().as_u16();
    let url = without_query(response.url());
    let body = response.text().await.unwrap_or_default();
    eyre::Report::new(HttpStatus {
        status,
        url,
        body: snippet(&body),
    })
}

/// A reqwest error as a message without the request's query string, which can hold tokens.
/// Connection problems and timeouts come out as [`Transient`].
pub fn request_failed(err: reqwest::Error) -> eyre::Report {
    let is_transient = is_transient_request_error(&err);
    let at = err.url().map(without_query);
    let err = err.without_url();
    let mut message = err.to_string();
    let mut cause = std::error::Error::source(&err);
    while let Some(inner) = cause {
        message.push_str(": ");
        message.push_str(&inner.to_string());
        cause = inner.source();
    }
    if let Some(url) = at {
        message = format!("{message} ({url})");
    }
    if is_transient {
        transient(message)
    } else {
        eyre::Report::msg(message)
    }
}

/// The URL without query, fragment and user info: what is safe to show in an error.
pub fn without_query(url: &reqwest::Url) -> String {
    let mut url = url.clone();
    url.set_query(None);
    url.set_fragment(None);
    let _ = url.set_password(None);
    let _ = url.set_username("");
    url.to_string()
}

/// How much of a body is looked at for a snippet: enough to redact around the part shown.
const SNIPPET_SOURCE_CHARS: usize = 2000;

/// The start of a response body for an error message, with secrets taken out.
pub fn snippet(text: &str) -> String {
    let start: String = text.chars().take(SNIPPET_SOURCE_CHARS).collect();
    redact(&start).chars().take(300).collect()
}

/// Fetches a list page by page. `fetch` gets a cursor and returns that page's items and the
/// next cursor, or `None` after the last page.
///
/// A cursor that comes back a second time, or more than `max_pages` pages, is an error and
/// not the end of the list: a list cut short must never be saved as the whole library,
/// because the games it lacks would be removed from Steam.
pub async fn paginate<C, T, F, Fut>(
    what: &str,
    first: C,
    max_pages: usize,
    mut fetch: F,
) -> eyre::Result<Vec<T>>
where
    C: Clone + PartialEq,
    F: FnMut(C) -> Fut,
    Fut: Future<Output = eyre::Result<(Vec<T>, Option<C>)>>,
{
    let mut items = Vec::new();
    let mut seen = vec![first.clone()];
    let mut cursor = first;
    for _ in 0..max_pages {
        let (page, next) = fetch(cursor).await?;
        items.extend(page);
        let Some(next) = next else {
            return Ok(items);
        };
        if seen.contains(&next) {
            return Err(eyre::format_err!(
                "{what} sent the same page twice, so the list is incomplete and was not saved"
            ));
        }
        seen.push(next.clone());
        cursor = next;
    }
    Err(eyre::format_err!(
        "{what} kept sending more pages after {max_pages}, so the list is incomplete and was not saved"
    ))
}

/// Removes what may be a secret from text that ends up in errors and logs: the query string
/// and fragment of every URL (tokens, client secrets, Humble order keys), and the values of
/// JSON fields whose names suggest a credential.
pub fn redact(text: &str) -> String {
    redact_json_secrets(&redact_urls(text))
}

fn redact_urls(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = ["https://", "http://"]
        .iter()
        .filter_map(|scheme| rest.find(scheme))
        .min()
    {
        let (before, from_url) = rest.split_at(start);
        out.push_str(before);
        let end = from_url
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | ')' | ']' | '}' | '\\')
            })
            .unwrap_or(from_url.len());
        let (url, after) = from_url.split_at(end);
        let kept = url.find(['?', '#']).unwrap_or(url.len());
        out.push_str(url.get(..kept).unwrap_or(url));
        rest = after;
    }
    out.push_str(rest);
    out
}

const SECRET_FIELD_PARTS: &[&str] = &[
    "token", "secret", "password", "cookie", "code", "key", "session", "auth", "ticket", "sid",
];

fn is_secret_field(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    !name.starts_with("error") && SECRET_FIELD_PARTS.iter().any(|part| name.contains(part))
}

/// Replaces the string values of secret-looking JSON fields with `"…"`. Works on cut-off or
/// otherwise broken JSON too.
fn redact_json_secrets(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        if c != '"' {
            out.push(c);
            i += 1;
            continue;
        }
        let name_end = string_end(&chars, i);
        let name: String = chars
            .get(i + 1..name_end.saturating_sub(1).max(i + 1))
            .unwrap_or_default()
            .iter()
            .collect();
        out.extend(chars.get(i..name_end).unwrap_or_default());
        i = name_end;
        let colon = skip_whitespace(&chars, i);
        if chars.get(colon) != Some(&':') || !is_secret_field(&name) {
            continue;
        }
        let value = skip_whitespace(&chars, colon + 1);
        if chars.get(value) != Some(&'"') {
            continue;
        }
        out.extend(chars.get(i..value).unwrap_or_default());
        out.push_str("\"…\"");
        i = string_end(&chars, value);
    }
    out
}

/// The index after the closing quote of the string that opens at `open`.
fn string_end(chars: &[char], open: usize) -> usize {
    let mut i = open + 1;
    while let Some(&c) = chars.get(i) {
        match c {
            '\\' => i += 2,
            '"' => return i + 1,
            _ => i += 1,
        }
    }
    chars.len()
}

fn skip_whitespace(chars: &[char], mut i: usize) -> usize {
    while chars.get(i).is_some_and(|c| c.is_whitespace()) {
        i += 1;
    }
    i
}

/// Reads a query parameter from a URL.
pub fn query_param(url: &str, name: &str) -> Option<String> {
    let url = url::Url::parse(url).ok()?;
    url.query_pairs()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
}

/// A client whose cookie jar is seeded from a saved session, so cookies follow redirects and
/// cookies a store renews can be read back into the session. Used by the cookie-session
/// stores (EA, Battle.net).
pub mod cookie_jar {
    use std::sync::Arc;

    use reqwest::{cookie::Jar, Client};

    use crate::{http, Cookie};

    /// The jar of a client made by [`client_with`], and the cookies it started with.
    pub(crate) struct SessionJar {
        jar: Arc<Jar>,
        seeded: Vec<Cookie>,
    }

    impl std::ops::Deref for SessionJar {
        type Target = Jar;

        fn deref(&self) -> &Jar {
            &self.jar
        }
    }

    pub(crate) fn client_with(cookies: &[Cookie]) -> eyre::Result<(Client, SessionJar)> {
        let jar = Arc::new(Jar::default());
        for cookie in cookies {
            add(&jar, cookie);
        }
        let client = http::builder().cookie_provider(jar.clone()).build()?;
        let seeded = cookies.to_vec();
        Ok((client, SessionJar { jar, seeded }))
    }

    /// Adds a saved cookie for its domain and every subdomain.
    ///
    /// A login window hands cookies over without saying whether they are host-only: the
    /// `cookie` crate drops a domain's leading dot, so `.battle.net` and a host-only
    /// `battle.net` cookie look the same. Treating each one as a domain cookie sends it to
    /// the hosts the browser would have sent it to (and, for a cookie that was host-only, to
    /// that host's subdomains as well, which stores do not mind). A cookie the store renews
    /// for the same domain and path replaces the saved one.
    fn add(jar: &Jar, cookie: &Cookie) {
        let host = cookie.domain.trim_start_matches('.');
        if host.is_empty() || cookie.name.is_empty() {
            return;
        }
        let Ok(url) = url::Url::parse(&format!("https://{host}/")) else {
            return;
        };
        jar.add_cookie_str(
            &format!("{}={}; Domain={host}; Path=/", cookie.name, cookie.value),
            &url,
        );
    }

    /// `at` as a URL: a full URL as it is, a bare host as `https://<host>/`.
    fn url_of(at: &str) -> Option<url::Url> {
        if at.contains("://") {
            url::Url::parse(at).ok()
        } else {
            url::Url::parse(&format!("https://{at}/")).ok()
        }
    }

    /// The `name=value` pairs the jar would send to `at` (a host or a URL).
    pub(crate) fn pairs_for(jar: &Jar, at: &str) -> Vec<(String, String)> {
        use reqwest::cookie::CookieStore;
        let Some(url) = url_of(at) else {
            return vec![];
        };
        let Some(header) = jar.cookies(&url) else {
            return vec![];
        };
        header
            .to_str()
            .unwrap_or_default()
            .split(';')
            .filter_map(|pair| {
                let (name, value) = pair.trim().split_once('=')?;
                Some((name.to_string(), value.to_string()))
            })
            .collect()
    }

    /// Copies renewed cookie values from the jar into `cookies`, and adds cookies the stores
    /// set since. `at` lists the hosts or URLs BoilR calls; give the URL a store renews its
    /// cookies on when they are scoped to a path (`https://accounts.ea.com/connect/auth`).
    ///
    /// A cookie renewed for a deeper path than `/` sits in the jar next to the seeded one.
    /// Of two values for one name, the one that is not the seeded value is the renewed one.
    pub(crate) fn refresh(cookies: &mut Vec<Cookie>, jar: &SessionJar, at: &[&str]) {
        for at in at {
            let Some(host) = url_of(at).and_then(|url| url.host_str().map(str::to_string)) else {
                continue;
            };
            let pairs = pairs_for(jar, at);
            let mut done: Vec<&str> = Vec::new();
            for (name, _) in &pairs {
                if done.contains(&name.as_str()) {
                    continue;
                }
                done.push(name);
                let seeded = jar
                    .seeded
                    .iter()
                    .find(|c| &c.name == name && http::domain_matches(&c.domain, &host))
                    .map(|c| c.value.as_str());
                let mut values = pairs.iter().filter(|(n, _)| n == name).map(|(_, v)| v);
                let Some(value) = values
                    .clone()
                    .find(|v| Some(v.as_str()) != seeded)
                    .or_else(|| values.next())
                else {
                    continue;
                };
                match cookies
                    .iter_mut()
                    .find(|c| &c.name == name && http::domain_matches(&c.domain, &host))
                {
                    Some(existing) => existing.value = value.clone(),
                    None => cookies.push(Cookie {
                        name: name.clone(),
                        value: value.clone(),
                        domain: host.clone(),
                    }),
                }
            }
        }
    }

    /// The cookies a browser would send to any of `hosts`, one per name and domain (the
    /// last wins). Cookies for other hosts of the same site (its shop, its forum, trackers)
    /// are not needed to list games, so they are not kept.
    pub(crate) fn keep_for_hosts(cookies: &[Cookie], hosts: &[&str]) -> Vec<Cookie> {
        let mut kept: Vec<Cookie> = Vec::new();
        for cookie in cookies {
            let domain = cookie.domain.trim_start_matches('.');
            if domain.is_empty()
                || !hosts
                    .iter()
                    .any(|host| http::domain_matches(&cookie.domain, host))
            {
                continue;
            }
            kept.retain(|c| !(c.name == cookie.name && c.domain == cookie.domain));
            kept.push(cookie.clone());
        }
        kept
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn cookie(name: &str, value: &str, domain: &str) -> Cookie {
            Cookie {
                name: name.into(),
                value: value.into(),
                domain: domain.into(),
            }
        }

        #[test]
        fn cookies_reach_subdomains_whatever_the_capture_calls_their_domain() -> eyre::Result<()> {
            // A login window reports `.battle.net` as `battle.net` (no leading dot).
            let captured = vec![
                cookie("login", "1", "battle.net"),
                cookie("host", "2", "oauth.battle.net"),
                cookie("dotted", "3", ".battle.net"),
            ];
            let (_client, jar) = client_with(&captured)?;
            let mut names: Vec<String> = pairs_for(&jar, "oauth.battle.net")
                .into_iter()
                .map(|(name, _)| name)
                .collect();
            names.sort();
            assert_eq!(names, vec!["dotted", "host", "login"]);
            let other: Vec<String> = pairs_for(&jar, "account.battle.net")
                .into_iter()
                .map(|(name, _)| name)
                .collect();
            assert!(other.contains(&"login".to_string()));
            assert!(!other.contains(&"host".to_string()));
            assert!(pairs_for(&jar, "example.com").is_empty());
            Ok(())
        }

        #[test]
        fn renewed_cookies_are_read_back_whatever_their_path() -> eyre::Result<()> {
            let mut saved = vec![
                cookie("sid", "old", "accounts.ea.com"),
                cookie("remid", "keep", "accounts.ea.com"),
            ];
            let (_client, jar) = client_with(&saved)?;
            let auth = url::Url::parse("https://accounts.ea.com/connect/auth")?;
            // The store renews `sid` without a path, so it is scoped to `/connect`.
            jar.add_cookie_str("sid=new", &auth);
            jar.add_cookie_str("fresh=1; Path=/", &auth);

            let at = ["https://accounts.ea.com/connect/auth"];
            refresh(&mut saved, &jar, &at);
            // Twice gives the same answer.
            refresh(&mut saved, &jar, &at);

            let value = |name: &str| {
                saved
                    .iter()
                    .find(|c| c.name == name)
                    .map(|c| c.value.clone())
            };
            assert_eq!(value("sid").as_deref(), Some("new"));
            assert_eq!(value("remid").as_deref(), Some("keep"));
            assert_eq!(value("fresh").as_deref(), Some("1"));
            assert_eq!(saved.len(), 3);
            Ok(())
        }

        #[test]
        fn only_cookies_for_the_hosts_in_use_are_kept() {
            let captured = vec![
                cookie("sid", "1", "accounts.ea.com"),
                cookie("site", "2", "ea.com"),
                cookie("shop", "3", "www.ea.com"),
                cookie("sid", "4", "accounts.ea.com"),
                cookie("other", "5", "example.com"),
            ];
            let kept = keep_for_hosts(&captured, &["accounts.ea.com"]);
            let names: Vec<(&str, &str)> = kept
                .iter()
                .map(|c| (c.name.as_str(), c.value.as_str()))
                .collect();
            assert_eq!(names, vec![("site", "2"), ("sid", "4")]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_header_keeps_matching_domains() {
        let cookies = vec![
            Cookie {
                name: "a".into(),
                value: "1".into(),
                domain: ".humblebundle.com".into(),
            },
            Cookie {
                name: "b".into(),
                value: "2".into(),
                domain: "other.com".into(),
            },
        ];
        assert_eq!(cookie_header(&cookies, "www.humblebundle.com"), "a=1");
        assert_eq!(cookie_header(&cookies, "humblebundle.com"), "a=1");
        assert_eq!(cookie_header(&cookies, "nothumblebundle.com"), "");
    }

    #[test]
    fn redacts_secrets_in_urls_and_json() {
        let message = r#"400 Bad Request from https://auth.gog.com/token?client_id=1&refresh_token=SECRET: {"error":"invalid_grant","refresh_token": "SECRET2","access_token":"SEC\"RET3","count":2}"#;
        let redacted = redact(message);
        assert!(!redacted.contains("SECRET"), "{redacted}");
        assert!(!redacted.contains("RET3"), "{redacted}");
        assert!(
            redacted.contains("https://auth.gog.com/token "),
            "{redacted}"
        );
        assert!(
            redacted.contains(r#""error":"invalid_grant""#),
            "{redacted}"
        );
        assert!(redacted.contains(r#""count":2"#), "{redacted}");
        // Cut-off JSON, as in a snippet.
        assert_eq!(redact(r#"{"gamekey":"abc"#), r#"{"gamekey":"…""#);
        assert_eq!(
            redact("see (https://x.com/a?k=1#f) ok"),
            "see (https://x.com/a) ok"
        );
        let url =
            reqwest::Url::parse("https://u:p@humblebundle.com/api/v1/order/K?all_tpkds=true#x")
                .map(|u| without_query(&u))
                .unwrap_or_default();
        assert_eq!(url, "https://humblebundle.com/api/v1/order/K");
    }

    #[test]
    fn reads_query_parameters() {
        assert_eq!(
            query_param(
                "https://embed.gog.com/on_login_success?origin=client&code=XYZ",
                "code"
            ),
            Some("XYZ".to_string())
        );
    }

    fn status(status: u16) -> eyre::Report {
        eyre::Report::new(HttpStatus {
            status,
            url: "https://store.example/api".into(),
            body: String::new(),
        })
    }

    #[test]
    fn only_a_refused_login_asks_to_sign_in_again() {
        use eyre::WrapErr;
        assert!(is_login_rejected(&status(401)));
        assert!(is_login_rejected(&login_rejected("signed out")));
        // Wrapping keeps the kind.
        let wrapped = Err::<(), _>(status(401)).wrap_err("Could not load the library");
        assert!(wrapped.is_err_and(|e| is_login_rejected(&e) && status_of(&e) == Some(401)));
        // A bot block, an outage and a rate limit say nothing about the login.
        for other in [403, 429, 500, 502, 503] {
            assert!(!is_login_rejected(&status(other)), "{other}");
        }
        assert!(!is_login_rejected(&transient("no connection")));
        assert!(!is_login_rejected(&eyre::format_err!("sign in again")));
    }

    #[test]
    fn outages_and_rate_limits_are_transient() {
        for code in [408, 429, 500, 502, 503, 504] {
            assert!(is_transient(&status(code)), "{code}");
        }
        for code in [400, 401, 403, 404] {
            assert!(!is_transient(&status(code)), "{code}");
        }
        assert!(is_transient(&transient("timed out")));
        assert!(!is_transient(&eyre::format_err!("unexpected answer")));
    }

    #[test]
    fn snippets_are_short_and_redacted() {
        let body = format!(
            r#"{{"access_token":"SECRET","filler":"{}"}}"#,
            "x".repeat(5000)
        );
        let shown = snippet(&body);
        assert!(!shown.contains("SECRET"), "{shown}");
        assert_eq!(shown.chars().count(), 300);
    }

    fn run<T>(future: impl Future<Output = T>) -> eyre::Result<T> {
        Ok(tokio::runtime::Builder::new_current_thread()
            .build()?
            .block_on(future))
    }

    /// Pages of a list: page `n` holds `[n]` and is followed by `next(n)`.
    async fn pages(max_pages: usize, next: impl Fn(u32) -> Option<u32>) -> eyre::Result<Vec<u32>> {
        let next = &next;
        paginate("The test store", 1u32, max_pages, |page| async move {
            Ok((vec![page], next(page)))
        })
        .await
    }

    #[test]
    fn paging_ends_on_the_last_page() -> eyre::Result<()> {
        let all = run(pages(10, |page| (page < 3).then_some(page + 1)))??;
        assert_eq!(all, vec![1, 2, 3]);
        Ok(())
    }

    #[test]
    fn a_list_cut_short_is_an_error_not_a_shorter_list() -> eyre::Result<()> {
        // More pages than allowed.
        let capped = run(pages(3, |page| Some(page + 1)))?;
        assert!(capped.is_err_and(|e| e.to_string().contains("more pages after 3")));
        // The store keeps answering with the same cursor.
        let repeated = run(pages(10, |page| Some(page.min(2))))?;
        assert!(repeated.is_err_and(|e| e.to_string().contains("same page twice")));
        // A failing page fails the list.
        let failed = run(paginate("The test store", 1u32, 10, |page| async move {
            if page == 2 {
                Err(status(503))
            } else {
                Ok((vec![page], Some(page + 1)))
            }
        }))?;
        assert!(failed.is_err_and(|e: eyre::Report| is_transient(&e)));
        Ok(())
    }
}
