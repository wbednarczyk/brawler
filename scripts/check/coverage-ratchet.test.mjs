// Coverage ratchet (G15, hard gates wave 2): per-directory floors layered on
// top of the existing global-floor ratchet. Spawns the script with cwd set to
// a tmp dir carrying a synthetic coverage-baseline.json + coverage-summary.json
// (frontend layer only — rust aggregation shares the same code path).
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_PATH = path.resolve(fileURLToPath(new URL(".", import.meta.url)), "coverage-ratchet.mjs");

function setup({ baseline, summary }) {
  const dir = mkdtempSync(path.join(tmpdir(), "coverage-ratchet-"));
  mkdirSync(path.join(dir, "coverage/frontend"), { recursive: true });
  writeFileSync(path.join(dir, "coverage-baseline.json"), JSON.stringify(baseline, null, 2));
  writeFileSync(path.join(dir, "coverage/frontend/coverage-summary.json"), JSON.stringify(summary, null, 2));
  return dir;
}

function run(dir, extraArgs = []) {
  return spawnSync("node", [SCRIPT_PATH, "--layer=frontend", ...extraArgs], { cwd: dir, encoding: "utf8" });
}

function cleanup(dir) {
  rmSync(dir, { recursive: true, force: true });
}

const SUMMARY = {
  total: { lines: { total: 100, covered: 90, skipped: 0, pct: 90 } },
  "src/screens/Today/TodayScreen.tsx": { lines: { total: 20, covered: 18, skipped: 0, pct: 90 } }, // 90%
  "src/ui/Button.tsx": { lines: { total: 10, covered: 8, skipped: 0, pct: 80 } }, // 80%
  "src/app/AppStateRoot.tsx": { lines: { total: 10, covered: 4, skipped: 0, pct: 40 } }, // 40%
};

test("a measured directory below its pinned floor fails", () => {
  const dir = setup({
    baseline: {
      frontend: { lines: 80.0, dirs: { "src/screens/Today": 95, "src/ui": 70, "src/app": 30 } },
      rust: { lines: 86.5 },
    },
    summary: SUMMARY,
  });
  try {
    const r = run(dir);
    assert.equal(r.status, 1);
    // Per-directory status lines (pinned or not) print like the existing
    // global-floor line — to stdout; only the closing summary is on stderr.
    assert.match(r.stdout, /FAIL frontend src\/screens\/Today/);
  } finally {
    cleanup(dir);
  }
});

test("a missing dirs block fails with the --seed bootstrap hint", () => {
  const dir = setup({
    baseline: { frontend: { lines: 80.0 }, rust: { lines: 86.5 } },
    summary: SUMMARY,
  });
  try {
    const r = run(dir);
    assert.equal(r.status, 1);
    assert.match(r.stderr, /missing "frontend\.dirs"/);
    assert.match(r.stderr, /--seed --layer=frontend/);
  } finally {
    cleanup(dir);
  }
});

test("an unpinned directory at or above 70% fails with an add-pin hint", () => {
  const dir = setup({
    baseline: {
      frontend: { lines: 80.0, dirs: { "src/screens/Today": 90, "src/app": 30 } }, // src/ui (80%) unpinned
      rust: { lines: 86.5 },
    },
    summary: SUMMARY,
  });
  try {
    const r = run(dir);
    assert.equal(r.status, 1);
    assert.match(r.stderr, /src\/ui.*not pinned/s);
    assert.match(r.stderr, /add `"src\/ui": 80\.0`/);
  } finally {
    cleanup(dir);
  }
});

test("an unpinned directory below 70% fails with the admission-floor message", () => {
  const dir = setup({
    baseline: {
      frontend: { lines: 80.0, dirs: { "src/screens/Today": 90, "src/ui": 70 } }, // src/app (40%) unpinned
      rust: { lines: 86.5 },
    },
    summary: SUMMARY,
  });
  try {
    const r = run(dir);
    assert.equal(r.status, 1);
    assert.match(r.stderr, /src\/app.*70% admission floor/s);
  } finally {
    cleanup(dir);
  }
});

test("--seed prints the dirs block and exits 0 without touching the baseline file", () => {
  const baseline = { frontend: { lines: 80.0 }, rust: { lines: 86.5 } };
  const dir = setup({ baseline, summary: SUMMARY });
  try {
    const before = readFileSync(path.join(dir, "coverage-baseline.json"), "utf8");
    const r = run(dir, ["--seed"]);
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /"frontend"/);
    assert.match(r.stdout, /"src\/screens\/Today": 90/);
    assert.match(r.stdout, /"src\/ui": 80/);
    const after = readFileSync(path.join(dir, "coverage-baseline.json"), "utf8");
    assert.equal(after, before);
  } finally {
    cleanup(dir);
  }
});

test("--write raises a pin that measured coverage now clears, and never lowers one", () => {
  const dir = setup({
    baseline: {
      frontend: { lines: 80.0, dirs: { "src/screens/Today": 50, "src/ui": 95, "src/app": 30 } },
      rust: { lines: 86.5 },
    },
    summary: SUMMARY,
  });
  try {
    const r = run(dir, ["--write"]);
    assert.equal(r.status, 0, r.stderr);
    const written = JSON.parse(readFileSync(path.join(dir, "coverage-baseline.json"), "utf8"));
    assert.equal(written.frontend.dirs["src/screens/Today"], 90); // 50 -> 90, raised
    assert.equal(written.frontend.dirs["src/ui"], 95); // measured 80 < pin 95 — never lowered
    assert.equal(written.frontend.dirs["src/app"], 40); // 30 -> 40, raised
  } finally {
    cleanup(dir);
  }
});
