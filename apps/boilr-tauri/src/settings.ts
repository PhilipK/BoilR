export type SteamSettings = {
  stop_steam?: boolean;
  start_steam?: boolean;
  create_collections?: boolean;
  optimize_for_big_picture?: boolean;
  location?: string | null;
};

export type SteamGridDbSettings = {
  enabled?: boolean;
  prefer_animated?: boolean;
  allow_nsfw?: boolean;
  only_download_boilr_images?: boolean;
  auth_key?: string | null;
};

export type Settings = {
  steam?: SteamSettings;
  steamgrid_db?: SteamGridDbSettings;
  blacklisted_games?: number[];
  [key: string]: unknown;
};
