//! BoilR's launcher support (`platforms`), backups and renames, usable by any front end.
//! The egui interface lives in `ui`, behind the default `egui-ui` feature.
#![deny(clippy::unwrap_in_result)]
#![deny(clippy::get_unwrap)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]

pub mod backups;
pub mod platforms;
pub mod renames;
#[cfg(feature = "egui-ui")]
pub mod ui;

// Importing the core modules at the crate root keeps `crate::settings` etc. paths working.
#[allow(unused_imports)]
use boilr_core::{config, migration, settings, steam, steamgriddb, sync};
