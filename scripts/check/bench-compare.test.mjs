import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { collectKernels, compareReport } from "./bench-compare.mjs";

function makeRoot() {
  return mkdtempSync(join(tmpdir(), "bench-compare-test-"));
}

function writeEstimates(dir, pointEstimate, { lowerBound, upperBound } = {}) {
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, "estimates.json"),
    JSON.stringify({
      median: {
        point_estimate: pointEstimate,
        confidence_interval: {
          confidence_level: 0.95,
          lower_bound: lowerBound ?? pointEstimate,
          upper_bound: upperBound ?? pointEstimate,
        },
        standard_error: 0,
      },
    }),
  );
}

// Writes a fully comparable kernel: audit-base/, new/, change/ estimates.json.
function writeComparableKernel(root, kernelPath, { baseNs, headNs, changePoint, changeLower }) {
  const dir = join(root, kernelPath);
  writeEstimates(join(dir, "audit-base"), baseNs);
  writeEstimates(join(dir, "new"), headNs);
  writeEstimates(join(dir, "change"), changePoint, { lowerBound: changeLower, upperBound: changeLower });
}

test("regression: change lower_bound above threshold fails", () => {
  const root = makeRoot();
  try {
    writeComparableKernel(root, "regressed_kernel", { baseNs: 1000, headNs: 1450, changePoint: 0.45, changeLower: 0.45 });
    const report = compareReport(root);
    assert.equal(report.comparable.length, 1);
    assert.equal(report.comparable[0].verdict, "FAIL");
    assert.equal(report.overallFail, true);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("noisy-but-unproven: large point estimate but low lower_bound stays ok", () => {
  const root = makeRoot();
  try {
    writeComparableKernel(root, "noisy_kernel", { baseNs: 1000, headNs: 1500, changePoint: 0.5, changeLower: 0.1 });
    const report = compareReport(root);
    assert.equal(report.comparable[0].verdict, "ok");
    assert.equal(report.overallFail, false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("within tolerance: small delta stays ok", () => {
  const root = makeRoot();
  try {
    writeComparableKernel(root, "stable_kernel", { baseNs: 1000, headNs: 1050, changePoint: 0.05, changeLower: -0.02 });
    const report = compareReport(root);
    assert.equal(report.comparable[0].verdict, "ok");
    assert.equal(report.overallFail, false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("added and removed kernels are informational, never fail", () => {
  const root = makeRoot();
  try {
    writeComparableKernel(root, "steady_kernel", { baseNs: 1000, headNs: 1010, changePoint: 0.01, changeLower: -0.05 });
    writeEstimates(join(root, "added_kernel", "new"), 800);
    writeEstimates(join(root, "removed_kernel", "audit-base"), 900);

    const report = compareReport(root);
    assert.deepEqual(report.added, ["added_kernel"]);
    assert.deepEqual(report.removed, ["removed_kernel"]);
    assert.equal(report.comparable.length, 1);
    assert.equal(report.overallFail, false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("zero comparable kernels errors loudly", () => {
  const root = makeRoot();
  try {
    writeEstimates(join(root, "added_only", "new"), 800);
    writeEstimates(join(root, "removed_only", "audit-base"), 900);
    assert.throws(() => compareReport(root), /zero comparable kernels/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("malformed estimates.json names the offending file", () => {
  const root = makeRoot();
  try {
    const dir = join(root, "broken_kernel");
    writeEstimates(join(dir, "audit-base"), 1000);
    mkdirSync(join(dir, "new"), { recursive: true });
    writeFileSync(join(dir, "new", "estimates.json"), "not json");

    assert.throws(() => compareReport(root), (err) => {
      assert.match(err.message, /new[\\/]estimates\.json/);
      return true;
    });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("nested kernel directories are enumerated with the joined id", () => {
  const root = makeRoot();
  try {
    writeComparableKernel(root, join("group", "func"), { baseNs: 2000, headNs: 2100, changePoint: 0.05, changeLower: -0.01 });

    const kernels = collectKernels(root);
    assert.ok(kernels.has("group/func"), `expected 'group/func' in ${[...kernels.keys()]}`);

    const report = compareReport(root);
    assert.equal(report.comparable[0].id, "group/func");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
