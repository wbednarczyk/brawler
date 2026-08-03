import "@testing-library/jest-dom/vitest";
import { configure } from "@testing-library/react";
import { beforeEach, vi } from "vitest";

// Per-test isolation for browser storage (ADR 0048 decision 2 — "no cross-test
// bleed" — which the runtime reset honored but `localStorage` did not).
// The cockpit persists its live dockview geometry under `cockpit.layout.v2` on
// EVERY layout change, and restores it on mount whenever the saved panel ids all
// still exist. jsdom keeps one `localStorage` for the whole FILE, so each cockpit
// test inherited whatever geometry the previous one happened to leave — and
// whether the save landed before the test ended depends on machine timing. That
// is the real reason `CockpitScreen.test.tsx`'s pin/unpin test failed twice in
// CI while passing on every local run (PRs #316, #317).
beforeEach(() => {
  window.localStorage.clear();
  window.sessionStorage.clear();
});

// Testing Library's `waitFor`/`findBy*` default budget is 1 SECOND, which is
// marginal on a loaded runner rendering the whole app. Raised once here so a
// slow-but-correct wait is not reported as a wrong result.
//
// It MUST stay well under vitest's per-test `testTimeout` (15s, vitest.config.ts).
// When the two are equal, a wait that needs its full budget kills the test with a
// bare "timed out in 5000ms" instead of the assertion's own message — which is
// exactly how PR #317's failure read after #316 set this to the then-default 5s.
configure({ asyncUtilTimeout: 5_000 });

// Tauri module mocks for the whole frontend test run. These live here (the
// configured `setupFiles` entry in vitest.config.ts) rather than in
// appWorkflowHarness: since vitest 3, a `vi.mock` call is only honored from the
// test file itself or a setup file, not from a transitively-imported helper.
// The harness imports/re-exports the mocked members and resets them per-test in
// its own `beforeEach`.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
  downloadDir: vi.fn(() => Promise.resolve("/home/test/Downloads")),
  join: vi.fn((...paths: string[]) => Promise.resolve(paths.join("/"))),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(() => Promise.resolve("/tmp/brawler-export.json")),
}));

// dockview (Companies workspace pilot, ADR 0053) constructs a ResizeObserver at
// mount; jsdom does not implement it. This minimal stub lets the docked
// workspace render in the jsdom harness. NOTE for the pilot evaluation: needing
// to polyfill a browser layout API to test the view at all is a recorded cost of
// adopting dockview (test-architecture-fit criterion).
if (typeof globalThis.ResizeObserver === "undefined") {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}
