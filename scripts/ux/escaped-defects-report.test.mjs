// Node built-in test runner (`node:test`) for the escaped-defect report
// (ADR 0081, plan Q7). Exercises the real file-scanning path against
// synthetic sample retro files under a throwaway temp dir — never against
// the maintainer's local (gitignored) retros, which stay unread by this
// suite on purpose.

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { run } from "./escaped-defects-report.mjs";

function markedTable(rows) {
  return [
    "<!-- escaped-defects:start -->",
    "| Ref | Origin class | Detected at | Earliest prevention point | Disposition | Status |",
    "| --- | --- | --- | --- | --- | --- |",
    ...rows,
    "<!-- escaped-defects:end -->",
    "",
  ].join("\n");
}

function withRetroDir(files, fn) {
  const dir = mkdtempSync(join(tmpdir(), "escaped-defects-test-"));
  try {
    for (const [name, content] of Object.entries(files)) {
      writeFileSync(join(dir, name), content, "utf8");
    }
    return fn(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("unknown origin class fails validation", () => {
  withRetroDir(
    {
      "sample.md": markedTable([
        "| d1 | not-a-real-class | implementation | code review | fixed-instance-only | closed |",
      ]),
    },
    (dir) => {
      const { exitCode, output } = run(dir);
      assert.equal(exitCode, 1);
      assert.match(output, /unknown origin class/);
    },
  );
});

test("unknown detection stage fails validation", () => {
  withRetroDir(
    {
      "sample.md": markedTable([
        "| d1 | spec-gap | some-made-up-stage | code review | fixed-instance-only | closed |",
      ]),
    },
    (dir) => {
      const { exitCode, output } = run(dir);
      assert.equal(exitCode, 1);
      assert.match(output, /unknown detection stage/);
    },
  );
});

test("unknown disposition fails validation", () => {
  withRetroDir(
    {
      "sample.md": markedTable([
        "| d1 | spec-gap | implementation | code review | wontfix | closed |",
      ]),
    },
    (dir) => {
      const { exitCode, output } = run(dir);
      assert.equal(exitCode, 1);
      assert.match(output, /unknown disposition/);
    },
  );
});

test("duplicate ref across files fails validation", () => {
  withRetroDir(
    {
      "a.md": markedTable([
        "| d1 | spec-gap | implementation | code review | fixed-instance-only | closed |",
      ]),
      "b.md": markedTable([
        "| d1 | test-flake | full-gate | targeted test | human-checklist | open |",
      ]),
    },
    (dir) => {
      const { exitCode, output } = run(dir);
      assert.equal(exitCode, 1);
      assert.match(output, /duplicate escaped-defect ref/);
    },
  );
});

test("old retro with no marked table is ignored", () => {
  withRetroDir(
    {
      "old.md": "# Retrospective\n\nNo escaped-defect table here — predates the taxonomy.\n",
    },
    (dir) => {
      const { exitCode, output } = run(dir);
      assert.equal(exitCode, 0);
      assert.match(output, /Scanned 1 retro file\(s\), 0 with a marked table/);
      assert.match(output, /Defects recorded: 0/);
    },
  );
});

test("counts and repeated-class output matches a small sample", () => {
  withRetroDir(
    {
      "a.md": markedTable([
        "| d1 | spec-gap | implementation | code review | fixed-instance-only | closed |",
        "| d2 | spec-gap | full-gate | targeted test | human-checklist | open |",
        "| d3 | test-flake | targeted-test | proptest | automated-guardrail | closed |",
      ]),
    },
    (dir) => {
      const { exitCode, output } = run(dir);
      assert.equal(exitCode, 0);
      assert.match(output, /Defects recorded: 3/);
      assert.match(output, /spec-gap: 2/);
      assert.match(output, /test-flake: 1/);
      assert.match(output, /Repeated origin classes[\s\S]*spec-gap \(2\)/);
    },
  );
});

test("increasing a count stays exit 0", () => {
  withRetroDir(
    {
      "a.md": markedTable([
        "| d1 | spec-gap | implementation | code review | fixed-instance-only | closed |",
      ]),
    },
    (dirSmall) => {
      assert.equal(run(dirSmall).exitCode, 0);
    },
  );
  withRetroDir(
    {
      "a.md": markedTable([
        "| d1 | spec-gap | implementation | code review | fixed-instance-only | closed |",
        "| d2 | spec-gap | implementation | code review | fixed-instance-only | closed |",
        "| d3 | spec-gap | implementation | code review | fixed-instance-only | closed |",
      ]),
    },
    (dirBig) => {
      const { exitCode, output } = run(dirBig);
      assert.equal(exitCode, 0);
      assert.match(output, /spec-gap: 3/);
    },
  );
});

test("malformed opted-in row exits non-zero", () => {
  withRetroDir(
    {
      "a.md": markedTable(["| d1 | spec-gap | implementation |  |"]),
    },
    (dir) => {
      const { exitCode, output } = run(dir);
      assert.equal(exitCode, 1);
      assert.match(output, /malformed/);
    },
  );
});

test("a tracked:<hex7> disposition is valid", () => {
  withRetroDir(
    {
      "a.md": markedTable([
        "| d1 | integration-seam | vertical-slice | contract test | tracked:1a2b3c4 | open |",
      ]),
    },
    (dir) => {
      const { exitCode, output } = run(dir);
      assert.equal(exitCode, 0);
      assert.match(output, /Defects recorded: 1/);
    },
  );
});
