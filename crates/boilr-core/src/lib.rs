//! UI-independent core of BoilR: settings, Steam file formats, SteamGridDB artwork and the
//! import pipeline. Game platforms and user interfaces live outside this crate and hand the
//! pipeline plain shortcut lists.
#![deny(clippy::unwrap_in_result)]
#![deny(clippy::get_unwrap)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]

pub mod config;
pub mod migration;
pub mod settings;
pub mod steam;
pub mod steamgriddb;
pub mod sync;
