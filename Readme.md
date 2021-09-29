# Steam Shortcuts Sync

## Description

This little tool will synchronize games from other platforms into your Steam library, using the Steam Shortcuts feature.
The goal is that you do not have to leave your Steam library to launch games from other launchers/stores.

## Features
- [x] Download art from [SteamGridDB](https://www.steamgriddb.com/)
- [x] [Legendary](https://github.com/derrod/legendary) integration 
- [x] [Epic Games Store](https://www.epicgames.com/) integration
- [x] [Itch.io](https://itch.io/app) integration
- [x] Cross Platform
- [x] Small (~1.5mb on disk), lightweight (~2mb ram) and fast(~1s sync)
- [ ] Origin integration
- [ ] XBox/Microsoft Store integration
- [ ] GOG integration
- [ ] [Lutris](https://github.com/lutris/lutris) integration
- [ ] UI For configuration
- [ ] Scheduling of synchronization
- [ ] Steam Deck support (should work, but need to test when I get one)

## Very early alpha

This tool is still in very early alpha, there are still lots of things to do.
Currently it is only a cli and there are no recuring synchronizations (you have to manually run it or schedule the run yourself).
Also only Epic Games, Legendary and Itch.io are supported at the moment, but many more are planned.

It is a Minimal Viable Product currently, I will update it with new features and bug fixes as I get time.

Feel free to submit issues and pull requests

## Getting started

* Download the latest release from the [releases page](https://github.com/PhilipK/steam_shortcuts_sync/releases).
* Place it in a folder of your choice.
* Create a file called `config.toml` in the same folder as the executable.
* Write your [SteamGridDB API key](https://www.steamgriddb.com/profile/preferences/api) in the `config.toml` file (see the [config section](#configuration)).
* Run the executable.
* Restart Steam to see your new shortcuts.

## Configuration
The tool reads its configuration from a `config.toml` file.
In the future there will be an UI for changing these configurations.

Here is a simple example of how to write the config file:
```toml
[steamgrid_db]
auth_key="your steamgrid db auth key"
```

And here is a full example of all configuration options:
```toml

[epic_games]
enabled=true #On windows this is default true, on linux default false
location="C:\ProgramData\Epic\EpicGamesLauncher" #If this value is not defined, the tool will try to find it automatically (only windows). If it can't find it, it will fail and tell you.

[legendary]
enabled=false #On windows this is default false, on linux default true
executable="legendary" #If this value is not defined, "legendary" will be used, it is assumed to be on the path.

[itch]
enabled=false #Default false
location="C:\Users\user\AppData\Roaming\itch" #If this value is not defined, "%APPDATA%itch" will be used on windows, and HOME/.config/itch on linux.

[steam]
location="C:\\Program Files (x86)\\Steam\\" #If this value is not defined, the tool will try to find it automatically. If it can't find it, it will fail and tell you.

[steamgrid_db]
enabled = true #If false, the whole download of custom art will be skipped.
auth_key="<your steamgrid db auth key>" #This value is mandatory if you have steamgrid_db enabled.
```