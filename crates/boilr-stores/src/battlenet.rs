//! Battle.net: a placeholder until its client is added later in this series.

use reqwest::Client;

use crate::{Fetch, LoginCapture, ManualLogin, OwnedGame, PendingLogin, Session, StoreId};

pub fn begin_login() -> PendingLogin {
    PendingLogin {
        store: StoreId::BattleNet,
        start_url: String::new(),
        done_url_prefixes: vec![],
        page_text_url_prefixes: vec![],
        cookie_urls: vec![],
        manual: ManualLogin::Unsupported,
        user_agent: None,
        state: serde_json::Value::Null,
    }
}

pub async fn complete_login(
    _client: &Client,
    _pending: &PendingLogin,
    _capture: &LoginCapture,
) -> eyre::Result<Session> {
    Err(eyre::eyre!("Signing in to Battle.net is not supported yet"))
}

pub async fn fetch_owned(
    _client: &Client,
    _session: &mut Session,
    _fetch: &mut Fetch<'_>,
) -> eyre::Result<Vec<OwnedGame>> {
    Err(eyre::eyre!("Listing Battle.net games is not supported yet"))
}
