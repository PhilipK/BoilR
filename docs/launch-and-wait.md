# Launch and wait: EA app and Ubisoft Connect games in Steam (Windows)

EA app and Ubisoft Connect games start through their launcher, and the launcher, not Steam,
starts the game. Out of the box Steam then gets the game's state wrong: EA games show as
stopped while they run, and Ubisoft games show as running until Ubisoft Connect is closed.
On Windows, BoilR's shortcuts for these games run BoilR itself in a mode that fixes this,
`--launch-and-wait` (`crates/boilr-core/src/launch_and_wait.rs`). Linux/Proton shortcuts
are unchanged.

## How Steam treats a non-Steam shortcut

Found by testing on Windows 11 with Steam's desktop client and Big Picture (November 2026,
EA app 13.822, Ubisoft Connect 175.1); Valve documents none of this.

- **StartDir must not be empty.** Steam passes it to `CreateProcess` as the working folder;
  an empty one fails before anything runs: `Failed running GameID <id>: <command> (OS Error
  123)` in `Steam\logs\gameprocess_log.txt`.
- **Playing status follows the process tree.** Steam tracks the program it starts and every
  program that one starts (`adding PID ... as a tracked process` in `gameprocess_log.txt`).
  The shortcut shows as running while any of them is alive. A program started by a process
  outside that tree, such as a launcher that was already open, is not tracked.
- **Overlay and Resume** reach only a game that Steam's tree started. A game handed to an
  already open launcher gets neither: the controller's Home button opens Big Picture, and
  "Resume game" does nothing.
- **Shortcuts are read at startup**, so Steam must be closed while `shortcuts.vdf` is
  written.

A testing pitfall: a Steam restarted from an IDE's terminal can inherit the terminal's
Windows job object, and then the EA app cannot start games ("Create Process: Access is
denied"). Users don't hit this. Restart Steam through `explorer.exe "<path to steam.exe>"`
when testing from a terminal.

## What BoilR does

The shortcut runs `"<BoilR.exe>" --launch-and-wait --game-folder "<install folder>"
--launcher "<launcher>" --link "<game link>" ...`, with StartDir set to BoilR's folder. BoilR
handles the flag before its single-instance lock or any window, then:

1. Checks whether the launcher is already running.
2. Starts the launcher with the game's link as its own child, so Steam tracks it.
3. Waits up to 10 minutes for a program from the game's folder, then stays alive until none
   has run for 5 seconds. Steam shows the game as running for exactly that long.
4. If this launch started the launcher, closes it again after the game. Otherwise Steam
   would show the game as running until the launcher closed. A launcher that was already
   open is left alone.

When the launcher was closed it starts inside Steam's tree, so the game gets the overlay and
Resume. When it was open it takes the game over outside Steam's tree: the status is right,
but there is no overlay.

What happens is logged to `launch-and-wait.log` in BoilR's config folder.

### EA app

- Launcher `EALauncher.exe`, link `origin2://game/launch?offerIds=...`. Only games found by
  their installer data (the current EA app) use this; `.mfst` games keep their shortcut.
- After the game, BoilR waits until `EADesktop.log` says the post-game cloud sync is done (at
  most 60 seconds), then asks the EA app's "EA" window to close, which quits it cleanly. Its
  programs are ended only if that fails. Steam shows Play again about 9 seconds after the
  game.

### Ubisoft Connect

- Launcher `upc.exe`, link `uplay://launch/<id>/0`.
- Ubisoft Connect logs no end of its cloud sync, so BoilR waits 5 seconds after the game
  (a Far Cry 3 save was uploaded within 2 seconds in testing), then asks it to quit.
- By default, closing Ubisoft Connect's window only hides it in the tray. Before starting it,
  BoilR therefore changes `closebehavior: CloseBehavior_Minimize` to `CloseBehavior_Close` in
  `%LOCALAPPDATA%\Ubisoft Game Launcher\settings.yaml`, and changes it back once Ubisoft
  Connect has exited. This happens only while Ubisoft Connect is closed, because it writes its
  settings back when it exits. Ubisoft Connect is ended only if it doesn't quit within
  15 seconds. Steam shows Play again about 18 seconds after the game.

## Settings

| Setting (`settings.toml` key) | Default | What it does | Trade-off |
|---|---|---|---|
| *(none)* close a launcher BoilR started | always on | Overlay whenever the launcher was closed; Steam returns to Play after the game | Closes a launcher the user didn't have open. A download started in it during the game stops. Ubisoft: a crash, or a game that never starts, can leave `closebehavior` changed (the X button then quits Ubisoft Connect instead of hiding it), and its quit doesn't wait for a cloud upload still running after the 5-second wait. |
| EA `steam_overlay` | off | Also closes an **open** EA app before the game, so the overlay always reaches it | Stops the EA app's downloads, and another EA game running at the time loses its launcher. The EA app restarts with each game (signed in, no prompt). |
| Ubisoft `steam_overlay` | off | Same for Ubisoft Connect | As for EA. An open Ubisoft Connect can only be ended, not asked to quit, because its setting can't be changed while it runs. |

The timings (5 s before a game counts as closed, 5 s for cloud saves without a log, 60 s for
a logged sync, 15 s for a launcher to close) are constants, not settings. Changing them only
trades a faster Play button against a cut-off upload, which users can't judge.

## Known limits

- The shortcut points at BoilR's own exe, and Steam derives a shortcut's app id from its
  exe and name. Moving BoilR changes the shortcuts' app ids at the next import, and the
  shortcuts don't start until that import.
- Untested: Steam Input layouts, playtime and "In game" status for friends; launchers that
  ask for a sign-in or an update first (the 10-minute limit); games that run a program from
  outside their install folder; two games from the same launcher at once.
