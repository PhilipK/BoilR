# Owned games in Steam

BoilR can sign in to game stores itself and add every game an account owns to Steam,
installed or not. No Playnite or Heroic is needed. A game that is not installed gets a
shortcut that installs it. Once the game is installed, the shortcut becomes the real game and
keeps its Steam app id, so artwork, playtime and collections stay.

The approach follows Playnite's library plugins
(github.com/JosefNemec/PlayniteExtensions, `source/Libraries`, and Jeshibu's `EaLibrary`).
They talk to the same store services.

## Pieces

| Piece | Where | What it does |
|---|---|---|
| Store clients | `crates/boilr-stores` | Sign-in, session storage, owned-game lists. One module per store. No Steam, no UI. |
| Account platforms | `src/platforms/accounts` | One `GamesPlatform` per store (`epic_account`, `gog_account`, ...). They turn the cached library into shortcuts. |
| Identity and sticky app ids | `crates/boilr-core/src/sync/synchronization.rs` | Each shortcut gets `DevkitGameID = boilr:<store>:<id>`. A sync keeps the app id Steam already has for that identity. An identity can list more ids of the same game after commas (`ea:<offer>,<content id>`); two identities are the same game when any of their ids match. |
| Login window | `apps/boilr-tauri/src/accounts.rs` | A private webview per store, with no access to BoilR's commands. When the store lands on its post-login page, BoilR stops the navigation, reads the window's cookies and the URL, and finishes the sign-in. Epic's post-login page holds the login code, so that page is loaded and its text handed over first. |
| Accounts screen | `apps/boilr-tauri/src/components/AccountsView.tsx` | Sign in, sign in with your own browser (paste), refresh, sign out, turn the account on as a game source, choose what not-installed games open. |
| egui | `AccountPlatform::render_ui` | Paste sign-in for the stores that support it. Sign-in and refresh run on a background thread. EA and Battle.net need the login window, so they need the Tauri app. |

Signing in adds nothing to Steam. The account becomes a game source when the user turns it
on; the screens say how many games that adds. Signing out turns it off again.

## Stores

| Store | Sign-in | Owned list | Not installed: shortcut opens |
|---|---|---|---|
| Epic | Login window, or paste the `authorizationCode` | library-service items, then the catalog for the entries the last list does not have yet. Skips DLC and Unreal assets. | Epic Games Launcher with `apps/<ns>:<catalog>:<app>?action=launch&silent=true`, Heroic on Linux, or the store page |
| GOG | Login window, or paste the URL after login | `embed.gog.com/account/getFilteredProducts` | GOG Galaxy `goggalaxy://openGameView/<id>`, Heroic on Linux, or the store page |
| Amazon | Login window, or paste the URL after login. Registers a device, as the Amazon Games app does. | Entitlements service | Amazon Games `amazon-games://install/<id>`, Heroic on Linux, or the Prime Gaming page |
| EA | Login window only | `getPreloadedOwnedGames` GraphQL. Skips games the account only has through a linked Steam or Epic account, and trials. | EA app `origin2://game/launch/?offerIds=<content id>`, or ea.com |
| Battle.net | Login window only | `account.battle.net/api/games-and-subs` and classic games. A title BoilR has no entry for is still listed, by the name Blizzard gives it. The China region is not supported. | `Battle.net.exe --game=<internal id>`, or the download page |
| Humble | Login window, or paste the `_simpleauth_sess` cookie | Orders API: DRM-free games with a Windows or Linux download. Skips a game when its own order holds a third-party key for it, as Playnite does. A title bought in several orders keeps the id it was first listed under. | The order's download page |
| itch.io | Paste an API key | `api.itch.io/profile/owned-keys`. Skips games that only run on Android or macOS. | itch app `itch://install?game_id=<id>&launch`, or the game page |

Each store module documents its endpoints and the reference code they come from.

## Fetching a library

Listing games never waits on a store: an account platform reads the library as it was last
fetched (`stores::cached_library`). Fetching is its own step:

- The Tauri app fetches the libraries that got older than 6 hours in the background when it
  starts, all stores at once, and reads the game list again when new lists are in.
- The classic app and `boilr --no-ui` fetch an old library on the thread that loads that
  source, before they list it.
- "Refresh library" on an account fetches it now.

A fetch (`stores::refresh_if_stale`, `stores::refresh_library`):

- ends after 3 minutes at the latest;
- runs once per store at a time, across BoilR processes too: an OS file lock
  (`stores/.locks/<store>.refresh.lock`) is held for the whole fetch, because two fetches
  at once would both use, and rotate, the same refresh token. A caller that waited for
  another's fetch uses its result;
- saves renewed tokens as soon as it has them, before it lists the games;
- keeps the old list when it fails, so a network problem never removes games from Steam,
  and is not retried for 15 minutes;
- never saves a list that looks cut short. Paging that hits its page limit, a page that
  comes twice and an answer without its list are errors. An empty list never replaces a
  list with games. A list less than half as long as the last one (of 20 games or more) is
  only taken when the user asked for the refresh;
- marks a list `partial` when some games could not be looked up (an Epic catalog request
  failed), and fetches it again after 15 minutes instead of 6 hours. Such a game keeps its
  entry from the last list.

Errors have a kind (`boilr_stores::http`). Only a store that refuses the saved login (HTTP
401, an OAuth `invalid_grant`, its sign-in page where data was expected) asks the user to
sign in again. No connection, a timeout, HTTP 429 and 5xx are transient. HTTP 403 is
neither: stores answer it for bot blocks too.

## What happens on a sync

1. Installed-game platforms (Epic, Legendary, GOG, MiniGalaxy, Amazon, Heroic, itch, EA,
   and Lutris for the games of a store account connected there) report their games with
   the same identity as the store accounts. An installed EA game lists every offer id in
   its manifest. Playnite and Bottles games have no identity; Battle.net and Humble games
   have no installed-games platform, so they stay "Not installed".
2. `accounts::arrange_owned` decides which not-installed games go in, and their names
   (see below). It always drops an owned entry whose game is installed through the same
   store: they are the same game when any of their ids match.
3. The sync writes the shortcuts. A game whose identity is already in `shortcuts.vdf`, by
   any of its ids, keeps that app id, even though its target changed.
4. Owned games go into the Steam collection "Not installed" when collections are on.

## Settings

`[owned_games]` in `config.toml`, on the Accounts page of the Tauri app and in the egui
settings:

| Setting | Default | What it does |
|---|---|---|
| `skip_games_in_steam` | `true` | Leaves out a game Steam has installed itself (same title). |
| `skip_installed_elsewhere` | `true` | Leaves out a game installed from another store or launcher (same title). |
| `multi_store` | `"preferred"` | A title several accounts own: `"preferred"` keeps the entry of the first store in `store_order`; `"label_store"` keeps one per store, named "Title (GOG)"; `"all"` keeps one per store, same name. |
| `store_order` | GOG, Epic, Amazon, EA, Battle.net, Humble, itch | Store keys, most preferred first. Stores left out come after, in this default order. |
| `name_label` | `"none"` | `"not_installed"`: "Title (Not installed)"; `"store"`: "Title (GOG)"; `"store_and_not_installed"`: "Title (GOG, not installed)". Steam has no not-installed state for shortcuts, so the name is the only place to say it. |
| `confirm_above` | `100` | The Tauri app asks before an import that adds more not-installed games than this; `0` never asks. |

Titles are compared by their letters and digits only ("PREY" and "Prey" match). A label
does not change a game's app id, which comes from its identity, and is gone once the game
is installed; artwork searches use the title without it.

Per account (`[<store>_account]`): `skip_prereleases` (default `true`) leaves out betas,
demos, playtests and test servers ("Barony (Beta)", "KillingFloor2Beta"), matched on whole
words so "Demon's Tilt" stays.

`[steamgrid_db] match_by_store_id` (default `true`) looks Epic, EA and Battle.net games up
on SteamGridDB by their store id before searching by name. GOG and Amazon have no such
lookup there and are searched by name.

An owned shortcut's app id is a hash of its identity alone, not of exe and name. Owned
shortcuts share their exe (`xdg-open`, a store launcher), and one title owned in two stores
would otherwise get one app id. The exe also changes when the user installs a launcher or
picks another way to open the game, while hidden games and renames are keyed by the id. A
rename made while a game was not installed still applies once it is.

## Storage

Each store has a folder at `<config folder>/stores/<store>/`:
- `session.json` holds tokens or cookies.
- `library.json` holds the last owned list.

Both are written with mode 0600 on Unix. For EA and Battle.net the session holds only the
cookies a browser would send to the hosts BoilR calls, not every cookie of the site.
`stores/.status/<store>.json` records the last failed refresh until one works, so the
accounts screens can say what went wrong, also after a restart.

Every change to these files happens under an OS file lock per store
(`stores/.locks/<store>.lock`), because the classic app, the Tauri app and `boilr --no-ui`
can run at once. A refresh only saves when the session on disk is still the one it started
with, so signing out or in again during a refresh wins.

Signing in again keeps the cached library only when it belongs to the same account (the
same account name); another account starts without a list.

Sign-out deletes the folder. Amazon and Epic are told first: Amazon removes BoilR from the
account's devices (as nile does), Epic ends the session (as legendary does). The other
stores have no way to end a session from outside their apps; the login stays valid until
it expires.

Errors that leave `boilr-stores` have URL query strings and secret-looking JSON fields
removed, since tokens travel in URLs (GOG) and bodies. `Debug` output of sessions, cookies
and logins hides their secrets. GOG, Amazon and Epic renew the token once and retry when
the store rejects one that has not expired yet.

## Not verified yet

None of the store clients have been run against real accounts. The parsers are tested against
fixtures made from the reference code's models. These need a real account, and some need
Windows:

- **Every sign-in, end to end:**
  - Epic's captcha in the login window. It uses an ordinary browser's user agent: with the
    launcher's, as Playnite used, the sign-in stops after two-factor authentication.
  - Amazon's sign-in page in a webview.
  - Which EA and Battle.net cookies keep the session alive. A login window reports a
    cookie's domain without its leading dot, so BoilR sends every saved cookie to its
    domain and that domain's subdomains.
  - Epic's login code read from the page inside the login window. Fetching that page with
    the window's cookies is the fallback.
  - The sign-out requests to Amazon and Epic.
  - What the stores answer for a dead login. The error kinds follow the reference code.
- **Steam honours the app id in `shortcuts.vdf`:** both the kept one after the exe changes
  and the identity-based one every owned shortcut starts with, across restarts and property
  edits. Check on Windows and Linux.
- **Windows install links:**
  - Whether Epic `?action=launch` on a game that is not installed offers the install. The
    fallback is `store/library`.
  - Whether `amazon-games://install` works while the Amazon client is not running.
  - EA `origin2://` with a content id.
  - Battle.net `--game=`.
- **Matching an installed copy to its owned entry:**
  - EA: whether one of the `.mfst` ids is the account's offer id or content id. The current
    EA app writes no `.mfst`; its installed games are found by the content ids in
    `__Installer\installerdata.xml`, in the folder Windows lists for them, and open through
    `origin2://game/launch?offerIds=<content ids>`. Whether those content ids are the ones
    the account's library reports is not checked yet.
  - itch: whether butler's receipt carries the game id. Older installs without it simply
    don't match.
- **Heroic on Linux:** used only when it is signed in to the same store. It finds the game
  in its own library. From inside BoilR's Flatpak, a Heroic Flatpak is found by its data
  folder (`~/.var/app/com.heroicgameslauncher.hgl`).
- **EA's game list** is asked for by a query hash the EA website uses. When EA changes its
  website, the list fails with a message that BoilR needs an update.

## Not covered

- **Ubisoft Connect:** Playnite only reads the launcher's local cache. BoilR's Ubisoft
  platform already does that for installed games.
- **Xbox / Game Pass:** Xbox has no ownership API. Playnite lists play history.
- **Steam:** native.
- **Linux without Heroic:** Epic, GOG, Amazon, EA and Battle.net have no native client, so
  an owned game's shortcut opens the store page. BoilR does not download games itself.
