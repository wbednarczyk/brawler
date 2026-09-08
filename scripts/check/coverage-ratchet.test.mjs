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

function run(dir, extraArgs = [], envOverrides = {}) {
  // Default to a CLEAN environment for COVERAGE_BASE_REF: existing tests
  // exercise the ratchet with no base-baseline comparison, so an accidental
  // ambient COVERAGE_BASE_REF (e.g. set by a wrapping CI job) must never leak
  // in and change their outcome.
  const env = { ...process.env };
  delete env.COVERAGE_BASE_REF;
  Object.assign(env, envOverrides);
  return spawnSync("node", [SCRIPT_PATH, "--layer=frontend", ...extraArgs], { cwd: dir, encoding: "utf8", env });
}

function cleanup(dir) {
  rmSync(dir, { recursive: true, force: true });
}

const GIT_IDENTITY = ["-c", "user.name=Test", "-c", "user.email=test@example.com"];

function git(dir, args) {
  const result = spawnSync("git", [...GIT_IDENTITY, ...args], { cwd: dir, encoding: "utf8" });
  if (result.status !== 0) throw new Error(`git ${args.join(" ")} failed: ${result.stderr}`);
  return result.stdout;
}

/** Commit the CURRENT coverage-baseline.json in `dir` as the "base" revision
 * (what COVERAGE_BASE_REF will point at), then restore the head content the
 * test actually wants the script to read from disk. */
function commitAsBase(dir, baseBaselineObj) {
  const baselinePath = path.join(dir, "coverage-baseline.json");
  const headContent = readFileSync(baselinePath, "utf8");
  git(dir, ["init", "-q"]);
  writeFileSync(baselinePath, JSON.stringify(baseBaselineObj, null, 2));
  git(dir, ["add", "-A"]);
  git(dir, ["commit", "-q", "-m", "base"]);
  const baseSha = git(dir, ["rev-parse", "HEAD"]).trim();
  writeFileSync(baselinePath, headContent); // restore head (uncommitted)
  return baseSha;
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

// --- G15 fix #5: base-baseline admission-hole close --------------------------

test("COVERAGE_BASE_REF unset skips the base-baseline comparison with a printed note and does not fail", () => {
  const dir = setup({
    baseline: {
      frontend: { lines: 80.0, dirs: { "src/screens/Today": 90, "src/ui": 70, "src/app": 30 } },
      rust: { lines: 86.5 },
    },
    summary: SUMMARY,
  });
  try {
    const r = run(dir);
    assert.equal(r.status, 0, r.stdout + r.stderr);
    assert.match(r.stdout, /COVERAGE_BASE_REF not set/);
  } finally {
    cleanup(dir);
  }
});

test("a NEW directory (absent from the base baseline) entering with a pin of 0 fails even at high measured coverage", () => {
  const SUMMARY_APP_100 = {
    total: { lines: { total: 100, covered: 98, skipped: 0, pct: 98 } },
    "src/screens/Today/TodayScreen.tsx": { lines: { total: 20, covered: 18, skipped: 0, pct: 90 } },
    "src/ui/Button.tsx": { lines: { total: 10, covered: 8, skipped: 0, pct: 80 } },
    "src/app/AppStateRoot.tsx": { lines: { total: 10, covered: 10, skipped: 0, pct: 100 } },
  };
  const dir = setup({
    baseline: {
      // Head baseline: src/app admitted with pin 0 — the exploit this closes.
      frontend: { lines: 80.0, dirs: { "src/screens/Today": 90, "src/ui": 80, "src/app": 0 } },
      rust: { lines: 86.5 },
    },
    summary: SUMMARY_APP_100,
  });
  try {
    const baseSha = commitAsBase(dir, {
      // Base baseline: src/app was never pinned at all — it's a NEW key.
      frontend: { lines: 80.0, dirs: { "src/screens/Today": 90, "src/ui": 80 } },
      rust: { lines: 86.5 },
    });
    const r = run(dir, [], { COVERAGE_BASE_REF: baseSha });
    assert.equal(r.status, 1, r.stdout + r.stderr);
    assert.match(r.stderr, /src\/app.*new directory/is);
  } finally {
    cleanup(dir);
  }
});

test("an existing directory's pin lowered from the base baseline fails even though measured coverage still clears it", () => {
  const dir = setup({
    baseline: {
      // src/ui lowered 90 (base) -> 70 (head); measured 80% clears 70, so the
      // plain floor check alone would say "ok" — this must still fail.
      frontend: { lines: 80.0, dirs: { "src/screens/Today": 90, "src/ui": 70, "src/app": 30 } },
      rust: { lines: 86.5 },
    },
    summary: SUMMARY,
  });
  try {
    const baseSha = commitAsBase(dir, {
      frontend: { lines: 80.0, dirs: { "src/screens/Today": 90, "src/ui": 90, "src/app": 30 } },
      rust: { lines: 86.5 },
    });
    const r = run(dir, [], { COVERAGE_BASE_REF: baseSha });
    assert.equal(r.status, 1, r.stdout + r.stderr);
    assert.match(r.stderr, /src\/ui.*pin lowered/is);
  } finally {
    cleanup(dir);
  }
});

// --- G15 fix #6: pinned-but-unmeasured directory ------------------------------

test("a pinned directory absent from the summary fails when the directory still exists on disk", () => {
  const dir = setup({
    baseline: {
      frontend: { lines: 80.0, dirs: { "src/screens/Today": 90, "src/ui": 70, "src/app": 30, "src/shared": 60 } },
      rust: { lines: 86.5 },
    },
    summary: SUMMARY, // src/shared is pinned but not present in the summary at all
  });
  try {
    mkdirSync(path.join(dir, "src/shared"), { recursive: true });
    writeFileSync(path.join(dir, "src/shared/.gitkeep"), "");
    const r = run(dir);
    assert.equal(r.status, 1, r.stdout + r.stderr);
    assert.match(r.stderr, /src\/shared.*no measured coverage/s);
  } finally {
    cleanup(dir);
  }
});

test("a pinned directory absent from the summary AND from disk passes with a drop-the-pin note", () => {
  const dir = setup({
    baseline: {
      frontend: { lines: 80.0, dirs: { "src/screens/Today": 90, "src/ui": 70, "src/app": 30, "src/shared": 60 } },
      rust: { lines: 86.5 },
    },
    summary: SUMMARY,
  });
  try {
    // src/shared is never created on disk in this tmp dir — simulates deleted source.
    const r = run(dir);
    assert.equal(r.status, 0, r.stdout + r.stderr);
    assert.match(r.stdout, /src\/shared.*drop/s);
  } finally {
    cleanup(dir);
  }
});

// --- G15 fix #7: src/(root) bucket + number validation ------------------------

test("a frontend file outside the fixed buckets lands in the src/(root) bucket and is enforced", () => {
  const dir = setup({
    baseline: {
      frontend: { lines: 80.0, dirs: { "src/screens/Today": 90, "src/ui": 70, "src/app": 30, "src/(root)": 50 } },
      rust: { lines: 86.5 },
    },
    summary: { ...SUMMARY, "src/main.tsx": { lines: { total: 10, covered: 4, skipped: 0, pct: 40 } } },
  });
  try {
    const r = run(dir);
    assert.equal(r.status, 1, r.stdout + r.stderr); // measured 40% < pinned 50%
    assert.match(r.stdout, /FAIL frontend src\/\(root\)/);
  } finally {
    cleanup(dir);
  }
});

test("a non-numeric pin in coverage-baseline.json fails with a clear message", () => {
  const dir = setup({
    baseline: {
      frontend: { lines: 80.0, dirs: { "src/screens/Today": 90, "src/ui": "eighty", "src/app": 30 } },
      rust: { lines: 86.5 },
    },
    summary: SUMMARY,
  });
  try {
    const r = run(dir);
    assert.equal(r.status, 1, r.stdout + r.stderr);
    assert.match(r.stderr, /src\/ui.*not a valid percentage/);
  } finally {
    cleanup(dir);
  }
});

test("a pin above 100% fails with a clear message", () => {
  const dir = setup({
    baseline: {
      frontend: { lines: 80.0, dirs: { "src/screens/Today": 90, "src/ui": 150, "src/app": 30 } },
      rust: { lines: 86.5 },
    },
    summary: SUMMARY,
  });
  try {
    const r = run(dir);
    assert.equal(r.status, 1, r.stdout + r.stderr);
    assert.match(r.stderr, /src\/ui.*not a valid percentage/);
  } finally {
    cleanup(dir);
  }
});
