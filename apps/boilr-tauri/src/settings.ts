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
  /** Find artwork by store id (Epic, EA, Battle.net) before searching by name. */
  match_by_store_id?: boolean;
};

export type MultiStore = "preferred" | "label_store" | "all";
export type NameLabel = "none" | "not_installed" | "store" | "store_and_not_installed";

/** `[owned_games]`: how games an account owns but that are not installed show up in Steam. */
export type OwnedGamesSettings = {
  skip_games_in_steam?: boolean;
  skip_installed_elsewhere?: boolean;
  multi_store?: MultiStore;
  /** Store keys, most preferred first. */
  store_order?: string[];
  name_label?: NameLabel;
  /** Ask before an import that adds more not-installed games than this; 0 never asks. */
  confirm_above?: number;
};

export type Settings = {
  steam?: SteamSettings;
  steamgrid_db?: SteamGridDbSettings;
  owned_games?: OwnedGamesSettings;
  blacklisted_games?: number[];
  [key: string]: unknown;
};
