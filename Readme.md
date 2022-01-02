# BoilR

## Description

This little tool will synchronize games from other platforms into your Steam library, using the Steam Shortcuts feature.
The goal is that you do not have to leave your Steam library to launch games from other launchers/stores.

## Features

- [x] Download art from [SteamGridDB](https://www.steamgriddb.com/)
- [x] [Legendary](https://github.com/derrod/legendary) integration
- [x] [Epic Games Store](https://www.epicgames.com/) integration
- [x] [Itch.io](https://itch.io/app) integration
- [x] [Origin](https://www.origin.com) integration (currently only windows, linux comming soon)
- [x] [GOG](https://www.gog.com/galaxy) integration
- [x] [UPlay](https://ubisoftconnect.com) integration (Windows only)
- [x] Cross Platform (Linux & Windows)
- [x] UI For configuration
- [x] Small (~1.5mb on disk)
- [x] Lightweight (~2mb ram)
- [x] Fast synchronization (~30ms)
- [x] Fast art download (as fast as your internet will take you)
- [ ] [Lutris](https://github.com/lutris/lutris) integration
- [ ] XBox/Microsoft Store integration
- [ ] Scheduling of synchronization
- [ ] Steam Deck support (should work, but need to test when I get one)

## Beta

This tool is still in beta, there are still lots of things to do, but the things that are there work.
Feel free to submit issues and pull requests.

## Getting started

- Download the latest release from the [releases page](https://github.com/PhilipK/BoilR/releases).
- Choose the UI version or the cli version (if in doubt pick the UI version)
- Place it in a folder of your choice.
- Run the executable.
- Restart Steam to see your new shortcuts.

## Getting art for your shortcuts

- Get a [SteamGridDB API key](https://www.steamgriddb.com/profile/preferences/api)
- For the CLI version, Write it in the `config.toml` file (see how in the [config section](#configuration)).
- for the Ui version , copy it into the auth key input.
- Run the executable again

## Configuration

The tool reads its configuration from a `config.toml` file.
You can either manually edit this file or use the UI version to changing these configurations.

Here is a simple example of how to write the config file:

```toml
[steamgrid_db]
auth_key="your steamgrid db auth key"
```

And here is a full example of all configuration options:

```toml

[epic_games]
enabled=true #On windows this is default true, on linux default false
location="C:\\ProgramData\\Epic\\EpicGamesLauncher\\Data\\Manifests" #If this value is not defined, the tool will try to find it automatically (only windows). If it can't find it, it will fail and tell you.
create_symlinks = true #Only for Linux, To get around a bug in steam where paths can not contain spaces, BoilR creates symlinks in ~/.boilr/link and uses those. 

[legendary]
enabled=false #On windows this is default false, on linux default true
executable="legendary" #If this value is not defined, "legendary" will be used, it is assumed to be on the path.

[itch]
enabled=false #Default false
location="C:\\Users\\user\\AppData\\Roaming\\itch" #If this value is not defined, "%APPDATA%itch" will be used on windows, and HOME/.config/itch on linux.
create_symlinks = true #Only for Linux, To get around a bug in steam where paths can not contain spaces, BoilR creates symlinks in ~/.boilr/link and uses those. 

[origin]
enabled=false #Default false
location="C:\\ProgramData\\Origin" #If this value is not defined, "%PROGRAMGDATA%origin" will be used on windows, and HOME/Games/origin/drive_c/ProgramData/Origin/ on linux.

[gog]
enabled=false #Default false
location="C:\\ProgramData\\GOG.com\\Galaxy" #The location of GOG Galaxy will default to this value if not defined on windows and "~/Games/gog-galaxy/drive_c/ProgramData/GOG.com/Galaxy" on linux.
create_symlinks = true #Only for Linux, To get around a bug in steam where paths can not contain spaces, BoilR creates symlinks in ~/.boilr/link and uses those. 
wine_c_drive="/home/username/Games/gog-galaxy/drive_c" #Only for Linux, Is mandatory on linux. 

[steam]
location="C:\\Program Files (x86)\\Steam\\" #If this value is not defined, the tool will try to find it automatically. If it can't find it, it will fail and tell you.

[steamgrid_db]
enabled = true #If false, the whole download of custom art will be skipped.
auth_key="<your steamgrid db auth key>" #This value is mandatory if you have steamgrid_db enabled.
prefer_animated = false #If true, animated images will be prefered over static images when downloading art.
```

## Tips for Linux

If you are on linux, and want to use one of the launchers that is not available natively, here are a few ways that you can make them work.

### GOG

- Install [Lutris](https://lutris.net/)
- Install GOG thourgh Lutris [here](https://lutris.net/games/gog-galaxy/)
- Set the path to GOG in the config.toml (or through the ui)
- Run BoilR
- Remember to pick which version of Proton you want to use for a game, before the first time you play it.

### Epic

I recommend you just use [Legendary](https://github.com/derrod/legendary). But if you really want to use EGS you can:

- Install [Lutris](https://lutris.net/)
- Install EGS thourgh Lutris [here](https://lutris.net/games/epic-games-store/)
- Set the path to EGS in the config.toml (or through the ui)
- Run BoilR
- Remember to pick which version of Proton you want to use for a game, before the first time you play it.

### Origin

Currently BoilR can't setup shortcuts to Origin because Origin uses a special link to open games. But I know of a workaround, it is just going to take a bit to make (basicly let Boiler take that url as input and let it call origin with proton settings and the url).
This section will be updated when it is ready.
Untill then you can still play origin games thourgh Lutris [here](https://lutris.net/games/origin/)


## License

This project is dual license MIT or Apache 2.0 , it is up to you. In short, you can do what you want with this project, but if in doubt read the license files.
