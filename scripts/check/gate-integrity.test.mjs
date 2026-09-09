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

// Mirrors the real file's shape: a declared group AND an override block that
// actually assigns tests to it.
const VALID_CONFIG =
  "[test-groups]\n" +
  "loopback-sockets = { max-threads = 1 }\n\n" +
  "[[profile.default.overrides]]\n" +
  "filter = 'test(mcp::lifecycle) | binary(=brawler-mcp-stdio)'\n" +
  "test-group = 'loopback-sockets'\n";

test("flags a repo-root .config/nextest.toml — nextest never reads it there", () => {
  const root = makeTree({
    ".config/nextest.toml": VALID_CONFIG,
    "src-tauri/.config/nextest.toml": VALID_CONFIG,
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

test("flags a commented-out loopback-sockets group", () => {
  const root = makeTree({
    "src-tauri/.config/nextest.toml":
      "[test-groups]\n" +
      "# loopback-sockets = { max-threads = 1 }\n\n" +
      "[[profile.default.overrides]]\n" +
      "filter = 'test(mcp::lifecycle)'\n" +
      "test-group = 'loopback-sockets'\n",
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

test("flags a group with no override actually assigning tests to it", () => {
  const root = makeTree({
    "src-tauri/.config/nextest.toml": "[test-groups]\nloopback-sockets = { max-threads = 1 }\n",
  });
  try {
    const errors = checkNextestConfigLocation(root);
    assert.ok(
      errors.some((e) => e.includes("nothing is actually assigned")),
      errors.join("\n"),
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("flags an override assignment with no matching group declared", () => {
  const root = makeTree({
    "src-tauri/.config/nextest.toml":
      "[[profile.default.overrides]]\n" +
      "filter = 'test(mcp::lifecycle)'\n" +
      "test-group = 'loopback-sockets'\n",
  });
  try {
    const errors = checkNextestConfigLocation(root);
    assert.ok(
      errors.some((e) => e.includes("no uncommented")),
      errors.join("\n"),
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("passes when the config lives only at src-tauri/.config/nextest.toml with the group and a qualifying override", () => {
  const root = makeTree({
    "src-tauri/.config/nextest.toml": VALID_CONFIG,
  });
  try {
    assert.deepEqual(checkNextestConfigLocation(root), []);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("accepts trailing comments on the TOML headers (TOML ignores them, so must the scan)", () => {
  const withHeaderComments = VALID_CONFIG.replace("[test-groups]", "[test-groups] # socket limits").replace(
    "[[profile.default.overrides]]",
    "[[profile.default.overrides]] # socket assignments",
  );
  assert.notEqual(withHeaderComments, VALID_CONFIG);
  const root = makeTree({
    "src-tauri/.config/nextest.toml": withHeaderComments,
  });
  try {
    assert.deepEqual(checkNextestConfigLocation(root), []);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
