use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use super::game::StandaloneGame;

const IGNORED_EXECUTABLE_PATTERNS: [&str; 17] = [
    "unins",
    "uninstall",
    "uninstaller",
    "setup",
    "install",
    "installer",
    "update",
    "updater",
    "launcher",
    "helper",
    "reporter",
    "crashhandler",
    "crashreporter",
    "crashpadhandler",
    "dxsetup",
    "vcredist",
    "redistributable",
];

#[derive(Debug)]
struct Candidate {
    game: StandaloneGame,
    distance: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ManualExecutableCandidate {
    pub(crate) title: String,
    pub(crate) game_directory: PathBuf,
    pub(crate) executable: PathBuf,
}

#[cfg(test)]
pub(crate) fn scan_directories(directories: &[String]) -> eyre::Result<Vec<StandaloneGame>> {
    scan_directories_with_selections(directories, &[])
}

pub(crate) fn scan_directories_with_selections(
    directories: &[String],
    selected_executables: &[String],
) -> eyre::Result<Vec<StandaloneGame>> {
    let mut candidates = HashMap::new();
    let roots = visit_configured_roots(directories, |root| {
        walk_root(root, |executable| {
            consider_executable(root, executable, &mut candidates);
        })
    })?;
    add_manual_selections(selected_executables, &roots, &mut candidates);

    let mut games: Vec<StandaloneGame> = candidates
        .into_values()
        .map(|candidate| candidate.game)
        .collect();
    games.sort_by(|left, right| {
        left.title
            .to_lowercase()
            .cmp(&right.title.to_lowercase())
            .then_with(|| {
                left.executable
                    .to_string_lossy()
                    .to_lowercase()
                    .cmp(&right.executable.to_string_lossy().to_lowercase())
            })
    });
    Ok(games)
}

pub(crate) fn find_unmatched_executables(
    directories: &[String],
) -> eyre::Result<Vec<ManualExecutableCandidate>> {
    let mut candidates = HashMap::new();
    visit_configured_roots(directories, |root| {
        walk_root(root, |executable| {
            consider_unmatched_executable(root, executable, &mut candidates);
        })
    })?;

    let mut candidates: Vec<ManualExecutableCandidate> = candidates.into_values().collect();
    candidates.sort_by(|left, right| {
        left.title
            .to_lowercase()
            .cmp(&right.title.to_lowercase())
            .then_with(|| {
                left.executable
                    .to_string_lossy()
                    .to_lowercase()
                    .cmp(&right.executable.to_string_lossy().to_lowercase())
            })
    });
    Ok(candidates)
}

fn visit_configured_roots<F>(directories: &[String], mut visit: F) -> eyre::Result<Vec<PathBuf>>
where
    F: FnMut(&Path) -> std::io::Result<()>,
{
    let configured_directories: Vec<&str> = directories
        .iter()
        .map(|directory| directory.trim())
        .filter(|directory| !directory.is_empty())
        .collect();
    if configured_directories.is_empty() {
        return Err(eyre::eyre!("No standalone scan directories configured"));
    }

    let mut scanned_roots = HashSet::new();
    let mut scanned_directory_count = 0;
    let mut valid_roots = Vec::new();
    let mut errors = Vec::new();

    for configured_directory in configured_directories {
        let configured_path = Path::new(configured_directory);
        let canonical_root = match configured_path.canonicalize() {
            Ok(root) if root.is_dir() => root,
            Ok(_) => {
                let message =
                    format!("Standalone scan path is not a directory: {configured_path:?}");
                eprintln!("{message}");
                errors.push(message);
                continue;
            }
            Err(error) => {
                let message =
                    format!("Could not access standalone scan path {configured_path:?}: {error}");
                eprintln!("{message}");
                errors.push(message);
                continue;
            }
        };

        let root = if configured_path.is_absolute() {
            configured_path.to_path_buf()
        } else {
            match std::env::current_dir() {
                Ok(current_directory) => current_directory.join(configured_path),
                Err(error) => {
                    let message = format!(
                        "Could not resolve standalone scan path {configured_path:?}: {error}"
                    );
                    eprintln!("{message}");
                    errors.push(message);
                    continue;
                }
            }
        };

        if !scanned_roots.insert(canonical_root) {
            continue;
        }

        match visit(&root) {
            Ok(()) => {
                scanned_directory_count += 1;
                valid_roots.push(root);
            }
            Err(error) => {
                let message = format!("Could not scan standalone directory {root:?}: {error}");
                eprintln!("{message}");
                errors.push(message);
            }
        }
    }

    if scanned_directory_count == 0 {
        return Err(eyre::eyre!(
            "None of the configured standalone directories could be scanned: {}",
            errors.join("; ")
        ));
    }

    Ok(valid_roots)
}

fn walk_root<F>(root: &Path, mut visit: F) -> std::io::Result<()>
where
    F: FnMut(&Path),
{
    let initial_entries = fs::read_dir(root)?;
    let mut stack = vec![(root.to_path_buf(), Some(initial_entries))];

    while let Some((directory, supplied_entries)) = stack.pop() {
        let entries = match supplied_entries {
            Some(entries) => entries,
            None => match fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) => {
                    eprintln!("Could not read standalone subdirectory {directory:?}: {error}");
                    continue;
                }
            },
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    eprintln!("Could not read an entry under {directory:?}: {error}");
                    continue;
                }
            };
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    eprintln!(
                        "Could not inspect standalone path {:?}: {error}",
                        entry.path()
                    );
                    continue;
                }
            };

            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push((entry.path(), None));
            } else if file_type.is_file() {
                visit(&entry.path());
            }
        }
    }

    Ok(())
}

fn add_manual_selections(
    selected_executables: &[String],
    roots: &[PathBuf],
    candidates: &mut HashMap<PathBuf, Candidate>,
) {
    for selected_executable in selected_executables {
        let executable = Path::new(selected_executable.trim());
        if !executable.is_file()
            || !is_safe_executable(executable)
            || !is_executable_under_roots(executable, roots)
        {
            eprintln!("Selected standalone executable is unavailable: {executable:?}");
            continue;
        }
        let Some((matched_directory, title)) = fallback_game_identity(executable) else {
            continue;
        };
        let key = matched_directory
            .canonicalize()
            .unwrap_or(matched_directory);
        candidates.insert(
            key,
            Candidate {
                game: StandaloneGame {
                    title,
                    executable: executable.to_path_buf(),
                },
                distance: 0,
            },
        );
    }
}

fn is_executable_under_roots(executable: &Path, roots: &[PathBuf]) -> bool {
    let Ok(canonical_executable) = executable.canonicalize() else {
        return false;
    };
    roots.iter().any(|root| {
        root.canonicalize()
            .is_ok_and(|canonical_root| canonical_executable.starts_with(canonical_root))
    })
}

fn consider_unmatched_executable(
    root: &Path,
    executable: &Path,
    candidates: &mut HashMap<PathBuf, ManualExecutableCandidate>,
) {
    if !is_safe_executable(executable) {
        return;
    }
    let Some(stem) = normalized_stem(executable) else {
        return;
    };
    if find_matching_ancestor(root, executable, &stem).is_some() {
        return;
    }
    let Some((game_directory, title)) = fallback_game_identity(executable) else {
        return;
    };
    let key = executable
        .canonicalize()
        .unwrap_or_else(|_| executable.to_path_buf());
    candidates.insert(
        key,
        ManualExecutableCandidate {
            title,
            game_directory,
            executable: executable.to_path_buf(),
        },
    );
}

fn consider_executable(
    root: &Path,
    executable: &Path,
    candidates: &mut HashMap<PathBuf, Candidate>,
) {
    if !is_safe_executable(executable) {
        return;
    }

    let Some(stem) = normalized_stem(executable) else {
        return;
    };

    let Some((matched_directory, title)) = find_matching_ancestor(root, executable, &stem) else {
        return;
    };
    let distance = executable
        .parent()
        .and_then(|parent| parent.strip_prefix(&matched_directory).ok())
        .map(|relative| relative.components().count())
        .unwrap_or(usize::MAX);
    let key = matched_directory
        .canonicalize()
        .unwrap_or_else(|_| matched_directory.clone());
    let candidate = Candidate {
        game: StandaloneGame {
            title,
            executable: executable.to_path_buf(),
        },
        distance,
    };

    match candidates.entry(key) {
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(candidate);
        }
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            if is_better_candidate(&candidate, entry.get()) {
                entry.insert(candidate);
            }
        }
    }
}

fn normalized_stem(path: &Path) -> Option<String> {
    path.file_stem()
        .map(|stem| normalize_name(&stem.to_string_lossy()))
}

fn is_safe_executable(path: &Path) -> bool {
    if !has_exe_extension(path) {
        return false;
    }
    normalized_stem(path).is_some_and(|stem| !stem.is_empty() && !is_ignored_executable(&stem))
}

fn fallback_game_identity(executable: &Path) -> Option<(PathBuf, String)> {
    let mut directory = executable.parent()?.to_path_buf();
    while directory.file_name().is_some_and(|name| {
        is_generic_executable_directory(&normalize_name(&name.to_string_lossy()))
    }) {
        let Some(parent) = directory.parent() else {
            break;
        };
        directory = parent.to_path_buf();
    }

    let title = directory
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .or_else(|| {
            executable
                .file_stem()
                .map(|name| name.to_string_lossy().to_string())
        })?;
    Some((directory, title))
}

fn is_generic_executable_directory(normalized_name: &str) -> bool {
    matches!(
        normalized_name,
        "bin"
            | "bin32"
            | "bin64"
            | "binaries"
            | "retail"
            | "shipping"
            | "win32"
            | "win64"
            | "x64"
            | "x86"
    )
}

fn has_exe_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
}

fn normalize_name(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_ignored_executable(normalized_stem: &str) -> bool {
    IGNORED_EXECUTABLE_PATTERNS.iter().any(|pattern| {
        normalized_stem == *pattern
            || normalized_stem.starts_with(pattern)
            || normalized_stem.ends_with(pattern)
    })
}

fn find_matching_ancestor(
    root: &Path,
    executable: &Path,
    normalized_stem: &str,
) -> Option<(PathBuf, String)> {
    let mut current = executable.parent();
    while let Some(directory) = current {
        if let Some(folder_name) = directory.file_name() {
            let title = folder_name.to_string_lossy();
            if normalize_name(&title) == normalized_stem {
                return Some((directory.to_path_buf(), title.to_string()));
            }
        }
        if directory == root {
            break;
        }
        current = directory.parent();
    }
    None
}

fn is_better_candidate(candidate: &Candidate, current: &Candidate) -> bool {
    candidate.distance < current.distance
        || (candidate.distance == current.distance
            && candidate.game.executable.to_string_lossy().to_lowercase()
                < current.game.executable.to_string_lossy().to_lowercase())
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, File},
        path::Path,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    static NEXT_TEMP_ID: AtomicUsize = AtomicUsize::new(0);

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> eyre::Result<Self> {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("boilr-standalone-test-{}-{id}", std::process::id()));
            fs::create_dir_all(&path)?;
            Ok(Self { path })
        }

        fn directory(&self, relative: &str) -> eyre::Result<PathBuf> {
            let path = self.path.join(relative);
            fs::create_dir_all(&path)?;
            Ok(path)
        }

        fn file(&self, relative: &str) -> eyre::Result<PathBuf> {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            File::create(&path)?;
            Ok(path)
        }

        fn configured_path(&self) -> String {
            self.path.to_string_lossy().to_string()
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn recursively_finds_case_insensitive_exe_with_normalized_match() -> eyre::Result<()> {
        let test_directory = TestDirectory::new()?;
        let executable = test_directory.file("My Game/bin/My_Game.EXE")?;

        let games = scan_directories(&[test_directory.configured_path()])?;

        assert_eq!(games.len(), 1);
        assert_eq!(
            games.first().map(|game| game.title.as_str()),
            Some("My Game")
        );
        assert_eq!(
            games.first().map(|game| &game.executable),
            Some(&executable)
        );
        Ok(())
    }

    #[test]
    fn lists_and_imports_manually_selected_unmatched_executable() -> eyre::Result<()> {
        let test_directory = TestDirectory::new()?;
        let executable = test_directory.file("Death Stranding 2/bin/DS2.exe")?;

        let unmatched = find_unmatched_executables(&[test_directory.configured_path()])?;

        assert_eq!(unmatched.len(), 1);
        assert_eq!(
            unmatched.first().map(|candidate| candidate.title.as_str()),
            Some("Death Stranding 2")
        );
        assert_eq!(
            unmatched.first().map(|candidate| &candidate.executable),
            Some(&executable)
        );
        assert_eq!(
            unmatched
                .first()
                .map(|candidate| candidate.game_directory.file_name()),
            Some(Some(std::ffi::OsStr::new("Death Stranding 2")))
        );

        let games = scan_directories_with_selections(
            &[test_directory.configured_path()],
            &[executable.to_string_lossy().to_string()],
        )?;
        assert_eq!(games.len(), 1);
        assert_eq!(
            games.first().map(|game| game.title.as_str()),
            Some("Death Stranding 2")
        );
        assert_eq!(
            games.first().map(|game| &game.executable),
            Some(&executable)
        );
        Ok(())
    }

    #[test]
    fn ignores_manual_selection_outside_configured_roots() -> eyre::Result<()> {
        let test_directory = TestDirectory::new()?;
        let outside = TestDirectory::new()?;
        let executable = outside.file("Outside Game/Outside.exe")?;

        let games = scan_directories_with_selections(
            &[test_directory.configured_path()],
            &[executable.to_string_lossy().to_string()],
        )?;

        assert!(games.is_empty());
        Ok(())
    }

    #[test]
    fn manual_selection_overrides_automatic_choice_for_folder() -> eyre::Result<()> {
        let test_directory = TestDirectory::new()?;
        test_directory.file("Death Stranding 2/DeathStranding2.exe")?;
        let selected = test_directory.file("Death Stranding 2/DS2.exe")?;

        let games = scan_directories_with_selections(
            &[test_directory.configured_path()],
            &[selected.to_string_lossy().to_string()],
        )?;

        assert_eq!(games.len(), 1);
        assert_eq!(games.first().map(|game| &game.executable), Some(&selected));
        Ok(())
    }

    #[test]
    fn configured_root_can_be_the_game_directory() -> eyre::Result<()> {
        let test_directory = TestDirectory::new()?;
        let game_root = test_directory.directory("Root Game")?;
        test_directory.file("Root Game/RootGame.exe")?;

        let games = scan_directories(&[game_root.to_string_lossy().to_string()])?;

        assert_eq!(
            games.first().map(|game| game.title.as_str()),
            Some("Root Game")
        );
        Ok(())
    }

    #[test]
    fn rejects_non_matching_and_ignored_executables() -> eyre::Result<()> {
        let test_directory = TestDirectory::new()?;
        test_directory.file("Some Game/Other.exe")?;
        test_directory.file("Launcher/Launcher.exe")?;
        test_directory.file("Updater/Updater.exe")?;
        test_directory.file("Crash Handler/CrashHandler.exe")?;
        test_directory.file("Uninstall/Uninstall.exe")?;

        let games = scan_directories(&[test_directory.configured_path()])?;

        assert!(games.is_empty());
        Ok(())
    }

    #[test]
    fn prefers_executable_closest_to_matching_folder() -> eyre::Result<()> {
        let test_directory = TestDirectory::new()?;
        let closest = test_directory.file("A Game/A Game.exe")?;
        test_directory.file("A Game/bin/A_Game.exe")?;

        let games = scan_directories(&[test_directory.configured_path()])?;

        assert_eq!(games.first().map(|game| &game.executable), Some(&closest));
        Ok(())
    }

    #[test]
    fn deduplicates_overlapping_roots() -> eyre::Result<()> {
        let test_directory = TestDirectory::new()?;
        let game_root = test_directory.directory("Duplicate Game")?;
        test_directory.file("Duplicate Game/DuplicateGame.exe")?;

        let games = scan_directories(&[
            test_directory.configured_path(),
            game_root.to_string_lossy().to_string(),
        ])?;

        assert_eq!(games.len(), 1);
        Ok(())
    }

    #[test]
    fn continues_when_another_root_is_missing() -> eyre::Result<()> {
        let test_directory = TestDirectory::new()?;
        test_directory.file("Valid Game/ValidGame.exe")?;
        let missing = test_directory.path.join("missing");

        let games = scan_directories(&[
            missing.to_string_lossy().to_string(),
            test_directory.configured_path(),
        ])?;

        assert_eq!(games.len(), 1);
        Ok(())
    }

    #[test]
    fn errors_for_no_configured_roots() {
        let result = scan_directories(&[]);

        assert!(result.is_err());
    }

    #[test]
    fn errors_when_all_roots_are_invalid() -> eyre::Result<()> {
        let test_directory = TestDirectory::new()?;
        let missing = test_directory.path.join("missing");

        let result = scan_directories(&[missing.to_string_lossy().to_string()]);

        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn valid_empty_root_returns_empty_games() -> eyre::Result<()> {
        let test_directory = TestDirectory::new()?;

        let games = scan_directories(&[test_directory.configured_path()])?;

        assert!(games.is_empty());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_directory_symlinks() -> eyre::Result<()> {
        use std::os::unix::fs::symlink;

        let test_directory = TestDirectory::new()?;
        let outside = TestDirectory::new()?;
        outside.file("Linked Game/LinkedGame.exe")?;
        symlink(&outside.path, test_directory.path.join("linked"))?;

        let games = scan_directories(&[test_directory.configured_path()])?;

        assert!(games.is_empty());
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn does_not_follow_directory_symlinks_when_creation_is_permitted() -> eyre::Result<()> {
        use std::os::windows::fs::symlink_dir;

        let test_directory = TestDirectory::new()?;
        let outside = TestDirectory::new()?;
        outside.file("Linked Game/LinkedGame.exe")?;
        let link_result = symlink_dir(&outside.path, test_directory.path.join("linked"));
        if link_result.is_err() {
            return Ok(());
        }

        let games = scan_directories(&[test_directory.configured_path()])?;

        assert!(games.is_empty());
        Ok(())
    }

    #[test]
    fn lexical_path_breaks_equal_distance_ties() -> eyre::Result<()> {
        let test_directory = TestDirectory::new()?;
        let alphabetically_first = test_directory.file("Tie Game/a/TieGame.exe")?;
        test_directory.file("Tie Game/b/TieGame.exe")?;

        let games = scan_directories(&[test_directory.configured_path()])?;

        assert_eq!(
            games.first().map(|game| &game.executable),
            Some(&alphabetically_first)
        );
        Ok(())
    }

    #[test]
    fn normalizer_removes_non_alphanumeric_characters() {
        assert_eq!(normalize_name("My_Game-2"), "mygame2");
    }

    #[test]
    fn executable_match_does_not_escape_configured_root() -> eyre::Result<()> {
        let test_directory = TestDirectory::new()?;
        let game_root = test_directory.directory("Outer Game/scan-root")?;
        test_directory.file("Outer Game/scan-root/bin/OuterGame.exe")?;

        let games = scan_directories(&[game_root.to_string_lossy().to_string()])?;

        assert!(games.is_empty());
        Ok(())
    }

    #[test]
    fn a_file_path_is_not_a_valid_root() -> eyre::Result<()> {
        let test_directory = TestDirectory::new()?;
        let file = test_directory.file("not-a-directory")?;

        let result = scan_directories(&[file.to_string_lossy().to_string()]);

        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn helper_accepts_only_exe_extension() {
        assert!(has_exe_extension(Path::new("game.EXE")));
        assert!(!has_exe_extension(Path::new("game.com")));
    }
}
