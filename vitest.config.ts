import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    // Discover Vitest specs under src only — the Playwright browser specs live in
    // tests/browser/*.spec.ts and must not be collected here (a bare `vitest run`
    // would otherwise pull them in and report spurious "0 test" failures).
    include: ["src/**/*.test.{ts,tsx}"],
    // `threads` trims per-worker process-spawn overhead vs `forks` (small but
    // real; tests are pure jsdom + React with per-test state reset, so threads
    // are safe). NOTE: the suite's wall-clock is dominated by module-graph
    // import/transform, not the pool.
    pool: "threads",
    coverage: {
      provider: "v8",
      // Coverage of the app source only; tests, mocks, and generated/entry files
      // are excluded so the number reflects behavior coverage (ADR 0048).
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/**/*.test.{ts,tsx}",
        "src/test/**",
        "src/main.tsx",
        "src/gallery.tsx",
        "src/**/*.d.ts",
      ],
      reporter: ["text-summary", "json-summary"],
      reportsDirectory: "./coverage/frontend",
    },
  },
});
