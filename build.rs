use std::io;
#[cfg(windows)]
use winres::WindowsResource;

fn main() -> io::Result<()> {
    // Only for the egui `boilr.exe`: the resource is linked into everything that depends on this
    // crate, and the Tauri app embeds its own, so two would clash (CVT1100 duplicate resource).
    #[cfg(windows)]
    if std::env::var_os("CARGO_FEATURE_EGUI_UI").is_some() {
        WindowsResource::new()
            .set_icon("resources/logo.ico")
            .compile()?;
    }
    Ok(())
}
