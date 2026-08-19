// Negative tests for the file-size ratchet (ADR 0103): the fitness function
// that freezes oversized production source files and only lets the baseline
// move down. Pure-logic tests on plain objects; one tmp-dir test for the
// filesystem scan (exclusions + determinism).
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { countLines, isExcluded, scanAll, evaluate, ratchetDown, bootstrap } from "./file-size-ratchet.mjs";

const THRESHOLD = 100;

test("a new file at or over the threshold outside the baseline fails", () => {
  const failures = evaluate({ files: {} }, { "src/big.ts": 100 }, THRESHOLD);
  assert.equal(failures.length, 1);
  assert.match(failures[0], /src\/big\.ts/);
  assert.match(failures[0], /not baselined/);
});

test("a baselined file that grew fails and names the hand-edit escape hatch", () => {
  const failures = evaluate({ files: { "src/big.ts": 150 } }, { "src/big.ts": 151 }, THRESHOLD);
  assert.equal(failures.length, 1);
  assert.match(failures[0], /grew 150 -> 151/);
  assert.match(failures[0], /file-size-baseline\.json/);
});

test("a baselined file that shrank without --write fails with the --write instruction", () => {
  const failures = evaluate({ files: { "src/big.ts": 150 } }, { "src/big.ts": 120 }, THRESHOLD);
  assert.equal(failures.length, 1);
  assert.match(failures[0], /shrank 150 -> 120/);
  assert.match(failures[0], /--write/);
});

test("a baselined file that no longer exists fails until --write removes it", () => {
  const failures = evaluate({ files: { "src/gone.ts": 150 } }, {}, THRESHOLD);
  assert.equal(failures.length, 1);
  assert.match(failures[0], /no longer exists/);
  assert.match(failures[0], /--write/);

  const { baseline, refusals } = ratchetDown({ files: { "src/gone.ts": 150 } }, {}, THRESHOLD);
  assert.equal(refusals.length, 0);
  assert.deepEqual(baseline.files, {});
});

test("an exactly-pinned baseline passes clean", () => {
  const failures = evaluate({ files: { "src/big.ts": 150 } }, { "src/big.ts": 150, "src/small.ts": 20 }, THRESHOLD);
  assert.deepEqual(failures, []);
});

test("--write ratchets pins down and drops entries that fell under the threshold", () => {
  const old = { files: { "src/a.ts": 150, "src/b.ts": 150 } };
  const scan = { "src/a.ts": 120, "src/b.ts": 80 };
  const { baseline, refusals } = ratchetDown(old, scan, THRESHOLD);
  assert.equal(refusals.length, 0);
  assert.deepEqual(baseline.files, { "src/a.ts": 120 });
  // the ratcheted baseline is exactly what evaluate() then accepts
  assert.deepEqual(evaluate(baseline, scan, THRESHOLD), []);
});

test("--write refuses to raise a pin (growth is a hand edit, never automated)", () => {
  const { refusals } = ratchetDown({ files: { "src/a.ts": 150 } }, { "src/a.ts": 200 }, THRESHOLD);
  assert.equal(refusals.length, 1);
  assert.match(refusals[0], /refuses to raise/);
  assert.match(refusals[0], /150 -> 200/);
});

test("--write refuses to add a new offender to the baseline", () => {
  const { refusals } = ratchetDown({ files: {} }, { "src/new.ts": 150 }, THRESHOLD);
  assert.equal(refusals.length, 1);
  assert.match(refusals[0], /refuses to add/);
});

test("bootstrap builds a sorted baseline of every file at or over the threshold", () => {
  const baseline = bootstrap({ "src/z.ts": 150, "src/a.ts": 120, "src/small.ts": 20 }, THRESHOLD);
  assert.deepEqual(Object.keys(baseline.files), ["src/a.ts", "src/z.ts"]);
  assert.equal(baseline.thresholdLines, THRESHOLD);
});

test("countLines counts newlines (wc -l equivalent)", () => {
  assert.equal(countLines("a\nb\n"), 2);
  assert.equal(countLines("a\nb"), 1); // no final newline: last fragment uncounted, like wc -l
  assert.equal(countLines(""), 0);
});

test("exclusions: dedicated test files, generated bindings and locale resources are out of scope", () => {
  for (const rel of [
    "src/test/scenarios/runtime.ts",
    "src/api/generated/Thing.ts",
    "src/shared/locale/resources/plText.ts",
    "src-tauri/src/storage/tests/migration_safety.rs",
    "src-tauri/src/storage/kpi_ingest_commit/tests.rs",
    "src/screens/Today/TodayScreen.test.tsx",
    "tests/live/kpi.live.spec.ts",
  ]) {
    assert.equal(isExcluded(rel), true, `${rel} should be excluded`);
  }
  for (const rel of [
    "src-tauri/src/mcp/registry.rs",
    "src/app/AppStateRoot.tsx",
    "src/shared/components/TickerLabel.tsx",
  ]) {
    assert.equal(isExcluded(rel), false, `${rel} should be in scope`);
  }
});

test("scanAll walks the real tree deterministically and honors exclusions", () => {
  const root = mkdtempSync(path.join(tmpdir(), "fsr-"));
  try {
    mkdirSync(path.join(root, "src/test"), { recursive: true });
    mkdirSync(path.join(root, "src-tauri/src/mcp"), { recursive: true });
    writeFileSync(path.join(root, "src/app.tsx"), "x\n".repeat(5));
    writeFileSync(path.join(root, "src/app.css"), "x\n".repeat(5)); // wrong extension: out
    writeFileSync(path.join(root, "src/test/helper.ts"), "x\n".repeat(5)); // excluded dir
    writeFileSync(path.join(root, "src-tauri/src/mcp/registry.rs"), "x\n".repeat(7));
    const first = scanAll(root);
    assert.deepEqual(first, { "src-tauri/src/mcp/registry.rs": 7, "src/app.tsx": 5 });
    assert.deepEqual(scanAll(root), first); // deterministic
    assert.deepEqual(Object.keys(first), Object.keys(first).slice().sort()); // sorted keys
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
