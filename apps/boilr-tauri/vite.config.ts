import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
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
