use std::{path::Path, process::Command, thread::sleep, time::Duration};

use sysinfo::{ProcessExt, System, SystemExt};

pub fn ensure_steam_stopped() {
    #[cfg(target_os = "windows")]
    let steam_name = "steam.exe";
    #[cfg(target_os = "linux")]
    let steam_name = "steam";
    
    let s = System::new_all();
    let processes = s.processes_by_name(&steam_name);
    for process in processes {
        let mut s = System::new();
        process.kill();
        while s.refresh_process(process.pid()) {
            println!("Waiting for steam to stop");
            sleep(Duration::from_millis(500));
        }
    }
}

pub fn ensure_steam_started(settings: &super::SteamSettings) {
    #[cfg(target_os = "windows")]
    let steam_name = "steam.exe";
    #[cfg(target_os = "linux")]
    let steam_name = "steam";

    let s = System::new_all();
    let mut processes = s.processes_by_name(&steam_name);
    if processes.next().is_none() {
        //no steam, we need to start it
        println!("Starting steam");
        let folder = super::get_steam_path(settings);
        if let Ok(folder) = folder {
            let path = Path::new(&folder).join(steam_name);
            let mut command = Command::new(&path);
            if let Err(e) = command.spawn() {
                println!("Failed to start steam: {:?}", e);
            };
        }
    }
}
