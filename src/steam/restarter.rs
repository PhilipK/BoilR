use std::{process::Command, thread::sleep, time::Duration};

use sysinfo::{ProcessExt, System, SystemExt};

pub fn ensure_steam_stopped() {
    #[cfg(target_os = "windows")]
    let steam_name = "steam.exe";
    #[cfg(target_family = "unix")]
    let steam_name = "steam";

    let s = System::new_all();
    let processes = s.processes_by_name(steam_name);
    for process in processes {
        let mut s = System::new();
        process.kill_with(sysinfo::Signal::Quit);
        process.kill_with(sysinfo::Signal::Kill);
        while s.refresh_process(process.pid()) {
            println!("Waiting for steam to stop");
            sleep(Duration::from_millis(500));
            process.kill_with(sysinfo::Signal::Quit);
            process.kill_with(sysinfo::Signal::Kill);
        }
    }
}
#[cfg(target_os = "windows")]
pub fn ensure_steam_started(settings: &super::SteamSettings) {
    let steam_name = "steam.exe";
    let s = System::new_all();
    let mut processes = s.processes_by_name(steam_name);
    if processes.next().is_none() {
        //no steam, we need to start it
        println!("Starting steam");
        let folder = super::get_steam_path(settings);
        if let Ok(folder) = folder {
            let path = std::path::Path::new(&folder).join(steam_name);
            let mut command = Command::new(path);
            if let Err(e) = command.spawn() {
                println!("Failed to start steam: {:?}", e);
            };
        }
    }
}

#[cfg(target_family = "unix")]
pub fn ensure_steam_started(_settings: &super::SteamSettings) {
    let steam_name = "steam";
    let s = System::new_all();
    let mut processes = s.processes_by_name(steam_name);
    if processes.next().is_none() {
        //no steam, we need to start it
        println!("Starting steam");
        let mut command = Command::new(steam_name);
        if let Err(e) = command.spawn() {
            println!("Failed to start steam: {e:?}");
        };
    }
}
