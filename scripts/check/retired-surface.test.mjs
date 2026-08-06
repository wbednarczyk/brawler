// Tests for the retired-surface gate: live docs may not reference retired
// surface; history homes and per-token allow lists are exempt.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { findHits, isScanned, scan } from "./retired-surface.mjs";

function makeTree(files) {
  const root = mkdtempSync(join(tmpdir(), "retired-surface-"));
  for (const [rel, content] of Object.entries(files)) {
    const full = join(root, rel);
    mkdirSync(join(full, ".."), { recursive: true });
    writeFileSync(full, content);
  }
  return root;
}

test("a live doc mentioning a retired token is a violation", () => {
  const root = makeTree({
    "docs/contracts.md": "Call `old_cmd` to run the analysis.\n",
  });
  try {
    const v = scan(root, { retired: [{ token: "old_cmd", adr: "ADR 0084" }] });
    assert.equal(v.length, 1);
    assert.deepEqual(
      { file: v[0].file, line: v[0].line, token: v[0].token },
      { file: "docs/contracts.md", line: 1, token: "old_cmd" },
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("history homes are never scanned", () => {
  const root = makeTree({
    "docs/adr/0084-x.md": "old_cmd everywhere\n",
    "docs/kanban-archive.md": "old_cmd\n",
    "docs/bad-ideas.md": "old_cmd\n",
    "docs/retros/2026.md": "old_cmd\n",
    "docs/plans/v0.59-x.md": "old_cmd\n",
  });
  try {
    assert.equal(scan(root, { retired: [{ token: "old_cmd", adr: "A" }] }).length, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("plans README is live and scanned; wiki is scanned", () => {
  const root = makeTree({
    "docs/plans/README.md": "run old_cmd\n",
    "wiki/how-to.md": "old_cmd\n",
  });
  try {
    const v = scan(root, { retired: [{ token: "old_cmd", adr: "A" }] });
    assert.deepEqual(v.map((x) => x.file).sort(), ["docs/plans/README.md", "wiki/how-to.md"]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("allow list exempts exactly the listed file", () => {
  const root = makeTree({
    "docs/testing.md": "the now-retired old_cmd taught us a lesson\n",
    "docs/contracts.md": "old_cmd is available\n",
  });
  try {
    const v = scan(root, {
      retired: [{ token: "old_cmd", adr: "A", allow: ["docs/testing.md"] }],
    });
    assert.deepEqual(v.map((x) => x.file), ["docs/contracts.md"]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("token boundaries: embedded identifiers do not match", () => {
  assert.deepEqual(findHits("bold_cmdX and old_cmd2 here", "old_cmd"), []);
  assert.deepEqual(findHits("`old_cmd` in backticks", "old_cmd"), [1]);
  assert.deepEqual(findHits("path/old_cmd.rs mention", "old_cmd"), [1]);
});

test("isScanned covers only live markdown under docs/ and wiki/", () => {
  assert.equal(isScanned("docs/contracts.md"), true);
  assert.equal(isScanned("wiki/x.md"), true);
  assert.equal(isScanned("docs/adr/0001-x.md"), false);
  assert.equal(isScanned("docs/plans/old-plan.md"), false);
  assert.equal(isScanned("docs/plans/README.md"), true);
  assert.equal(isScanned("src/whatever.md"), false);
  assert.equal(isScanned("docs/image.png"), false);
});
