// Covers the nextest-config-location half of gate-integrity's checks
// (scripts/check/nextest-config-location.mjs). gate-integrity.mjs itself is a
// top-level script with side effects against the real repo (no `main()`
// guard like its siblings docs-drift.mjs/retired-surface.mjs), so importing
// it would run its full check suite against whatever real repo state happens
// to exist — this file instead unit-tests the extracted pure check against
// fixture trees, the same pattern retired-surface.test.mjs uses for `scan`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { checkNextestConfigLocation } from "./nextest-config-location.mjs";

function makeTree(files) {
  const root = mkdtempSync(join(tmpdir(), "gate-integrity-"));
  for (const [rel, content] of Object.entries(files)) {
    const full = join(root, rel);
    mkdirSync(join(full, ".."), { recursive: true });
    writeFileSync(full, content);
  }
  return root;
}

test("flags a repo-root .config/nextest.toml — nextest never reads it there", () => {
  const root = makeTree({
    ".config/nextest.toml": "[test-groups]\nloopback-sockets = { max-threads = 1 }\n",
    "src-tauri/.config/nextest.toml": "[test-groups]\nloopback-sockets = { max-threads = 1 }\n",
  });
  try {
    const errors = checkNextestConfigLocation(root);
    assert.ok(
      errors.some((e) => e.includes("repo root")),
      errors.join("\n"),
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("flags a src-tauri config missing the loopback-sockets group", () => {
  const root = makeTree({
    "src-tauri/.config/nextest.toml": "[profile.mutants]\ntest-threads = 2\n",
  });
  try {
    const errors = checkNextestConfigLocation(root);
    assert.ok(
      errors.some((e) => e.includes("loopback-sockets")),
      errors.join("\n"),
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("flags a missing src-tauri/.config/nextest.toml entirely", () => {
  const root = makeTree({ "README.md": "x" });
  try {
    const errors = checkNextestConfigLocation(root);
    assert.ok(
      errors.some((e) => e.includes("not found")),
      errors.join("\n"),
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("passes when the config lives only at src-tauri/.config/nextest.toml with the group", () => {
  const root = makeTree({
    "src-tauri/.config/nextest.toml": "[test-groups]\nloopback-sockets = { max-threads = 1 }\n",
  });
  try {
    assert.deepEqual(checkNextestConfigLocation(root), []);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
