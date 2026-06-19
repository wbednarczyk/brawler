import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    // `threads` trims per-worker process-spawn overhead vs `forks` (small but
    // real; tests are pure jsdom + React with per-test state reset, so threads
    // are safe). NOTE: the suite's wall-clock is dominated by module-graph
    // import/transform, not the pool — the deeper lever is trimming the heavy
    // shared harness imports (workflowHarness testData/commands), tracked as a
    // follow-up (ADR 0048 loop-speed).
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
