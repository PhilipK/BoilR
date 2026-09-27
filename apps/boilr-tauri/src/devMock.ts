// Browser-only stand-in for the Tauri backend, so the UI can be developed and
// clicked through in a normal browser (`npm run dev`). Never loaded inside Tauri.
import { mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { PlatformSummary, ShortcutSummary, SyncPlan } from "./types";
import type { Settings } from "./settings";

let nextId = 3000000000;
const game = (name: string, exe = `/games/${name}`): ShortcutSummary => ({
  app_id: nextId++,
  app_name: name,
  display_name: name,
  exe,
  start_dir: "/games",
  icon: null,
  needs_proton: false,
  needs_symlinks: false,
  blacklisted: false,
});

let settings: Settings = {
  steam: { stop_steam: false, start_steam: false, create_collections: false, optimize_for_big_picture: false, location: null },
  steamgrid_db: { enabled: true, prefer_animated: false, allow_nsfw: false, only_download_boilr_images: false, auth_key: null },
  blacklisted_games: [],
};

let platforms: PlatformSummary[] = [
  { code_name: "epic_games", name: "Epic", enabled: true, error: null, games: ["Death Stranding", "Outer Wilds", "Hyper Light Drifter", "Kena: Bridge of Spirits"].map((n) => game(n)) },
  { code_name: "gog", name: "GOG", enabled: true, error: null, games: ["Blackwell Epiphany", "Disco Elysium"].map((n) => game(n)) },
  { code_name: "heroic", name: "Heroic", enabled: true, error: null, games: [] },
  { code_name: "itch", name: "Itch", enabled: true, error: "Path not found: ~/.config/itch/db/butler.db-wal", games: [] },
  { code_name: "bottles", name: "Bottles", enabled: true, error: "EOF while parsing a value at line 1 column 0", games: [] },
  { code_name: "lutris", name: "Lutris", enabled: true, error: "No such file or directory (os error 2)", games: [] },
  { code_name: "legendary", name: "Legendary", enabled: true, error: "No such file or directory (os error 2)", games: [] },
  { code_name: "flatpak", name: "Flatpak", enabled: true, error: null, games: ["Ludusavi", "Modrinth App", "OrcaSlicer", "Snapmaker_Orca", "Vinegar"].map((n) => game(n, "flatpak")) },
  { code_name: "minigalaxy", name: "MiniGalaxy", enabled: false, error: null, games: [] },
];

const importedIds = new Set<number>();
const plan = (): SyncPlan => ({
  additions: platforms
    .filter((p) => p.enabled)
    .flatMap((p) =>
      p.games
        .filter((g) => !(settings.blacklisted_games ?? []).includes(g.app_id) && !importedIds.has(g.app_id))
        .map((g) => ({ platform: p.name, platform_code: p.code_name, needs_proton: false, needs_symlinks: false, shortcut: g }))
    ),
  removals: importedIds.size ? [] : [
    {
      user_id: "12345678",
      steam_user_data_folder: "/home/deck/.steam/steam/userdata/12345678",
      reason: "legacy_boilr",
      shortcut: { app_id: 2999999999, app_name: "Hollow Knight (old)", display_name: "Hollow Knight (old)", exe: "/games/hk", start_dir: "/games", icon: null },
    },
  ],
});

const withBlacklist = (): PlatformSummary[] =>
  platforms.map((p) => ({
    ...p,
    games: p.games.map((g) => ({ ...g, blacklisted: (settings.blacklisted_games ?? []).includes(g.app_id) })),
  }));

const stamp = () => new Date().toISOString().slice(0, 19).replace(/[T:]/g, "-");
let backups = [
  { path: "/backup/12345678-2026-09-26-18-02-11-shortcuts.vdf", user_id: "12345678", taken_at: "2026-09-26 18:02:11" },
  { path: "/backup/12345678-2026-09-20-09-15-40-shortcuts.vdf", user_id: "12345678", taken_at: "2026-09-20 09:15:40" },
];
let managed = [
  { app_id: 3111111111, name: "Hades" },
  { app_id: 3222222222, name: "Celeste" },
  { app_id: 3333333333, name: "Hollow Knight" },
];

const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

mockIPC(
  async (cmd, args) => {
    await delay(150);
    const a = (args ?? {}) as Record<string, any>;
    switch (cmd) {
      case "load_settings":
        return settings;
      case "update_settings":
        settings = {
          ...settings,
          ...a.update,
          steam: { ...settings.steam, ...a.update?.steam },
          steamgrid_db: { ...settings.steamgrid_db, ...a.update?.steamgrid_db },
        };
        return settings;
      case "discover_games":
        await delay(600);
        return withBlacklist();
      case "plan_sync":
        return plan();
      case "update_platform_enabled":
        platforms = platforms.map((p) => (p.code_name === a.codeName ? { ...p, enabled: a.enabled } : p));
        return { platforms: withBlacklist(), plan: plan() };
      case "list_platform_settings":
        return platforms.map((p) => ({ code_name: p.code_name, name: p.name, settings: { enabled: p.enabled, location: null } }));
      case "update_platform_settings":
        return { code_name: a.codeName, name: a.codeName, settings: a.settings };
      case "run_full_sync": {
        const n = plan().additions.length;
        for (const payload of [
          { state: "starting" },
          { state: "found_games", games_found: n },
          { state: "finding_images" },
          { state: "downloading_images", to_download: n * 3 },
          { state: "done" },
        ]) {
          await emit("sync-progress", payload);
          await delay(1400);
        }
        // Everything imported: nothing new remains.
        plan().additions.forEach((a) => importedIds.add(a.shortcut.app_id));
        return { imported_platforms: 3, shortcuts_considered: n, steam_users_updated: 1, images_requested: false, platform_errors: [] };
      }
      case "list_backups":
        return backups;
      case "create_backup":
        backups = [{ path: `/backup/12345678-${stamp()}-shortcuts.vdf`, user_id: "12345678", taken_at: stamp().replace(/^(\d+-\d+-\d+)-(\d+)-(\d+)-(\d+)$/, "$1 $2:$3:$4") }, ...backups];
        return backups;
      case "restore_shortcuts":
        return backups;
      case "list_boilr_shortcuts":
        return managed;
      case "release_shortcut":
        managed = managed.filter((m) => m.app_id !== a.appId);
        settings = { ...settings, blacklisted_games: [...(settings.blacklisted_games ?? []), a.appId] };
        return settings;
      default:
        console.warn("[devMock] unhandled command", cmd, args);
        return null;
    }
  },
  { shouldMockEvents: true }
);
console.info("[devMock] Tauri backend mocked for browser development");
