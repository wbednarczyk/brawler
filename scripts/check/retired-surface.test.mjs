// Tests for the retired-surface gate: live docs may not reference retired
// surface; history homes and per-token allow lists are exempt.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { findHits, isPointerLine, isScanned, scan } from "./retired-surface.mjs";

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

test("allow permits only pointer-shaped lines in the listed file", () => {
  const root = makeTree({
    "docs/testing.md":
      "the now-retired old_cmd taught us a lesson\nbut old_cmd is still how you run it\n",
    "docs/contracts.md": "old_cmd was removed (ADR X)\n",
  });
  try {
    const v = scan(root, {
      retired: [{ token: "old_cmd", adr: "A", allow: ["docs/testing.md"] }],
    });
    // testing.md line 1 is pointer-shaped (allowed); line 2 specifies live
    // behavior (violation). contracts.md is not allow-listed at all — even its
    // pointer-shaped line reddens until the allowance is reviewed in.
    assert.deepEqual(
      v.map((x) => `${x.file}:${x.line}`).sort(),
      ["docs/contracts.md:1", "docs/testing.md:2"],
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("allowFile exempts the whole file regardless of line shape", () => {
  const root = makeTree({
    "docs/testing.md": "old_cmd corpus rows: 17 old_cmd documents\n",
  });
  try {
    const v = scan(root, {
      retired: [{ token: "old_cmd", adr: "A", allowFile: ["docs/testing.md"] }],
    });
    assert.equal(v.length, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("token boundaries: embedded and hyphen-extended identifiers do not match", () => {
  assert.deepEqual(findHits("bold_cmdX and old_cmd2 here", "old_cmd"), []);
  assert.deepEqual(findHits("`old_cmd` in backticks", "old_cmd").map((h) => h.line), [1]);
  assert.deepEqual(findHits("path/old_cmd.rs mention", "old_cmd").map((h) => h.line), [1]);
  assert.deepEqual(findHits("use check-epic-v2 here", "check-epic"), []);
  assert.deepEqual(findHits("run make check-epic now", "check-epic").map((h) => h.line), [1]);
});

test("isPointerLine recognizes retirement wording and rejects live prose", () => {
  assert.equal(isPointerLine("`old_cmd` was dropped by migration 0102"), true);
  assert.equal(isPointerLine("renamed from `mutants.yml`"), true);
  assert.equal(isPointerLine("call `old_cmd` to refresh the panel"), false);
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
