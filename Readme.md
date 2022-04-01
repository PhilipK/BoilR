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
- [x] [Heroic Launcher](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher) (Only Linux & Epic Games for now)
- [ ] XBox/Microsoft Store integration



## Getting started

- Download the latest release from the [releases page](https://github.com/PhilipK/BoilR/releases).
- Run the executable.
- Click Synchronize.
- Restart Steam to see your new shortcuts.


## Getting cover art for your shortcuts

- Get a [SteamGridDB API key](https://www.steamgriddb.com/profile/preferences/api)
- Run BoilR put the auth key in the input.
- Click Save
- Click Syncrhonize


## Tips for steam deck

I currently don't have a steam deck, which slows down development for it a bit.
There might still be problems specific to the deck, so if you run into one please check the issues.

If you have a problem that a game wont launch, try to manually set a proton version for it.

## Tips for linux

If you are running linux and are running into problems check [tips for linux seciton](tips_for_linux.md)

## Configuration

Most people will not have to configure anything, just open BoilR and click Synchronize, but it is possible to configure a lot, see the [configuration section](configuration.md)


## What is up with the name BoilR?

This tool turns things into Steam, therefor boiler, And it is written in **R**ust so therefor: BoilR

## License

This project is dual license MIT or Apache 2.0 , it is up to you. In short, you can do what you want with this project, but if in doubt read the license files.
