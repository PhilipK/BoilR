# BoilR

## Description

This little tool will show games from other games platforms in your Steam library.
It uses the Steam 3rd party shortcuts feature and does not require you to set up anything.
The goal is that you do not have to leave your Steam library to launch games from other launchers/stores, so that you can find all the games that you have available.
Optionally you can set BoilR up to automatically download artwork from [SteamGridDB](https://www.steamgriddb.com/).

## Features

- [x] Show games from other platforms in your steam library
- [x] Automatically download art from [SteamGridDB](https://www.steamgriddb.com/)
- [x] Cross Platform (Windows, Linux, Mac, Steam Deck)
- [x] Standalone / No install needed
- [x] Small (~1.5mb on disk)
- [x] Lightweight (~2mb ram)
- [x] Fast synchronization (~1 second)

## Integrations

- [x] [Epic Games Store](https://www.epicgames.com/)
- [x] [Itch.io](https://itch.io/app)
- [x] [Origin](https://www.origin.com)
- [x] [GOG](https://www.gog.com/galaxy)
- [x] [UPlay](https://ubisoftconnect.com)
- [x] [Lutris](https://github.com/lutris/lutris)
- [x] [Legendary](https://github.com/derrod/legendary)
- [x] [Heroic Launcher](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher) (Only Epic Games for now)
- [ ] XBox/Microsoft Store integration


## Beta

This tool is still in beta, there are still a few things to do, but the things that are there work.
Feel free to submit issues and pull requests.

## Getting started

- Download the latest release from the [releases page](https://github.com/PhilipK/BoilR/releases).
- Run the executable.
- Restart Steam to see your new shortcuts.

## Getting cover art for your shortcuts

- Get a [SteamGridDB API key](https://www.steamgriddb.com/profile/preferences/api)
- Write it in the `config.toml` file (see how in the [config section](#configuration)).
- If you are using the  Ui version just put it into the auth key input.
- Run the executable again

## Configuration

The tool reads its configuration from a `config.toml` file. If it cannot find one, it will create a default one on its first run.
You can either manually edit this file or use the UI version to changing these configurations.

Here is a simple example of how to write the config file:

```toml
[steamgrid_db]
auth_key="Write your authentication key between these quotes"
```

And here is a full example of all configuration options:

```toml

[epic_games]
enabled=true #On windows this is default true, on linux default false
location="C:\\ProgramData\\Epic\\EpicGamesLauncher\\Data\\Manifests" #If this value is not defined, the tool will try to find it automatically (only windows). If it can't find it, it will fail and tell you.
create_symlinks = true #Only for Linux, To get around a bug in steam where paths can not contain spaces, BoilR creates symlinks in ~/.boilr/link and uses those. 

[legendary]
enabled=true
executable="legendary" #The name of the "legendary" executable will be used, it is assumed to be on the path.

[lutris]
enabled=true
executable="lutris" #The executable to run for lutris, default is "lutris".

[itch]
enabled=true
location="C:\\Users\\user\\AppData\\Roaming\\itch" #If this value is not defined, "%APPDATA%itch" will be used on windows, and HOME/.config/itch on linux.
create_symlinks = true #Only for Linux, To get around a bug in steam where paths can not contain spaces, BoilR creates symlinks in ~/.boilr/link and uses those. 

[origin]
enabled=true
location="C:\\ProgramData\\Origin" #If this value is not defined, "%PROGRAMGDATA%origin" will be used on windows, and HOME/Games/origin/drive_c/ProgramData/Origin/ on linux.

[gog]
enabled=true
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

If you are on Linux, and want to use one of the launchers that is not available natively, I suggest you use Lutris and make sure that the BoilR integration for Lutris is enabled (it is by default, see config section). If you want to avoid launching into Lutris, here are a few ways that you can do that.

### GOG

- Install [Lutris](https://lutris.net/)
- Install GOG from Lutris [here](https://lutris.net/games/gog-galaxy/)
- Set the path to GOG in the config.toml (or through the ui)
- Run BoilR
- Remember to pick which version of Proton you want to use for a game, before the first time you play it.

### Epic

I recommend you just use [Legendary](https://github.com/derrod/legendary). But if you really really want to use EGS you can:

- Install [Lutris](https://lutris.net/)
- Install EGS from Lutris [here](https://lutris.net/games/epic-games-store/)
- Set the path to EGS in the config.toml (or through the ui)
- Run BoilR
- Remember to pick which version of Proton you want to use for a game, before the first time you play it.

### Origin

Here I just suggest you use [Lutris](https://lutris.net/games/origin/)

## What is up with the name BoilR?

This tool turns things into Steam, therefor boiler, And it is written in **R**ust so therefor: BoilR

## License

This project is dual license MIT or Apache 2.0 , it is up to you. In short, you can do what you want with this project, but if in doubt read the license files.
