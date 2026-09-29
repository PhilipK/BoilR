export type ShortcutSummary = {
  app_id: number;
  app_name: string;
  display_name: string;
  exe: string;
  start_dir: string;
  icon: string | null;
  needs_proton: boolean;
  needs_symlinks: boolean;
  blacklisted: boolean;
};

export type PlatformSummary = {
  code_name: string;
  name: string;
  enabled: boolean;
  games: ShortcutSummary[];
  error: string | null;
};

export type PlatformToggleResponse = {
  platforms: PlatformSummary[];
  plan: SyncPlan;
};

export type PlatformSettingsPayload = {
  code_name: string;
  name: string;
  settings: Record<string, unknown>;
};

export type PlannedShortcut = {
  app_id: number;
  app_name: string;
  display_name: string;
  exe: string;
  start_dir: string;
  icon: string | null;
};

export type AdditionPlan = {
  platform: string;
  platform_code: string;
  needs_proton: boolean;
  needs_symlinks: boolean;
  shortcut: PlannedShortcut;
};

export type RemovalReason = "legacy_boilr" | "duplicate_app_id";

export type RemovalShortcut = {
  app_id: number;
  app_name: string;
  display_name: string;
  exe: string;
  start_dir: string;
  icon: string | null;
};

export type RemovalPlan = {
  user_id: string;
  steam_user_data_folder: string;
  reason: RemovalReason;
  shortcut: RemovalShortcut;
};

export type SyncPlan = {
  additions: AdditionPlan[];
  removals: RemovalPlan[];
};

export type SyncProgressEvent =
  | { state: "not_started" }
  | { state: "starting" }
  | { state: "found_games"; games_found: number }
  | { state: "finding_images" }
  | { state: "downloading_images"; to_download: number }
  | { state: "done" }
  | { state: "error"; message: string };

export type SettingsUpdatePayload = {
  steam?: {
    stop_steam?: boolean;
    start_steam?: boolean;
    create_collections?: boolean;
    optimize_for_big_picture?: boolean;
    location?: string | null;
  };
  steamgrid_db?: {
    enabled?: boolean;
    prefer_animated?: boolean;
    allow_nsfw?: boolean;
    only_download_boilr_images?: boolean;
    auth_key?: string | null;
  };
  blacklisted_games?: number[];
};

export type PlatformError = {
  code_name: string;
  name: string;
  message: string;
};

export type SyncOutcome = {
  imported_platforms: number;
  shortcuts_considered: number;
  steam_users_updated: number;
  images_requested: boolean;
  platform_errors: PlatformError[];
};

export type BackupEntry = {
  path: string;
  user_id: string;
  /** UTC, "YYYY-MM-DD HH:MM:SS". */
  taken_at: string;
};

export type ManagedShortcut = {
  app_id: number;
  name: string;
};

export type ArtworkKind = "grid" | "wide_grid" | "hero" | "logo" | "icon" | "big_picture";

export type LocalImage = {
  path: string;
  /** Last modified, seconds; changes when the image is replaced. */
  version: number;
};

export type SteamAccount = {
  user_id: string;
  shortcut_count: number;
};

export type ArtworkGame = {
  app_id: number;
  name: string;
  from_boilr: boolean;
  images: Partial<Record<ArtworkKind, LocalImage>>;
  never_download: ArtworkKind[];
};

export type ArtworkOption = {
  id: number;
  thumb: string;
  url: string;
  extension: string;
  width: number;
  height: number;
  author: string;
};

export type GameCandidate = {
  id: number;
  name: string;
  year: number | null;
};

export type GameMatch = {
  current_id: number | null;
  candidates: GameCandidate[];
};
