//! Keeps a Steam shortcut "running" for as long as its game runs.
//!
//! Steam counts a non-Steam shortcut as running while the program it started, or any program
//! that one starts, is alive. Some launchers start their games themselves: the EA app's
//! `EALauncher.exe` hands the game's link to the EA app and exits within seconds, so Steam
//! shows the game as stopped while it runs. Play then starts it a second time, and the
//! controller's Home button finds no game to return to. A shortcut can instead run BoilR in
//! this mode: BoilR starts the launcher with the game's link, as its own child so Steam keeps
//! tracking it, and stays alive while any program from the game's folder runs.
//!
//! Steam's overlay (and Big Picture's Resume) only reaches a game that Steam's own process
//! tree started. When the launcher is closed, BoilR's launch starts it inside that tree, so
//! the game gets the overlay; Steam then counts the game as running until the launcher
//! closes. So BoilR closes a launcher it started, once the game has closed and the launcher
//! has had time for cloud saves ([`LaunchAndWait::cloud_sync`]). A launcher that was already
//! open is left alone: it takes the link over, outside Steam's tree, without the overlay.
//! With [`LaunchAndWait::restart_launcher`] BoilR also closes an open launcher before the
//! game, so the overlay always reaches it.
//!
//! What happens is written to `launch-and-wait.log` in BoilR's config folder.

use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
    thread::sleep,
    time::{Duration, Instant},
};

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// The command-line flag that starts BoilR in this mode instead of its interface.
pub const FLAG: &str = "--launch-and-wait";
const GAME_FOLDER: &str = "--game-folder";
const LAUNCHER: &str = "--launcher";
const LINK: &str = "--link";
const RESTART_LAUNCHER: &str = "--restart-launcher";
const CLOSE_WINDOW: &str = "--close-window";
const SYNC_LOG: &str = "--sync-log";
const SYNC_STARTED: &str = "--sync-started";
const SYNC_DONE: &str = "--sync-done";
const SETTING_FILE: &str = "--setting-file";
const SETTING_FROM: &str = "--setting-from";
const SETTING_TO: &str = "--setting-to";

/// How long the launcher gets to start the game: it may log in or update the game first.
const START_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// How long no program from the game's folder may run before the game counts as closed.
/// Some games restart themselves or hand over from a small launcher to the game.
const EXIT_GRACE: Duration = Duration::from_secs(5);
/// How long a launcher gets after the game, to upload cloud saves, before it is closed, when
/// its log can't tell when it is done. Ubisoft Connect showed a Far Cry 3 save uploaded within
/// 2 s of the game closing (tested 2026-11-13); this comes after the 5 s exit check.
const CLOUD_SAVE_GRACE: Duration = Duration::from_secs(5);
/// With a cloud sync log: how long to wait for the sync to start, after the game closed.
const SYNC_START_WAIT: Duration = Duration::from_secs(10);
/// With a cloud sync log: how long a started sync may take. The EA app itself gives up on
/// shutting down after 60 seconds.
const SYNC_TIMEOUT: Duration = Duration::from_secs(60);
/// How long a launcher that is being closed gets to go away.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_secs(1);

/// The program shortcuts run in this mode: the BoilR that is importing them, on Windows.
/// On Linux, launchers run under Proton, where Steam tracks the whole prefix.
pub fn helper() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::current_exe().ok()
    } else {
        None
    }
}

/// Runs this mode when `args` (the arguments after the program name) ask for it, and logs
/// what went wrong. `None` when they don't: the caller starts its interface instead. Call it
/// before anything else, in particular before taking BoilR's single-instance lock, since
/// the game can run while BoilR is open.
pub fn run_if_asked(args: &[String]) -> Option<eyre::Result<()>> {
    let request = LaunchAndWait::from_args(args)?;
    let result = request.and_then(|request| request.run());
    if let Err(err) = &result {
        Log::open().line(&format!("error: {err}"));
    }
    Some(result)
}

/// One game to start and wait for.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LaunchAndWait {
    /// The launcher that starts the game: `EALauncher.exe`, `upc.exe`.
    pub launcher: PathBuf,
    /// The launcher's argument: the game's link (`origin2://...`, `uplay://...`).
    pub link: String,
    /// The game's install folder. The game counts as running while a program from it runs.
    pub game_folder: PathBuf,
    /// Close an open launcher before the game too, so this launch starts it.
    pub restart_launcher: bool,
    /// The title of the launcher's window that, sent a close request, quits the launcher
    /// cleanly (the EA app's "EA"). Without it, or when that doesn't work, the launcher's
    /// programs are ended.
    pub close_window: Option<String>,
    /// The launcher's log, to see when its cloud sync after the game is done.
    pub cloud_sync: Option<CloudSyncLog>,
    /// A line of the launcher's settings to change while this launch has the launcher
    /// running, so that [`Self::close_window`] quits it (Ubisoft Connect only hides itself
    /// by default).
    pub setting: Option<SettingLine>,
}

/// A line in a launcher's settings file that BoilR changes before it starts the launcher,
/// and changes back once the launcher has exited. Only while the launcher is closed: it
/// writes its settings back when it exits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingLine {
    pub file: PathBuf,
    /// The line as the launcher has it by default (`closebehavior: CloseBehavior_Minimize`).
    pub from: String,
    /// The line while BoilR has the launcher running (`closebehavior: CloseBehavior_Close`).
    pub to: String,
}

/// Where a launcher logs its cloud sync after a game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudSyncLog {
    pub file: PathBuf,
    /// Text in a line the launcher writes when it starts syncing after a game.
    pub started: String,
    /// Text in a line the launcher writes when that sync has ended, successfully or not.
    pub done: String,
}

/// What a Steam shortcut runs: its `Exe`, `StartDir` and `LaunchOptions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutTarget {
    pub exe: String,
    pub start_dir: String,
    pub launch_options: String,
}

impl LaunchAndWait {
    /// The shortcut that runs `helper` (a BoilR executable) in this mode.
    pub fn shortcut_target(&self, helper: &Path) -> ShortcutTarget {
        let path = |path: &Path| quoted(&path.to_string_lossy());
        let mut options = vec![
            FLAG.to_string(),
            GAME_FOLDER.to_string(),
            path(&self.game_folder),
            LAUNCHER.to_string(),
            path(&self.launcher),
        ];
        let mut add = |name: &str, value: String| {
            options.push(name.to_string());
            options.push(value);
        };
        add(LINK, quoted(&self.link));
        if let Some(title) = &self.close_window {
            add(CLOSE_WINDOW, quoted(title));
        }
        if let Some(sync) = &self.cloud_sync {
            add(SYNC_LOG, path(&sync.file));
            add(SYNC_STARTED, quoted(&sync.started));
            add(SYNC_DONE, quoted(&sync.done));
        }
        if let Some(setting) = &self.setting {
            add(SETTING_FILE, path(&setting.file));
            add(SETTING_FROM, quoted(&setting.from));
            add(SETTING_TO, quoted(&setting.to));
        }
        if self.restart_launcher {
            options.push(RESTART_LAUNCHER.to_string());
        }
        ShortcutTarget {
            exe: quoted(&helper.to_string_lossy()),
            start_dir: helper
                .parent()
                .map(|folder| quoted(&folder.to_string_lossy()))
                .unwrap_or_default(),
            launch_options: options.join(" "),
        }
    }

    /// The request in a command line, when it has [`FLAG`]. `args` are the arguments after
    /// the program name.
    pub fn from_args(args: &[String]) -> Option<eyre::Result<Self>> {
        if !args.iter().any(|arg| arg == FLAG) {
            return None;
        }
        // `Ok(None)` when the flag is missing, an error when it has no value.
        let optional = |name: &str| -> eyre::Result<Option<String>> {
            let Some(at) = args.iter().position(|arg| arg == name) else {
                return Ok(None);
            };
            args.get(at + 1)
                .filter(|value| !value.starts_with("--"))
                .cloned()
                .map(Some)
                .ok_or_else(|| eyre::format_err!("{FLAG} needs {name} <value>"))
        };
        let required = |name: &str| {
            optional(name)?.ok_or_else(|| eyre::format_err!("{FLAG} needs {name} <value>"))
        };
        Some((|| {
            let cloud_sync = match optional(SYNC_LOG)? {
                Some(file) => Some(CloudSyncLog {
                    file: PathBuf::from(file),
                    started: required(SYNC_STARTED)?,
                    done: required(SYNC_DONE)?,
                }),
                None => None,
            };
            let setting = match optional(SETTING_FILE)? {
                Some(file) => Some(SettingLine {
                    file: PathBuf::from(file),
                    from: required(SETTING_FROM)?,
                    to: required(SETTING_TO)?,
                }),
                None => None,
            };
            Ok(LaunchAndWait {
                launcher: PathBuf::from(required(LAUNCHER)?),
                link: required(LINK)?,
                game_folder: PathBuf::from(required(GAME_FOLDER)?),
                restart_launcher: args.iter().any(|arg| arg == RESTART_LAUNCHER),
                close_window: optional(CLOSE_WINDOW)?,
                cloud_sync,
                setting,
            })
        })())
    }

    /// The folder of the launcher BoilR may close.
    fn launcher_folder(&self) -> Option<Folder> {
        self.launcher.parent().map(Folder::new)
    }

    /// Starts the game and returns once it has closed (or never started).
    pub fn run(&self) -> eyre::Result<()> {
        let mut log = Log::open();
        log.line(&format!("start: {self:?}"));
        // A drive's root would make every program on the drive count as the game.
        if self.game_folder.parent().is_none() {
            eyre::bail!("{} is not a game folder", self.game_folder.display());
        }
        let game_folder = Folder::new(&self.game_folder);
        let launcher_folder = self.launcher_folder();
        let mut processes = Processes::new();
        // Whether this launch starts the launcher, inside Steam's tree. Then it is closed
        // after the game, or Steam would count the game as running until it closes.
        let mut starts_launcher = false;
        // Whether this launch changed the launcher's setting line, to change back.
        let mut changed_setting = false;
        let mut sync_log_start = 0;
        if processes.any_in(&game_folder) {
            log.line("the game is already running; waiting for it");
        } else {
            if let Some(folder) = &launcher_folder {
                let was_running = processes.count_directly_in(folder) > 0;
                if was_running && self.restart_launcher {
                    // Its setting can't be changed while it runs: no close request.
                    let ask = self.setting.is_none();
                    self.close_launcher(folder, ask, &mut processes, &mut log);
                }
                starts_launcher = !was_running || self.restart_launcher;
                if let (true, Some(setting)) = (starts_launcher, &self.setting) {
                    changed_setting = setting.change(&mut log);
                }
                log.line(if starts_launcher {
                    "this launch starts the launcher; it is closed after the game"
                } else {
                    "the launcher is open; it takes the game over and stays open"
                });
            }
            sync_log_start = self.cloud_sync.as_ref().map_or(0, CloudSyncLog::length);
            let mut command = Command::new(&self.launcher);
            // Quoted, as Steam passes it: `arg` only quotes values with spaces.
            #[cfg(windows)]
            std::os::windows::process::CommandExt::raw_arg(&mut command, quoted(&self.link));
            #[cfg(not(windows))]
            command.arg(&self.link);
            if let Some(folder) = self.launcher.parent() {
                command.current_dir(folder);
            }
            // Not waited for: EALauncher.exe exits at once, Ubisoft Connect keeps running.
            command.spawn()?;
            let started = Instant::now();
            while !processes.any_in(&game_folder) {
                if started.elapsed() > START_TIMEOUT {
                    log.line("the game did not start; giving up");
                    return Ok(());
                }
                sleep(POLL);
            }
            log.line(&format!("the game started after {:?}", started.elapsed()));
        }

        let mut last_seen = Instant::now();
        while last_seen.elapsed() < EXIT_GRACE {
            sleep(POLL);
            if processes.any_in(&game_folder) {
                last_seen = Instant::now();
            }
        }
        log.line("the game closed");

        if let (true, Some(folder)) = (starts_launcher, &launcher_folder) {
            self.wait_for_cloud_sync(sync_log_start, &mut log);
            if processes.any_in(&game_folder) {
                log.line("the game started again; leaving the launcher open");
            } else {
                let ask = self.setting.is_none() || changed_setting;
                self.close_launcher(folder, ask, &mut processes, &mut log);
            }
        }
        if let (true, Some(setting)) = (changed_setting, &self.setting) {
            // Only once the launcher is gone: it writes its settings back when it exits.
            let gone = launcher_folder
                .as_ref()
                .is_none_or(|folder| processes.count_directly_in(folder) == 0);
            if gone {
                setting.change_back(&mut log);
            } else {
                log.line("the launcher still runs; its setting stays changed");
            }
        }
        Ok(())
    }

    /// Waits until the launcher is done with cloud saves after the game: until its log says
    /// so, or for a fixed time when there is no log to read.
    fn wait_for_cloud_sync(&self, log_start: u64, log: &mut Log) {
        let Some(sync) = &self.cloud_sync else {
            sleep(CLOUD_SAVE_GRACE);
            return;
        };
        let waiting = Instant::now();
        loop {
            let written = sync.written_since(log_start);
            if written.contains(&sync.done) {
                log.line("the launcher's cloud sync is done");
                return;
            }
            let limit = if written.contains(&sync.started) {
                SYNC_TIMEOUT
            } else {
                SYNC_START_WAIT
            };
            if waiting.elapsed() >= limit {
                log.line(&format!(
                    "no end of a cloud sync in the launcher's log after {limit:?}"
                ));
                return;
            }
            sleep(POLL);
        }
    }

    /// Closes the programs in the launcher's own folder (not in folders below it, where
    /// games can be installed), and waits for them to go: first by asking its window to
    /// close (when `ask`), then by ending them.
    fn close_launcher(&self, folder: &Folder, ask: bool, processes: &mut Processes, log: &mut Log) {
        if processes.count_directly_in(folder) == 0 {
            return;
        }
        if let (true, Some(title)) = (ask, &self.close_window) {
            let asked = ask_windows_to_close(&processes.pids_directly_in(folder), title);
            if asked > 0 {
                log.line("asking the launcher to close");
                let asked = Instant::now();
                while processes.count_directly_in(folder) > 0 && asked.elapsed() < CLOSE_TIMEOUT {
                    sleep(POLL);
                }
                let left = processes.count_directly_in(folder);
                if left == 0 {
                    log.line(&format!("the launcher closed after {:?}", asked.elapsed()));
                    return;
                }
                log.line(&format!(
                    "the launcher did not close ({left} programs left)"
                ));
            }
        }
        let ended = processes.kill_directly_in(folder);
        log.line(&format!("ending the launcher ({ended} programs)"));
        let ending = Instant::now();
        while processes.count_directly_in(folder) > 0 && ending.elapsed() < CLOSE_TIMEOUT {
            sleep(POLL);
            processes.kill_directly_in(folder);
        }
    }
}

impl SettingLine {
    /// Changes [`Self::from`] to [`Self::to`] when the file has that line exactly once;
    /// whether it did.
    fn change(&self, log: &mut Log) -> bool {
        let changed = replace_line(&self.file, &self.from, &self.to);
        log.line(if changed {
            "changed the launcher's setting for this launch"
        } else {
            "the launcher's setting is not the expected default; left as it is"
        });
        changed
    }

    fn change_back(&self, log: &mut Log) {
        if replace_line(&self.file, &self.to, &self.from) {
            log.line("changed the launcher's setting back");
        } else {
            log.line("could not change the launcher's setting back");
        }
    }
}

/// Replaces the one line of `file` that is `from` (ignoring indentation and line endings)
/// with `to`, keeping its indentation. False when there is no such line, or more than one.
fn replace_line(file: &Path, from: &str, to: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(file) else {
        return false;
    };
    if text.lines().filter(|line| line.trim() == from).count() != 1 {
        return false;
    }
    let changed: String = text
        .split_inclusive('\n')
        .map(|line| {
            if line.trim() != from {
                return line.to_string();
            }
            let indent = line
                .get(..line.len() - line.trim_start().len())
                .unwrap_or_default();
            let ending = line.get(line.trim_end().len()..).unwrap_or_default();
            format!("{indent}{to}{ending}")
        })
        .collect();
    std::fs::write(file, changed).is_ok()
}

impl CloudSyncLog {
    fn length(&self) -> u64 {
        log_length(&self.file)
    }

    fn written_since(&self, start: u64) -> String {
        written_since(&self.file, start)
    }
}

fn log_length(file: &Path) -> u64 {
    std::fs::metadata(file).map_or(0, |meta| meta.len())
}

/// What a launcher wrote to its log `file` after `start` bytes. All of it when the log is
/// shorter than that: it started over.
fn written_since(file: &Path, start: u64) -> String {
    // std opens files shared for reading and writing: the launcher keeps writing to it.
    let Ok(mut opened) = std::fs::File::open(file) else {
        return String::new();
    };
    let start = if log_length(file) < start { 0 } else { start };
    let mut bytes = Vec::new();
    if opened.seek(SeekFrom::Start(start)).is_err() || opened.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The top-level windows titled `title` of the programs `pids`. Process ids are only used
/// within a moment here, never stored: this runs on Windows only (no Flatpak namespaces).
#[cfg(windows)]
fn windows_titled(pids: &[u32], title: &str) -> Vec<windows_sys::Win32::Foundation::HWND> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId,
    };

    struct Search<'a> {
        pids: &'a [u32],
        title: &'a str,
        found: Vec<HWND>,
    }

    unsafe extern "system" fn each(window: HWND, search: LPARAM) -> windows_sys::core::BOOL {
        // SAFETY: `search` is the `Search` passed to EnumWindows below, alive for the call.
        let search = unsafe { &mut *(search as *mut Search) };
        let mut pid = 0u32;
        // SAFETY: a window handle from EnumWindows and a valid out pointer.
        unsafe { GetWindowThreadProcessId(window, &mut pid) };
        if search.pids.contains(&pid) {
            let mut text = [0u16; 256];
            // SAFETY: the buffer's length is passed along.
            let length = unsafe { GetWindowTextW(window, text.as_mut_ptr(), text.len() as i32) };
            let length = usize::try_from(length).unwrap_or(0).min(text.len());
            if text.get(..length).map(String::from_utf16_lossy).as_deref() == Some(search.title) {
                search.found.push(window);
            }
        }
        1
    }

    let mut search = Search {
        pids,
        title,
        found: Vec::new(),
    };
    // SAFETY: `each` only uses `search` while EnumWindows runs.
    unsafe { EnumWindows(Some(each), &mut search as *mut Search as LPARAM) };
    search.found
}

/// Sends a close request to the windows titled `title` of the programs `pids`; returns how
/// many there were.
#[cfg(windows)]
fn ask_windows_to_close(pids: &[u32], title: &str) -> usize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    let windows = windows_titled(pids, title);
    for window in &windows {
        // SAFETY: posting a message needs no memory from us.
        unsafe { PostMessageW(*window, WM_CLOSE, 0, 0) };
    }
    windows.len()
}

#[cfg(not(windows))]
fn ask_windows_to_close(_pids: &[u32], _title: &str) -> usize {
    0
}

/// `value` in double quotes for a Windows command line. Quotes inside are dropped, and so
/// are trailing backslashes: `\"` would escape the closing quote.
fn quoted(value: &str) -> String {
    let value = value.replace('"', "");
    format!("\"{}\"", value.trim_end_matches(['\\', '/']))
}

/// A path for comparing: lower case (Windows paths ignore case) with `\` separators, without
/// a trailing separator.
fn comparable(path: &Path) -> String {
    path.to_string_lossy()
        .to_lowercase()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_string()
}

/// `path` without the `\\?\` prefix that `std::fs::canonicalize` adds on Windows; programs'
/// paths come without it.
fn without_verbatim_prefix(path: String) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\unc\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path
    }
}

/// A folder to find programs in, as given and as it really is. Windows reports a program's
/// path through the real folder when the given one is a link: the EA app's
/// `EA Desktop\EA Desktop` links to a folder named after its version.
struct Folder {
    names: Vec<String>,
}

impl Folder {
    fn new(path: &Path) -> Self {
        let mut names = vec![comparable(path)];
        if let Ok(real) = std::fs::canonicalize(path) {
            let real = without_verbatim_prefix(comparable(&real));
            if !names.contains(&real) {
                names.push(real);
            }
        }
        Folder { names }
    }

    /// Whether `exe` is in this folder or a folder below it.
    fn holds(&self, exe: &Path) -> bool {
        let exe = comparable(exe);
        self.names.iter().any(|folder| {
            exe.strip_prefix(folder.as_str())
                .is_some_and(|rest| rest.starts_with('\\'))
        })
    }

    /// Whether `exe` is in this folder itself.
    fn holds_directly(&self, exe: &Path) -> bool {
        comparable(exe)
            .rsplit_once('\\')
            .is_some_and(|(parent, _)| self.names.iter().any(|folder| folder == parent))
    }
}

/// The programs running on this computer, by the path of their executable.
struct Processes {
    system: System,
}

impl Processes {
    fn new() -> Self {
        Processes {
            system: System::new(),
        }
    }

    fn refresh(&mut self) {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
    }

    fn any_in(&mut self, folder: &Folder) -> bool {
        self.refresh();
        self.system
            .processes()
            .values()
            .any(|process| process.exe().is_some_and(|exe| folder.holds(exe)))
    }

    fn count_directly_in(&mut self, folder: &Folder) -> usize {
        self.refresh();
        self.system
            .processes()
            .values()
            .filter(|process| process.exe().is_some_and(|exe| folder.holds_directly(exe)))
            .count()
    }

    /// The process ids of the programs in `folder` itself, for finding their windows.
    fn pids_directly_in(&mut self, folder: &Folder) -> Vec<u32> {
        self.refresh();
        self.system
            .processes()
            .iter()
            .filter(|(_, process)| process.exe().is_some_and(|exe| folder.holds_directly(exe)))
            .map(|(pid, _)| pid.as_u32())
            .collect()
    }

    /// Ends the programs in `folder` itself; returns how many there were.
    fn kill_directly_in(&mut self, folder: &Folder) -> usize {
        self.refresh();
        let mut found = 0;
        for process in self.system.processes().values() {
            if process.exe().is_some_and(|exe| folder.holds_directly(exe)) {
                process.kill();
                found += 1;
            }
        }
        found
    }
}

/// `launch-and-wait.log` in BoilR's config folder. This mode has no window to show errors
/// in. The log starts over once it grows past 256 KB.
struct Log {
    file: Option<std::fs::File>,
}

impl Log {
    fn open() -> Self {
        let folder = crate::config::get_config_folder();
        let _ = std::fs::create_dir_all(&folder);
        let path = folder.join("launch-and-wait.log");
        let too_big = std::fs::metadata(&path).is_ok_and(|meta| meta.len() > 256 * 1024);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(!too_big)
            .write(true)
            .truncate(too_big)
            .open(path)
            .ok();
        Log { file }
    }

    fn line(&mut self, message: &str) {
        let now = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        if let Some(file) = self.file.as_mut() {
            let _ = writeln!(file, "{now} {message}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &[&str]) -> Vec<String> {
        line.iter().map(|arg| arg.to_string()).collect()
    }

    fn alice() -> LaunchAndWait {
        LaunchAndWait {
            launcher: PathBuf::from(r"C:\EA Desktop\EALauncher.exe"),
            link: "origin2://game/launch?offerIds=alice2_dd".to_string(),
            game_folder: PathBuf::from(r"D:\OriginLibrary\Alice Madness Returns\"),
            ..LaunchAndWait::default()
        }
    }

    /// A command line split as Windows does for these values: at spaces outside quotes, and
    /// without the quotes.
    fn split(line: &str) -> Vec<String> {
        let mut args = vec![];
        let mut current = String::new();
        let mut quoted = false;
        for char in line.chars() {
            match char {
                '"' => quoted = !quoted,
                ' ' if !quoted => args.push(std::mem::take(&mut current)),
                _ => current.push(char),
            }
        }
        args.push(current);
        args
    }

    #[test]
    fn the_shortcut_runs_boilr_with_quoted_values() {
        let target = alice().shortcut_target(Path::new(r"C:\BoilR\BoilR.exe"));
        assert_eq!(target.exe, r#""C:\BoilR\BoilR.exe""#);
        assert_eq!(
            target.launch_options,
            r#"--launch-and-wait --game-folder "D:\OriginLibrary\Alice Madness Returns" --launcher "C:\EA Desktop\EALauncher.exe" --link "origin2://game/launch?offerIds=alice2_dd""#
        );
        // Only Windows reads `C:\` paths as folders.
        #[cfg(windows)]
        assert_eq!(target.start_dir, r#""C:\BoilR""#);

        let ubisoft = LaunchAndWait {
            restart_launcher: true,
            ..alice()
        };
        assert!(ubisoft
            .shortcut_target(Path::new(r"C:\BoilR\BoilR.exe"))
            .launch_options
            .ends_with(" --restart-launcher"));
    }

    #[test]
    fn reads_its_command_line() -> eyre::Result<()> {
        // As Windows splits the shortcut's command line: the quotes are gone.
        let line = args(&[
            "--launch-and-wait",
            "--game-folder",
            r"D:\OriginLibrary\Alice Madness Returns",
            "--launcher",
            r"C:\EA Desktop\EALauncher.exe",
            "--link",
            "origin2://game/launch?offerIds=alice2_dd",
        ]);
        let request = LaunchAndWait::from_args(&line).ok_or(eyre::format_err!("no request"))??;
        assert_eq!(
            request.launcher,
            PathBuf::from(r"C:\EA Desktop\EALauncher.exe")
        );
        assert_eq!(request.link, "origin2://game/launch?offerIds=alice2_dd");
        assert_eq!(
            request.game_folder,
            PathBuf::from(r"D:\OriginLibrary\Alice Madness Returns")
        );
        assert!(!request.restart_launcher);

        let mut restart = line.clone();
        restart.push("--restart-launcher".to_string());
        let restart =
            LaunchAndWait::from_args(&restart).ok_or(eyre::format_err!("no request"))??;
        assert!(restart.restart_launcher);

        assert!(LaunchAndWait::from_args(&args(&["--no-ui"])).is_none());
        let missing_link = args(&["--launch-and-wait", "--game-folder", "D:\\Game", "--link"]);
        assert!(matches!(
            LaunchAndWait::from_args(&missing_link),
            Some(Err(_))
        ));
        let no_link = args(&[
            "--launch-and-wait",
            "--game-folder",
            r"D:\Game",
            "--launcher",
            r"C:\EA Desktop\EALauncher.exe",
        ]);
        assert!(matches!(LaunchAndWait::from_args(&no_link), Some(Err(_))));
        Ok(())
    }

    #[test]
    fn every_option_survives_the_shortcut() -> eyre::Result<()> {
        let ea = LaunchAndWait {
            game_folder: PathBuf::from(r"D:\OriginLibrary\Alice Madness Returns"),
            restart_launcher: true,
            close_window: Some("EA".to_string()),
            cloud_sync: Some(CloudSyncLog {
                file: PathBuf::from(r"C:\Users\me\AppData\Local\EA\Logs\EADesktop.log"),
                started: "Cloud Sync entry point post game".to_string(),
                done: "handleTerminalCloudSyncStatus".to_string(),
            }),
            ..alice()
        };
        let ubisoft = LaunchAndWait {
            launcher: PathBuf::from(r"C:\Ubisoft Game Launcher\upc.exe"),
            link: "uplay://launch/46/0".to_string(),
            game_folder: PathBuf::from(r"D:\UplayLibrary\Far Cry 3"),
            close_window: Some("Ubisoft Connect".to_string()),
            setting: Some(SettingLine {
                file: PathBuf::from(r"C:\Users\me\Ubisoft Game Launcher\settings.yaml"),
                from: "closebehavior: CloseBehavior_Minimize".to_string(),
                to: "closebehavior: CloseBehavior_Close".to_string(),
            }),
            ..LaunchAndWait::default()
        };
        for request in [ea, ubisoft] {
            let target = request.shortcut_target(Path::new(r"C:\BoilR\BoilR.exe"));
            let read = LaunchAndWait::from_args(&split(&target.launch_options))
                .ok_or(eyre::format_err!("no request"))??;
            assert_eq!(read, request);
        }
        Ok(())
    }

    #[test]
    fn changes_one_setting_line_and_back() -> eyre::Result<()> {
        let file = std::env::temp_dir().join(format!("boilr-setting-{}.yaml", std::process::id()));
        let original = "misc:\r\n  closebehavior: CloseBehavior_Minimize\r\n  other: 1\r\n";
        std::fs::write(&file, original)?;
        let setting = SettingLine {
            file: file.clone(),
            from: "closebehavior: CloseBehavior_Minimize".to_string(),
            to: "closebehavior: CloseBehavior_Close".to_string(),
        };
        let mut log = Log { file: None };
        assert!(setting.change(&mut log));
        assert_eq!(
            std::fs::read_to_string(&file)?,
            "misc:\r\n  closebehavior: CloseBehavior_Close\r\n  other: 1\r\n"
        );
        // Already changed: not the expected default, left alone.
        assert!(!setting.change(&mut log));
        setting.change_back(&mut log);
        assert_eq!(std::fs::read_to_string(&file)?, original);
        std::fs::remove_file(&file)?;
        Ok(())
    }

    #[test]
    fn reads_what_the_launcher_logged_since_the_game_started() -> eyre::Result<()> {
        let file = std::env::temp_dir().join(format!("boilr-sync-log-{}.log", std::process::id()));
        std::fs::write(&file, "old line: handleTerminalCloudSyncStatus\n")?;
        let sync = CloudSyncLog {
            file: file.clone(),
            started: "post game".to_string(),
            done: "handleTerminalCloudSyncStatus".to_string(),
        };
        let start = sync.length();
        assert_eq!(sync.written_since(start), "");
        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)?
            .write_all(b"post game sync\n")?;
        assert_eq!(sync.written_since(start), "post game sync\n");
        // The launcher started a new, shorter log.
        std::fs::write(&file, "new\n")?;
        assert_eq!(sync.written_since(start), "new\n");
        std::fs::remove_file(&file)?;
        Ok(())
    }

    #[test]
    fn programs_belong_to_folders_by_path() {
        let game = Folder::new(Path::new(r"D:\UplayLibrary\Far Cry 3\"));
        assert!(game.holds(Path::new(r"d:\uplaylibrary\far cry 3\bin\farcry3.exe")));
        assert!(!game.holds(Path::new(
            r"D:\UplayLibrary\Far Cry 3 Blood Dragon\bin\fc3.exe"
        )));
        assert!(!game.holds(Path::new(r"D:\UplayLibrary\Far Cry 3")));

        let launcher = Folder::new(Path::new(
            r"C:\Program Files (x86)\Ubisoft\Ubisoft Game Launcher",
        ));
        assert!(launcher.holds_directly(Path::new(
            r"C:\Program Files (x86)\Ubisoft\Ubisoft Game Launcher\upc.exe"
        )));
        // Games can be installed below the launcher's folder; those are not the launcher.
        assert!(!launcher.holds_directly(Path::new(
            r"C:\Program Files (x86)\Ubisoft\Ubisoft Game Launcher\games\Far Cry 3\bin\farcry3.exe"
        )));
    }

    #[test]
    fn a_folder_also_matches_where_it_really_is() -> eyre::Result<()> {
        // "." stands in for a link: the given name differs from the real path.
        let real = std::env::current_dir()?;
        let folder = Folder::new(Path::new("."));
        assert!(folder.holds(&real.join("bin").join("game.exe")));
        assert!(folder.holds_directly(&real.join("launcher.exe")));
        Ok(())
    }

    #[test]
    fn canonical_paths_lose_their_verbatim_prefix() {
        assert_eq!(
            without_verbatim_prefix(r"\\?\c:\program files\ea".to_string()),
            r"c:\program files\ea"
        );
        assert_eq!(
            without_verbatim_prefix(r"\\?\unc\server\share\ea".to_string()),
            r"\\server\share\ea"
        );
        assert_eq!(without_verbatim_prefix(r"c:\ea".to_string()), r"c:\ea");
    }
}
