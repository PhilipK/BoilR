use std::{
    ffi::{OsStr, OsString},
    process::Command,
    thread::sleep,
    time::{Duration, Instant},
};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

#[cfg(target_os = "windows")]
const STEAM_BINARY: &str = "steam.exe";
#[cfg(target_family = "unix")]
const STEAM_BINARY: &str = "steam";

/// How long Steam gets to close itself after being asked to before we stop waiting and
/// fall back to signals. Steam saves its config on the way out, so giving it a few
/// seconds is normal.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(20);

/// Stop Steam before writing the non-Steam shortcuts, so the client doesn't overwrite
/// them behind our back.
///
/// Steam is asked to close itself first, with the `steam -shutdown` client command. A
/// signal does not do the job: the client does not handle `Signal::Quit`, so the old
/// `Quit`-then-`Kill` sequence went straight to SIGKILL, which can cut the client off
/// while it is writing. The signals are kept as a fallback, for a client that does not
/// react to the request, or that could not be asked at all.
pub fn ensure_steam_stopped(settings: &super::SteamSettings) {
    let pids = steam_pids();
    if pids.is_empty() {
        println!("Steam is stopped");
        return;
    }

    if ask_steam_to_shutdown(settings) && wait_for_steam_to_exit(&pids, SHUTDOWN_GRACE) {
        println!("Steam is stopped");
        return;
    }

    stop_steam_with_signals();
    println!("Steam is stopped");
}

fn steam_pids() -> Vec<Pid> {
    let s = System::new_all();
    s.processes_by_name(OsStr::new(STEAM_BINARY))
        .map(|process| process.pid())
        .collect()
}

/// The program and arguments used to ask the Steam client to close itself: `steam
/// -shutdown`, which Valve documents as "Shuts down (exits) Steam".
fn shutdown_command(settings: &super::SteamSettings) -> (OsString, Vec<OsString>) {
    (
        steam_shutdown_program(settings),
        vec![OsString::from("-shutdown")],
    )
}

/// On Windows the client is not on `PATH`, so the configured (or default) Steam folder
/// is used, the same way `ensure_steam_started` finds the binary.
#[cfg(target_os = "windows")]
fn steam_shutdown_program(settings: &super::SteamSettings) -> OsString {
    match super::get_steam_path(settings) {
        Ok(folder) => std::path::Path::new(&folder)
            .join(STEAM_BINARY)
            .into_os_string(),
        Err(_) => OsString::from(STEAM_BINARY),
    }
}

#[cfg(target_family = "unix")]
fn steam_shutdown_program(_settings: &super::SteamSettings) -> OsString {
    OsString::from(STEAM_BINARY)
}

/// Whether the request reached Steam. `false` means we never got a clean shutdown
/// request in, so the caller falls back to stopping it by signal.
fn ask_steam_to_shutdown(settings: &super::SteamSettings) -> bool {
    let (program, args) = shutdown_command(settings);
    let mut command = program.to_string_lossy().into_owned();
    for arg in &args {
        command.push(' ');
        command.push_str(&arg.to_string_lossy());
    }
    match Command::new(&program).args(&args).status() {
        Ok(status) if status.success() => {
            println!("Asked steam to shut down with `{command}`");
            true
        }
        Ok(status) => {
            println!("`{command}` exited with {status}, stopping steam by signal instead");
            false
        }
        Err(e) => {
            println!("Failed to run `{command}`: {e}, stopping steam by signal instead");
            false
        }
    }
}

/// Wait for the given Steam processes to disappear. Whether they all exited within
/// `timeout`.
fn wait_for_steam_to_exit(pids: &[Pid], timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        let running = running_pids(pids);
        if running.is_empty() {
            return true;
        }
        if Instant::now() >= deadline {
            println!(
                "Steam did not shut down within {}s, still running: {running:?}",
                timeout.as_secs()
            );
            return false;
        }
        println!("Waiting for steam to shut down. Still running: {running:?}");
        sleep(Duration::from_millis(500));
    }
}

fn running_pids(pids: &[Pid]) -> Vec<Pid> {
    let mut s = System::new();
    pids.iter()
        .copied()
        .filter(|pid| {
            let pid_arr = [*pid];
            s.refresh_processes_specifics(
                ProcessesToUpdate::Some(&pid_arr),
                true,
                ProcessRefreshKind::everything(),
            );
            s.process(*pid).is_some()
        })
        .collect()
}

fn stop_steam_with_signals() {
    let s = System::new_all();
    let processes = s.processes_by_name(OsStr::new(STEAM_BINARY));
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
}

#[cfg(target_os = "windows")]
pub fn ensure_steam_started(settings: &super::SteamSettings) {
    let os_steam_name = OsStr::new(STEAM_BINARY);
    let s = System::new_all();
    let mut processes = s.processes_by_name(os_steam_name);
    if processes.next().is_none() {
        //no steam, we need to start it
        println!("Starting steam");
        let folder = super::get_steam_path(settings);
        if let Ok(folder) = folder {
            let path = std::path::Path::new(&folder).join(STEAM_BINARY);
            let mut command = Command::new(path);
            if let Err(e) = command.spawn() {
                println!("Failed to start steam: {:?}", e);
            };
        }
    }
}

#[cfg(target_family = "unix")]
pub fn ensure_steam_started(_settings: &super::SteamSettings) {
    let os_steam_name = OsStr::new(STEAM_BINARY);
    let s = System::new_all();
    let mut processes = s.processes_by_name(os_steam_name);
    if processes.next().is_none() {
        //no steam, we need to start it
        println!("Starting steam");
        let mut command = Command::new(STEAM_BINARY);
        if let Err(e) = command.spawn() {
            println!("Failed to start steam: {e:?}");
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_command_asks_the_client_to_exit() {
        let (program, args) = shutdown_command(&super::super::SteamSettings::default());
        assert_eq!(
            args,
            vec![OsString::from("-shutdown")],
            "steam -shutdown is the client command that closes the client"
        );
        assert!(
            program.to_string_lossy().ends_with(STEAM_BINARY),
            "the request goes to the steam client, got {program:?}"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn shutdown_command_uses_the_configured_steam_folder() {
        let settings = super::super::SteamSettings {
            location: Some(r"C:\Program Files (x86)\Steam".to_string()),
            ..Default::default()
        };
        let (program, args) = shutdown_command(&settings);
        assert_eq!(
            program,
            OsString::from(r"C:\Program Files (x86)\Steam\steam.exe")
        );
        assert_eq!(args, vec![OsString::from("-shutdown")]);
    }

    #[test]
    fn waiting_is_over_once_no_process_is_left() {
        // A pid that is not running must not cost the whole grace period.
        let start = Instant::now();
        assert!(wait_for_steam_to_exit(
            &[Pid::from_u32(u32::MAX)],
            Duration::from_secs(30)
        ));
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}
