// Node built-in test runner (`node:test`) for the live-drive-hints classifier
// (card #337, epic #335, ADR 0096 principle 5). Exercises `classify` directly —
// no child processes, no git — the CLI plumbing (env vars, spawnSync, exit
// code) is covered by the manual VERIFY steps in the card, not here.

import { test } from "node:test";
import assert from "node:assert/strict";

import { classify } from "./live-drive-hints.mjs";

const MIGRATION_HINT =
  "Migrations changed — a PR binary applies them to the live database on first launch; drive against the real DB deliberately (backup first).";
const COMMAND_HINT = "IPC/MCP surface changed — drive the affected command flow on the real app.";
const JOBS_HINT = "Background jobs changed — watch a real sweep/autopilot cycle live.";
const SOURCE_ADAPTER_HINT = "Source adapters changed — verify against the real sources live.";
const STORAGE_HINT = "Storage/read models changed — check the affected panels over the real DB.";
const UI_HINT =
  "UI changed — a scoped live-drive (make pr-live-cycle PR=n LIVE_SPEC=<spec>) of the affected panel is worth it.";

// ---- 1. migrations.rs maps to the migration hint, not the storage hint ----

test("migrations.rs under storage/ wins the migration hint, not the storage hint", () => {
  const hints = classify(["src-tauri/src/storage/migrations.rs"]);
  assert.deepEqual(hints, [MIGRATION_HINT]);
});

// ---- 2. dedup: two command files -> one hint ------------------------------

test("two files under commands/ collapse to one hint", () => {
  const hints = classify([
    "src-tauri/src/commands/watchlist.rs",
    "src-tauri/src/commands/notebook.rs",
  ]);
  assert.deepEqual(hints, [COMMAND_HINT]);
});

// ---- 3. generated/test UI paths produce no UI hint; a real panel does -----

test("generated API bindings and test helpers are exempt from the UI hint", () => {
  const hints = classify(["src/api/generated/Foo.ts", "src/test/x.ts"]);
  assert.deepEqual(hints, []);
});

test("a real panel source file triggers the UI hint", () => {
  const hints = classify(["src/panels/Foo.tsx"]);
  assert.deepEqual(hints, [UI_HINT]);
});

// ---- 4. empty list -> [] ----------------------------------------------

test("no changed files yields no hints", () => {
  assert.deepEqual(classify([]), []);
});

// ---- 5. mixed set preserves rule order, not file order --------------------

test("a mixed changeset orders hints by rule order, independent of file order", () => {
  const hints = classify([
    "src/panels/Foo.tsx",
    "src-tauri/src/storage/read_models.rs",
    "src-tauri/src/source_adapters/gpw.rs",
    "src-tauri/src/jobs/sweep.rs",
    "src-tauri/src/commands/watchlist.rs",
    "src-tauri/migrations/0100_x.sql",
  ]);
  assert.deepEqual(hints, [
    MIGRATION_HINT,
    COMMAND_HINT,
    JOBS_HINT,
    SOURCE_ADAPTER_HINT,
    STORAGE_HINT,
    UI_HINT,
  ]);
});

// ---- 6. docs/wiki-only changes yield no hints ------------------------------

test("docs-only and wiki-only changes yield no hints", () => {
  assert.deepEqual(classify(["docs/x.md", "wiki/y.md"]), []);
});
