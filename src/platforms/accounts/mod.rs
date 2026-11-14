//! Store accounts as game platforms: every game a signed-in store account owns becomes a
//! shortcut, installed or not. The sign-in, sessions and owned-game lists live in the
//! `boilr-stores` crate; this module turns a cached library into shortcuts whose target
//! installs the game through the store's own launcher (or opens its store page).
//!
//! When a game is installed, the store's installed-games platform (Epic, GOG, Amazon,
//! Heroic, ...) reports it with the same identity (`boilr:<store>:<id>` in `DevkitGameID`).
//! [`drop_owned_duplicates`] then drops the owned entry, and the sync keeps the Steam app id
//! the game already had (see `boilr_core::sync::keep_identity_app_ids`).

mod arrange;
mod merge;
mod platform;
mod targets;

pub use arrange::{arrange_owned, arrange_owned_games, steam_titles, Arranged};
pub use boilr_stores::{self as stores, StoreId};
pub use merge::{drop_owned_duplicates, not_installed_collection, NOT_INSTALLED_COLLECTION};
pub use platform::{is_prerelease, AccountPlatform, AccountSettings, InstallWith};
pub use targets::describe_install_target;

/// Libraries older than this are fetched again by [`refresh_stale_libraries`].
pub const LIBRARY_MAX_AGE_SECS: u64 = 6 * 60 * 60;

/// Fetches the libraries that got old, of every account that is signed in and turned on as
/// a source: all stores at once, each for at most [`stores::REFRESH_DEADLINE`]. A store
/// that fails keeps its last list. Returns the stores whose list was fetched again.
///
/// This talks to the stores, so it runs in the background or before a sync without a
/// window; listing games only reads what was last fetched.
pub fn refresh_stale_libraries() -> Vec<StoreId> {
    let accounts: Vec<StoreId> = crate::platforms::get_platforms()
        .iter()
        .filter(|platform| platform.enabled())
        .filter_map(|platform| store_for_code_name(platform.code_name()))
        .collect();
    refresh_stale(&accounts)
}

/// [`refresh_stale_libraries`] for the account behind one source, when it is one; a no-op
/// for every other source.
pub fn refresh_stale_library(code_name: &str) {
    if let Some(store) = store_for_code_name(code_name) {
        refresh_stale(&[store]);
    }
}

fn refresh_stale(accounts: &[StoreId]) -> Vec<StoreId> {
    let stale: Vec<StoreId> = accounts
        .iter()
        .copied()
        .filter(|store| matches!(stores::load_session(*store), Ok(Some(_))))
        .filter(|store| {
            stores::is_stale(
                stores::cached_library(*store).as_ref(),
                LIBRARY_MAX_AGE_SECS,
            )
        })
        .collect();
    if stale.is_empty() {
        return vec![];
    }
    let fetched_before: Vec<Option<u64>> = stale.iter().map(|store| fetched_at(*store)).collect();
    let refreshed = block_on_fresh(futures::future::join_all(
        stale
            .iter()
            .map(|store| stores::refresh_if_stale(*store, LIBRARY_MAX_AGE_SECS)),
    ));
    if let Err(err) = refreshed {
        eprintln!("Could not refresh the store libraries: {err}");
    }
    stale
        .into_iter()
        .zip(fetched_before)
        .filter(|(store, before)| fetched_at(*store) != *before)
        .map(|(store, _)| store)
        .collect()
}

fn fetched_at(store: StoreId) -> Option<u64> {
    stores::cached_library(store).map(|library| library.fetched_at)
}

/// The settings section (and platform code name) of a store account. Never change these.
pub fn account_code_name(store: StoreId) -> &'static str {
    match store {
        StoreId::Epic => "epic_account",
        StoreId::Gog => "gog_account",
        StoreId::Amazon => "amazon_account",
        StoreId::Ea => "ea_account",
        StoreId::BattleNet => "battlenet_account",
        StoreId::Humble => "humble_account",
        StoreId::Itch => "itch_account",
    }
}

pub fn store_for_code_name(code_name: &str) -> Option<StoreId> {
    StoreId::ALL
        .into_iter()
        .find(|store| account_code_name(*store) == code_name)
}

/// Runs a future to completion on a fresh runtime in its own thread. Platforms are called
/// from synchronous code that may itself run inside a Tokio runtime (Tauri commands, the
/// egui app's blocking tasks), where `block_on` would panic.
pub fn block_on_fresh<F>(future: F) -> eyre::Result<F::Output>
where
    F: std::future::Future + Send,
    F::Output: Send,
{
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                Ok(runtime.block_on(future))
            })
            .join()
            .map_err(|_| eyre::format_err!("The store request thread stopped unexpectedly"))?
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_names_round_trip() {
        for store in StoreId::ALL {
            assert_eq!(store_for_code_name(account_code_name(store)), Some(store));
        }
        assert_eq!(store_for_code_name("epic_games"), None);
    }

    #[test]
    fn block_on_fresh_works_inside_a_runtime() -> eyre::Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        let value = runtime.block_on(async {
            tokio::task::spawn_blocking(|| block_on_fresh(async { 41 + 1 })).await
        })??;
        assert_eq!(value, 42);
        Ok(())
    }
}
