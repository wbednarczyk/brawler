import "@testing-library/jest-dom/vitest";
import { configure } from "@testing-library/react";
import { beforeEach, vi } from "vitest";

// Per-test isolation for browser storage (ADR 0048 decision 2 — "no cross-test
// bleed" — which the runtime reset honored but `localStorage` did not). jsdom
// keeps one `localStorage` for the whole FILE, so any persisted UI state
// (e.g. a stored pane width) would otherwise bleed across tests.
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

// Several density-tier hooks (QualityPanel, TodayScreen, EventsScreen — ADR
// 0076 D6) construct a ResizeObserver at mount; jsdom does not implement it.
// This minimal stub lets those views render in the jsdom harness.
if (typeof globalThis.ResizeObserver === "undefined") {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}
