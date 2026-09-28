/// <reference types="vite/client" />

/** Short git commit the frontend was built from; "+" means uncommitted changes. */
declare const __APP_VERSION__: string;
declare const __BUILD_COMMIT__: string;
/** ISO time the frontend was built. */
declare const __BUILD_TIME__: string;
/** Browser dev only: the dev server proxies SteamGridDB at /sgdb (see vite.config.ts). */
declare const __SGDB_PROXY__: boolean;
