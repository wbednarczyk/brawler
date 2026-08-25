import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    // The suite's timezone is pinned to Europe/Warsaw in the package.json
    // vitest scripts (spawn-time env — the only reliable point: ICU fixes the
    // zone per process, so setting TZ from config/setup files is a no-op in
    // workers). Local-day behavior (Dziś v2 bucketing, #422) is untestable
    // under UTC — a UTC runner can never observe a local-midnight split. Run
    // vitest via `npm test`, not bare `npx vitest`, or the machine TZ leaks in.
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
    // Must stay comfortably above Testing Library's `asyncUtilTimeout` (5s, set
    // in src/test/setup.ts): when the per-test budget equals the per-wait budget,
    // a wait that uses its full allowance dies as a bare "timed out" instead of
    // the assertion message that says what actually differed.
    testTimeout: 15_000,
    // Measured 2026-07-29 (#51, full `vitest run src`, 1013 tests, warm cache):
    // wall ~18s; aggregate import ~100s / transform ~40s / environment ~85s
    // across workers. Cheap levers were tried and rejected: prebundling
    // lucide-react via deps.optimizer moved import 103s→88s once, then
    // regressed to noise (wall unchanged); `--no-isolate` broke 134 tests
    // (suites rely on fresh module state) with no wall win. The cost is the
    // app's own module graph × jsdom per file — structural, not trimmable by
    // config. Focused runs (`npm test -- -t`, single files) stay ~2-3s, which
    // is the actual inner loop.
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
