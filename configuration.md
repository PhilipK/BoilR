# Configuration

The tool reads its configuration from a `config.toml` file. If it cannot find one, it will create a default one on its first run.
You can either manually edit this file or use the UI version to change the configurations.
Most people will not need to change the configuration and BoilR can find different launchers without problems.

Here is a simple example of how to write the config file:

```toml
[steamgrid_db]
auth_key="Write your authentication key between these quotes"
```

And here is a full example of all configuration options:

```toml

[epic_games]
enabled=true #On Windows this is default true, on Linux default false
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
location="C:\\ProgramData\\Origin" #If this value is not defined, "%PROGRAMDATA%\\Origin" will be used on Windows, and HOME/Games/origin/drive_c/ProgramData/Origin/ on Linux.

[gog]
enabled=true
location="C:\\ProgramData\\GOG.com\\Galaxy" #The location of GOG Galaxy will default to this value if not defined on windows and "~/Games/gog-galaxy/drive_c/ProgramData/GOG.com/Galaxy" on linux.
create_symlinks = true #Only for Linux, To get around a bug in steam where paths can not contain spaces, BoilR creates symlinks in ~/.boilr/link and uses those. 
wine_c_drive="/home/username/Games/gog-galaxy/drive_c" #Only for Linux, Is mandatory on linux. 

[steam]
location="C:\\Program Files (x86)\\Steam\\" #If this value is not defined, the tool will try to find it automatically. If it can't find it, it will fail and tell you.
optimize_for_big_picture=false #Set icons to wide images that Big Picture mode will use. This will make the icons have a wrong ratio in desktop mode but will improve the look in Big Picture mode
create_collections=false #Will try to create a Steam collection for each platform

[steamgrid_db]
enabled = true #If false, the whole download of custom art will be skipped.
auth_key="<your steamgrid db auth key>" #This value is mandatory if you have steamgrid_db enabled.
prefer_animated = false #If true, animated images will be prefered over static images when downloading art.
```


## No VSync
BoilR runs with VSync enabled to limit its resource use.
This can be a problem for some setups that run Linux, Wayland, and Nvidia (but not all).
If BoilR just crashes when you start it, try to add `--no-vsync` as an argument when you launch BoilR.
