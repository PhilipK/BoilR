// Browser-only stand-in for the Tauri backend, so the UI can be developed and
// clicked through in a normal browser (`npm run dev`). Never loaded inside Tauri.
import { mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { PlatformSummary, ShortcutSummary, SyncPlan } from "./types";
import type { Settings } from "./settings";
import type { ArtworkGame, ArtworkKind, ArtworkOption } from "./types";

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
  steamgrid_db: { enabled: true, prefer_animated: false, allow_nsfw: false, only_download_boilr_images: false, auth_key: "sample-key" },
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


// Sample artwork: pixel-style SVGs in BoilR's palette, in each kind's real proportions.
const SIZES: Record<ArtworkKind, [number, number]> = {
  grid: [600, 900], wide_grid: [920, 430], hero: [1920, 620], logo: [640, 360], icon: [256, 256], big_picture: [920, 430],
};
const PALETTE = ["#0D2B45", "#203C56", "#544E68", "#8D697A", "#D08159", "#FFAA5E", "#FFD4A3", "#FFECD6"];
const art = (kind: ArtworkKind, title: string, seed: number): string => {
  const [w, h] = SIZES[kind];
  const bg = kind === "logo" ? "none" : PALETTE[seed % 4];
  const cells = Array.from({ length: 24 }, (_, i) => {
    const x = ((i * 37 + seed * 11) % 20) * (w / 20);
    const y = ((i * 53 + seed * 7) % 12) * (h / 12);
    return kind === "logo" ? "" : `<rect x="${x}" y="${y}" width="${w / 20}" height="${h / 12}" fill="${PALETTE[3 + ((i + seed) % 5)]}" opacity="0.5"/>`;
  }).join("");
  const size = Math.round(Math.min(w, h) / (kind === "icon" ? 3 : 7));
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" shape-rendering="crispEdges"><rect width="${w}" height="${h}" fill="${bg}"/>${cells}<text x="50%" y="${kind === "grid" ? "80%" : "55%"}" font-family="monospace" font-weight="bold" font-size="${size}" fill="#FFECD6" text-anchor="middle">${kind === "icon" ? title.slice(0, 2).toUpperCase() : title}</text></svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
};
let artworkGames: ArtworkGame[] = [
  ["Outer Wilds", ["grid", "hero", "logo", "icon", "wide_grid"]],
  ["Hades", ["grid", "hero", "logo"]],
  ["Death Stranding", ["grid"]],
  ["Hyper Light Drifter", ["grid", "hero", "logo", "icon"]],
  ["Kena: Bridge of Spirits", []],
  ["Disco Elysium", ["grid", "wide_grid"]],
  ["Celeste", ["grid", "hero", "logo", "icon"]],
  ["Hollow Knight", []],
  ["OrcaSlicer", []],
].map(([name, kinds], i) => ({
  app_id: 3400000000 + i,
  name: name as string,
  from_boilr: true,
  images: Object.fromEntries((kinds as ArtworkKind[]).map((k) => [k, { path: art(k, name as string, i), version: 1 }])),
  never_download: [],
}));
const artworkOptions = (name: string, kind: ArtworkKind): ArtworkOption[] =>
  Array.from({ length: kind === "icon" ? 6 : 9 }, (_, i) => ({
    id: 900 + i,
    thumb: art(kind, name, i + 3),
    url: art(kind, name, i + 3),
    extension: "png",
    width: SIZES[kind][0],
    height: SIZES[kind][1],
    author: ["pixelfox", "grid_wizard", "steamy", "retrobox"][i % 4],
  }));

// With the dev server's SteamGridDB proxy (BOILR_SGDB_KEY_FILE), use real artwork instead.
type SgdbImage = { id: number; url: string; thumb: string; width: number; height: number; mime: string; author?: { name?: string } };
const sgdb = async <T,>(path: string): Promise<T> => {
  const response = await fetch(`/sgdb${path}`);
  const body = await response.json();
  if (!body.success) throw new Error((body.errors ?? ["SteamGridDB request failed"]).join(", "));
  return body.data as T;
};
const sgdbIds = new Map<string, number | null>();
const sgdbGameId = async (name: string): Promise<number | null> => {
  if (!sgdbIds.has(name)) {
    const found = await sgdb<{ id: number }[]>(`/search/autocomplete/${encodeURIComponent(name)}`);
    sgdbIds.set(name, found[0]?.id ?? null);
  }
  return sgdbIds.get(name) ?? null;
};
const WIDE = "?dimensions=920x430,460x215";
const SGDB_PATH: Record<ArtworkKind, string> = {
  grid: "/grids/game/{id}?dimensions=600x900,342x482,660x930",
  wide_grid: `/grids/game/{id}${WIDE}`,
  big_picture: `/grids/game/{id}${WIDE}`,
  hero: "/heroes/game/{id}",
  logo: "/logos/game/{id}",
  icon: "/icons/game/{id}",
};
const extensionOf = (mime: string) => (mime.includes("jpeg") ? "jpg" : mime.includes("webp") ? "webp" : mime.includes("icon") ? "ico" : "png");
const realOptions = async (name: string, kind: ArtworkKind): Promise<ArtworkOption[]> => {
  const id = await sgdbGameId(name);
  if (!id) return [];
  const images = await sgdb<SgdbImage[]>(SGDB_PATH[kind].replace("{id}", String(id)));
  return images.slice(0, 24).map((i) => ({
    id: i.id, thumb: i.thumb, url: i.url, extension: extensionOf(i.mime), width: i.width, height: i.height, author: i.author?.name ?? "unknown",
  }));
};
let realArtLoaded = false;
const loadRealArt = async () => {
  if (realArtLoaded) return;
  realArtLoaded = true;
  artworkGames = await Promise.all(
    artworkGames.map(async (g) => {
      const images: ArtworkGame["images"] = {};
      for (const kind of Object.keys(g.images) as ArtworkKind[]) {
        const first = (await realOptions(g.name, kind).catch(() => []))[0];
        if (first) images[kind] = { path: first.thumb, version: 1 };
      }
      return { ...g, images };
    })
  );
};

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
      case "rename_game":
        platforms = platforms.map((p) => ({
          ...p,
          games: p.games.map((g) => (g.app_id === a.appId ? { ...g, display_name: a.name.trim() || g.app_name } : g)),
        }));
        return null;
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
      case "list_steam_accounts":
        return [{ user_id: "12345678", shortcut_count: artworkGames.length }];
      case "list_artwork":
        if (__SGDB_PROXY__) await loadRealArt();
        return artworkGames;
      case "artwork_options":
        await delay(700);
        if (!settings.steamgrid_db?.auth_key) return Promise.reject("Add a SteamGridDB key in Settings first.");
        return __SGDB_PROXY__ ? realOptions(a.name, a.kindName) : artworkOptions(a.name, a.kindName);
      case "set_artwork": {
        await delay(900);
        const image = { path: a.url, version: Date.now() };
        artworkGames = artworkGames.map((g) => (g.app_id === a.appId ? { ...g, images: { ...g.images, [a.kindName]: image } } : g));
        return image;
      }
      case "clear_artwork":
        artworkGames = artworkGames.map((g) => {
          if (g.app_id !== a.appId) return g;
          const images = { ...g.images };
          delete images[a.kindName as ArtworkKind];
          const never = g.never_download.filter((k) => k !== a.kindName);
          return { ...g, images, never_download: a.neverDownload ? [...never, a.kindName] : never };
        });
        return settings;
      case "artwork_game_match":
        if (__SGDB_PROXY__) {
          const found = await sgdb<{ id: number; name: string; release_date?: number }[]>(
            `/search/autocomplete/${encodeURIComponent(a.query ?? a.name)}`
          );
          return {
            current_id: await sgdbGameId(a.name),
            candidates: found.slice(0, 8).map((f) => ({
              id: f.id, name: f.name, year: f.release_date ? new Date(f.release_date * 1000).getFullYear() : null,
            })),
          };
        }
        await delay(500);
        return {
          current_id: 1001,
          candidates: [
            { id: 1001, name: a.query ?? a.name, year: 2019 },
            { id: 1002, name: `${a.query ?? a.name}: Echoes of the Eye`, year: 2021 },
            { id: 1003, name: `${a.query ?? a.name} (Demo)`, year: 2018 },
          ],
        };
      case "set_artwork_game":
        sgdbIds.set(artworkGames.find((g) => g.app_id === a.appId)?.name ?? "", a.gridId);
        return null;
      case "find_missing_artwork":
        await delay(1500);
        artworkGames = artworkGames.map((g, i) =>
          g.images.grid ? g : { ...g, images: { ...g.images, grid: { path: art("grid", g.name, i + 5), version: Date.now() } } }
        );
        return null;
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
