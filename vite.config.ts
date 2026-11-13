import { fileURLToPath, URL } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// Tauri expects a fixed dev port and must see Rust compiler output.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    // The two WebViews the app ships in: WebView2 on Windows (evergreen Chromium) and WKWebView
    // on macOS, which is the system Safari engine (16.4 on the oldest supported macOS, 13.3:
    // LSMinimumSystemVersion). Lightning CSS (CSS minification, cssTarget follows target) adds
    // the -webkit- prefixes Safari still needs (backdrop-filter, user-select, …).
    target: ["chrome120", "safari16.4"],
    sourcemap: false,
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
    setupFiles: ["src/test/setup.ts"],
  },
});
