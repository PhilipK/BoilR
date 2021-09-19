# Steam Shortcuts Sync

## Description

This little tool will syncrhonize games from other store platforms into your Steam library.

The goal is that you do not have to leave your Steam library to launch games from other launchers/stores.

## Features

* Add shortcuts to games from [Epic Games Store](https://www.epicgames.com/) (Windows)
* Add shortcuts to games from [Legendary](https://github.com/derrod/legendary) (Linux)
* Download custom art for games from [SteamGridDB](https://www.steamgriddb.com/) for any custom steam shortcut.

## Very early alpha

This tool is still in very early alpha, there are still lots of things to do.
Currently it will only work if you have steam installed at the default location, and there are no options for configuration, or for scheduling recuring synchronizations.
Also only Epic Games and Legendary are supported at the moment.

It is a Minimal Viable Product currently, I will update it with new features and bug fixes as I get time.

Feel free to submit issues and pull requests

## Getting started

* Download the latest release from the [releases page](https://github.com/PhilipK/steam_shortcuts_sync/releases).
* Place it in a folder of your choice.
* Create a file called `auth_key.txt` in the same folder as the executable.
* Write your [SteamGridDB API key](https://www.steamgriddb.com/profile/preferences/api) in the `auth_key.txt` file.
* Run the executable!