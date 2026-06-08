import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: host || false,
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**", "**/.xwin-cache/**", "**/target/**", "**/test-results/**"],
    },
  },
  test: {
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    exclude: ["**/node_modules/**", "**/dist/**", "**/src-tauri/**", "**/tests/browser/**"],
  },
  envPrefix: ["VITE_", "TAURI_"],
});
