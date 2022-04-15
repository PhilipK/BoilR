mod egs;
mod gog;
mod heroic;
mod itch;
mod legendary;
mod lutris;
mod origin;
mod platform;
mod settings;
mod steam;
mod steamgriddb;
mod sync;
mod uplay;
mod ui;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args();
    if args.len() > 1 && args.nth(1).unwrap_or_default() == "--no-ui" {
        ui::run_sync();
        Ok(())
    }else{
        ui::run_ui()
    }
}