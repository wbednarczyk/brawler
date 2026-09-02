// Node built-in test runner (`node:test`) for the pinned-renderer predicate
// (#448): baselines must only be generated/compared inside the official
// Playwright docker image pinned to the installed @playwright/test version.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { pinnedImage, pinnedRenderer } from "./pinned-renderer.mjs";

function withTempDir(fn) {
  const dir = mkdtempSync(join(tmpdir(), "pinned-renderer-test-"));
  try {
    return fn(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("pinnedImage renders the exact docker image tag for a version", () => {
  assert.equal(pinnedImage("1.62.0"), "mcr.microsoft.com/playwright:v1.62.0-noble");
});

test("pinnedRenderer returns true for a matching info file", () => {
  withTempDir((dir) => {
    const infoPath = join(dir, ".docker-info");
    writeFileSync(
      infoPath,
      JSON.stringify({ driverVersion: "1.62.0", dockerImageName: "mcr.microsoft.com/playwright:v1.62.0-noble" }),
    );
    assert.equal(pinnedRenderer({ infoPath, version: "1.62.0" }), true);
  });
});

test("pinnedRenderer returns false when the image name doesn't match the pin", () => {
  withTempDir((dir) => {
    const infoPath = join(dir, ".docker-info");
    writeFileSync(
      infoPath,
      JSON.stringify({ driverVersion: "1.61.0", dockerImageName: "mcr.microsoft.com/playwright:v1.61.0-noble" }),
    );
    assert.equal(pinnedRenderer({ infoPath, version: "1.62.0" }), false);
  });
});

test("pinnedRenderer returns false when the info file is missing", () => {
  withTempDir((dir) => {
    const infoPath = join(dir, "does-not-exist.json");
    assert.equal(pinnedRenderer({ infoPath, version: "1.62.0" }), false);
  });
});

test("pinnedRenderer returns false for malformed JSON", () => {
  withTempDir((dir) => {
    const infoPath = join(dir, ".docker-info");
    writeFileSync(infoPath, "{not json");
    assert.equal(pinnedRenderer({ infoPath, version: "1.62.0" }), false);
  });
});

// The guard must refuse before spawning Playwright anywhere but the pinned
// renderer; this host has no /ms-playwright/.docker-info.
test("visual-update-guard refuses to run outside the pinned renderer", () => {
  const before = existsSync("test-results") ? readdirSync("test-results") : null;
  const result = spawnSync(process.execPath, ["scripts/ux/visual-update-guard.mjs"], {
    env: { ...process.env, SCREEN: "basic-info", REASON: "test" },
    encoding: "utf8",
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /pinned renderer/);
  const after = existsSync("test-results") ? readdirSync("test-results") : null;
  assert.deepEqual(after, before);
});
