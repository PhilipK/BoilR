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
  | { state: "done" };

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
