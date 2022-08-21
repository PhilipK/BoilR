mod amazon;
mod config;
mod egs;
mod flatpak;
mod gog;
mod heroic;
mod itch;
mod legendary;
mod lutris;
mod migration;
mod origin;
mod platform;
mod settings;
mod steam;
mod steamgriddb;
mod sync;
mod ui;
mod uplay;

fn main(){
    ensure_config_folder();
    migration::migrate_config();

    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--no-ui".to_string()) {
        ui::run_sync();        
    } else {
        ui::run_ui(args);
    }
}

fn ensure_config_folder() {
    let path = config::get_config_folder();
    let _ = std::fs::create_dir_all(&path);
}
