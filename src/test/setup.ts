import "@testing-library/jest-dom/vitest";
import { configure } from "@testing-library/react";
import { vi } from "vitest";

// Testing Library's `waitFor`/`findBy*` default budget is 1 SECOND — generous on
// a dev box, marginal on a 4-vCPU CI runner running four vitest workers over a
// full app render. That gap produced a mystery flake in CI (`CockpitScreen`'s
// pin/unpin tab swap, PR #316) that no local run could reproduce, and nothing in
// the suite overrode it. Raise it once, here: a real failure still fails — it
// just no longer races the machine. Only failing assertions pay the extra wait.
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
