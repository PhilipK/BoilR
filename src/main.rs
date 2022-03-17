mod egs;
mod gog;
mod itch;
mod lutris;
mod legendary;
mod origin;
mod platform;
mod settings;
mod steam;
mod steamgriddb;
mod sync;
mod uplay;
mod heroic;

#[cfg(feature = "ui")]
mod ui;

use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "ui")]
    {
        ui::run_ui().await
    }
    #[cfg(not(feature = "ui"))]
    {
        let settings = settings::Settings::new()?;
        settings::Settings::write_config_if_missing();
        sync::run_sync(&settings).await
    }
}
