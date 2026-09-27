import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Build stamp shown in the app, so testers can tell which code they are running.
const git = (args: string): string => {
  try {
    return execSync(`git ${args}`, { stdio: ["ignore", "pipe", "ignore"] }).toString().trim();
  } catch {
    return "";
  }
};
const commit = git("rev-parse --short HEAD") || "unknown";
const version: string = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8")).version;

// Browser development only: with BOILR_SGDB_KEY_FILE set, the dev server proxies SteamGridDB
// so the mock backend can show real artwork. The key stays in this process as a header.
const sgdbKeyFile = process.env.BOILR_SGDB_KEY_FILE;
const sgdbKey = sgdbKeyFile ? readFileSync(sgdbKeyFile, "utf8").trim() : "";
const dirty = git("status --porcelain --untracked-files=no") ? "+" : "";

export default defineConfig({
  plugins: [react()],
  define: {
    __APP_VERSION__: JSON.stringify(version),
    __BUILD_COMMIT__: JSON.stringify(commit + dirty),
    __BUILD_TIME__: JSON.stringify(new Date().toISOString()),
    __SGDB_PROXY__: JSON.stringify(Boolean(sgdbKey)),
  },
  build: {
    outDir: "dist",
    // main.tsx awaits the dev mock import at the top level.
    target: "es2022",
    sourcemap: true
  },
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
    proxy: sgdbKey
      ? {
          "/sgdb": {
            target: "https://www.steamgriddb.com/api/v2",
            changeOrigin: true,
            rewrite: (path) => path.replace(/^\/sgdb/, ""),
            headers: { Authorization: `Bearer ${sgdbKey}` },
          },
        }
      : undefined,
  }
});
