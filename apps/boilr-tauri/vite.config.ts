import { execSync } from "node:child_process";
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
const dirty = git("status --porcelain --untracked-files=no") ? "+" : "";

export default defineConfig({
  plugins: [react()],
  define: {
    __BUILD_COMMIT__: JSON.stringify(commit + dirty),
    __BUILD_TIME__: JSON.stringify(new Date().toISOString()),
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
    host: "127.0.0.1"
  }
});
