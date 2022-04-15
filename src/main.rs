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

#[cfg(feature = "ui")]
mod ui;

use std::error::Error;

#[cfg(not(feature = "ui"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    
    let settings = settings::Settings::new()?;
    if settings.steam.stop_steam{
        crate::steam::ensure_steam_stopped();
    }
    settings::Settings::write_config_if_missing();
    let usersinfo = sync::run_sync(&settings,&mut None).unwrap();
    sync::download_images(&settings,&usersinfo,&mut None).await;
    if settings.steam.start_steam{
        crate::steam::ensure_steam_started(&settings.steam);
    }
    Ok(())
}

#[cfg(feature = "ui")]
fn main() -> Result<(), Box<dyn Error>> {

    let mut args = std::env::args();
    if args.len() > 1 && args.nth(1).unwrap_or_default() == "--no-ui" {
        ui::run_sync();
        Ok(())
    }else{
        ui::run_ui()

    }
}
