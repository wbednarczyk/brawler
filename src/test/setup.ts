import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

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

vi.mock("@tauri-apps/plugin-fs", () => ({
  writeTextFile: vi.fn(() => Promise.resolve()),
}));
