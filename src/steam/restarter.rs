use std::{ffi::OsStr, process::Command, thread::sleep, time::Duration};

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

pub fn ensure_steam_stopped() {
    #[cfg(target_os = "windows")]
    let steam_name = "steam.exe";
    #[cfg(target_family = "unix")]
    let steam_name = "steam";

    let os_steam_name = OsStr::new(steam_name);

    let s = System::new_all();
    let processes = s.processes_by_name(os_steam_name);
    for process in processes {
        let mut s = System::new();

        let quit_res = process.kill_with(sysinfo::Signal::Quit);
        let kill_res = process.kill_with(sysinfo::Signal::Kill);

        if quit_res == Some(false) && kill_res == Some(false) {
            // Couldn't kill the process, this could be because it was already killed or we don't have permissions
            // For instance, the process "steamos-manager" in the Steam Deck is owned by root
            continue;
        }
        
        let pid = process.pid();
        let process_name = process.name();
        let pid_arr = [pid];
        let process_to_update = ProcessesToUpdate::Some(&pid_arr);

        while s.refresh_processes_specifics(process_to_update, true,ProcessRefreshKind::everything()) > 0 
                // The process is still alive
                && s.process(pid).is_some()
                 {
            println!("Waiting for steam to stop. PID: {pid:?} Name: {process_name:?}");
            sleep(Duration::from_millis(500));
            process.kill_with(sysinfo::Signal::Quit);
            process.kill_with(sysinfo::Signal::Kill);
        }
    }
    println!("Steam is stopped");
}
#[cfg(target_os = "windows")]
pub fn ensure_steam_started(settings: &super::SteamSettings) {
    let steam_name = "steam.exe";
    let os_steam_name = OsStr::new(steam_name);
    let s = System::new_all();
    let mut processes = s.processes_by_name(os_steam_name);
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
    let os_steam_name = OsStr::new(steam_name);
    let s = System::new_all();
    let mut processes = s.processes_by_name(os_steam_name);
    if processes.next().is_none() {
        //no steam, we need to start it
        println!("Starting steam");
        let mut command = Command::new(steam_name);
        if let Err(e) = command.spawn() {
            println!("Failed to start steam: {e:?}");
        };
    }
}
