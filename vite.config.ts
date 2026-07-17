import react from "@vitejs/plugin-react";
import path from "path";
import { defineConfig } from "vite";

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],
  build: {
    target: "es2022",
    sourcemap: false,
  },

  resolve: {
    alias: [{ find: "@", replacement: path.resolve(__dirname, "src") }],
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,

  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // 3. tell vite to ignore watching `src-tauri` and the workspace `target/`
      //    (watching the freshly-built, locked exe dies with EBUSY on Windows)
      ignored: ["**/src-tauri/**", "**/target/**"],
    },
  },

  test: {
    environment: "jsdom",
  },
}));
