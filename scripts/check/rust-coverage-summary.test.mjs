// Tests for rust-coverage-summary.mjs (#488): the lcov -> production-only
// coverage summary converter. Mirrors coverage-ratchet.test.mjs's style —
// tmp dir per test, spawn the CLI for exit-code cases, import the exports
// for unit cases.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, existsSync, readFileSync, rmSync, readdirSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { structuralTokens, cfgTestSpans, testOnlyFiles, convert } from "./rust-coverage-summary.mjs";

const SCRIPT_PATH = path.resolve(fileURLToPath(new URL(".", import.meta.url)), "rust-coverage-summary.mjs");
const REPO_ROOT = path.resolve(fileURLToPath(new URL(".", import.meta.url)), "../..");

function tmpTree() {
  return mkdtempSync(path.join(tmpdir(), "rust-coverage-summary-"));
}

function writeFiles(dir, files) {
  for (const [rel, content] of Object.entries(files)) {
    const p = path.join(dir, rel);
    mkdirSync(path.dirname(p), { recursive: true });
    writeFileSync(p, content);
  }
}

function lcovRecord(sf, das) {
  return [`SF:${sf}`, ...das.map(([ln, cnt]) => `DA:${ln},${cnt}`), "end_of_record"].join("\n");
}

function runCli(dir, args) {
  return spawnSync("node", [SCRIPT_PATH, ...args], { cwd: dir, encoding: "utf8" });
}

function cleanup(dir) {
  rmSync(dir, { recursive: true, force: true });
}

// ---- 1. inline mod tests {} removed; verbatim LF>DA record proves DA-only ---

test("inline #[cfg(test)] mod tests {} is stripped; production lines/counts survive; LF is ignored in favor of DA", () => {
  const dir = tmpTree();
  try {
    const aRs = [
      "pub fn add(a: i32, b: i32) -> i32 {",
      "    a + b",
      "}",
      "",
      "#[cfg(test)]",
      "mod tests {",
      "    use super::*;",
      "",
      "    #[test]",
      "    fn it_adds() {",
      "        assert_eq!(add(1, 2), 3);",
      "    }",
      "}",
      "",
    ].join("\n");
    // A 28-line stand-in for aggregator_fundamentals.rs (no cfg(test) content),
    // long enough to host the verbatim DA lines 17-28 below.
    const aggRs = Array.from({ length: 28 }, (_, i) => `// line ${i + 1}`).join("\n") + "\n";
    writeFiles(dir, {
      "src/a.rs": aRs,
      "src/commands/aggregator_fundamentals.rs": aggRs,
    });
    const lcovText = [
      lcovRecord("src/a.rs", [
        [1, 5],
        [2, 5],
        [10, 3],
        [11, 3],
      ]),
      // Verbatim record from coverage/rust.lcov (LF:12, only 10 DA lines).
      [
        "SF:src/commands/aggregator_fundamentals.rs",
        "FN:24,_RNCNCNvNtNtCs4ADLTnsBP7P_11brawler_lib8commands23aggregator_fundamentals32run_aggregator_fundamentals_pull00B9_",
        "FN:20,_RNCNvNtNtCs4ADLTnsBP7P_11brawler_lib8commands23aggregator_fundamentals32run_aggregator_fundamentals_pull0B7_",
        "FN:18,_RNvNtNtCs4ADLTnsBP7P_11brawler_lib8commands23aggregator_fundamentals32run_aggregator_fundamentals_pull",
        "FNDA:0,_RNCNCNvNtNtCs4ADLTnsBP7P_11brawler_lib8commands23aggregator_fundamentals32run_aggregator_fundamentals_pull00B9_",
        "FNF:3",
        "FNH:0",
        "DA:17,0",
        "DA:18,0",
        "DA:19,0",
        "DA:20,0",
        "DA:21,0",
        "DA:24,0",
        "DA:25,0",
        "DA:26,0",
        "DA:27,0",
        "DA:28,0",
        "BRF:0",
        "BRH:0",
        "LF:12",
        "LH:0",
        "end_of_record",
      ].join("\n"),
    ].join("\n");
    const out = convert({ lcovText, root: "src", cwd: dir });
    const a = out.data[0].files.find((f) => f.filename === "src/a.rs");
    assert.deepEqual(a.summary.lines, { count: 2, covered: 2, percent: 100 });
    const agg = out.data[0].files.find((f) => f.filename === "src/commands/aggregator_fundamentals.rs");
    assert.equal(agg.summary.lines.count, 10, "count must come from the 10 DA records, not LF:12");
    assert.equal(agg.summary.lines.covered, 0);
  } finally {
    cleanup(dir);
  }
});

// ---- 2. external #[cfg(test)] mod x; variants --------------------------------

test("external test-only mod declarations resolve: x.rs, x/mod.rs, #[path], from mod.rs and from a child dir", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      // Declared from src/mod.rs (dir = its own dir "src/"): x.rs form, and
      // the #[path] form.
      "src/mod.rs": ["#[cfg(test)]", "mod helpers;", "", "#[cfg(test)]", "#[path = \"aliased_impl.rs\"]", "mod aliased;"].join("\n"),
      "src/helpers.rs": "// test-only via x.rs form\n",
      "src/aliased_impl.rs": "// test-only via #[path]\n",
      // Declared from a plain (non-mod/lib/main) file src/foo.rs, whose own
      // submodule dir is "src/foo/": x/mod.rs form.
      "src/foo.rs": ["#[cfg(test)]", "mod nested_dir;"].join("\n"),
      "src/foo/nested_dir/mod.rs": "// test-only via x/mod.rs form, declared from foo.rs (child dir)\n",
    });
    const { testOnly, errors } = testOnlyFiles(path.join(dir, "src"));
    assert.deepEqual(errors, []);
    const rels = [...testOnly].map((p) => path.relative(dir, p)).sort();
    assert.deepEqual(rels, ["src/aliased_impl.rs", "src/foo/nested_dir/mod.rs", "src/helpers.rs"]);
  } finally {
    cleanup(dir);
  }
});

// ---- 3. transitive test-only propagation -------------------------------------

test("a mod declared inside a test-only file is also test-only, including a #[path] child", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": ["#[cfg(test)]", "mod tests;"].join("\n"),
      "src/tests.rs": ["mod helpers;", "", "#[path = \"alt.rs\"]", "mod aliased;"].join("\n"),
      "src/tests/helpers.rs": "// pulled in transitively\n",
      // #[path = "alt.rs"] resolves relative to the DECLARING file's own
      // directory (tests.rs lives at src/tests.rs, so dirname = "src"), not
      // to tests.rs's own submodule dir (src/tests/).
      "src/alt.rs": "// pulled in transitively via #[path]\n",
    });
    const { testOnly, errors } = testOnlyFiles(path.join(dir, "src"));
    assert.deepEqual(errors, []);
    const rels = [...testOnly].map((p) => path.relative(dir, p)).sort();
    assert.deepEqual(rels, ["src/alt.rs", "src/tests.rs", "src/tests/helpers.rs"]);
  } finally {
    cleanup(dir);
  }
});

// ---- 4. tokenizer edge cases --------------------------------------------------

test("tokenizer skips strings/raw strings/comments/chars/lifetimes when finding item end", () => {
  const lines = [
    "#[cfg(test)]",
    "fn weird_tokens() {",
    "    let s = \"{ not a brace, and a lifetime-looking 'x too }\";",
    "    let r = r#\"{ not a brace either \"#;",
    "    /* outer /* nested { still a comment } */ still commented too */",
    "    /// doc comment brace {",
    "    let c = '{';",
    "    'outer: loop {",
    "        break 'outer;",
    "    }",
    "}",
    "",
    "fn production_survives() {",
    "    1",
    "}",
  ];
  const src = lines.join("\n");
  const spans = cfgTestSpans("f.rs", src);
  assert.deepEqual(spans, [[1, 11]]);

  // const with a column-0 `} else {` stays one span.
  const constSrc = ["#[cfg(test)]", "const X: i32 = if true {", "    1", "} else {", "    2", "};"].join("\n");
  assert.deepEqual(cfgTestSpans("f.rs", constSrc), [[1, 6]]);

  // type alias with an array-size `;` inside `[]` ends at its own trailing `;`.
  const typeSrc = ["#[cfg(test)]", "type T = [u8; 4];"].join("\n");
  assert.deepEqual(cfgTestSpans("f.rs", typeSrc), [[1, 2]]);

  // tuple struct ends at the `;` after its parens, not inside them.
  const structSrc = ["#[cfg(test)]", "struct S(u8);"].join("\n");
  assert.deepEqual(cfgTestSpans("f.rs", structSrc), [[1, 2]]);
});

// ---- 5. attributes/comments stay inside the span; nested cfg(test) untouched -

test("attribute and comment lines between #[cfg(test)] and the item stay in the span; an indented #[cfg(test)] is not excluded", () => {
  const lines = [
    "#[cfg(test)]",
    "#[allow(dead_code)]",
    "// a leading comment",
    "fn t() {",
    "}",
    "",
    "fn prod() {",
    "    #[cfg(test)]",
    "    fn nested_not_excluded() {}",
    "}",
  ];
  const spans = cfgTestSpans("f.rs", lines.join("\n"));
  assert.deepEqual(spans, [[1, 5]]);
});

// ---- 6. self-checks: each exits 1, message names the file, output not written

function selfCheckHarness(dir, opts) {
  const outPath = path.join(dir, "coverage", "rust-summary.json");
  const r = runCli(dir, ["--lcov", opts.lcov ?? "rust.lcov", "--root", opts.root ?? "src", "--out", "coverage/rust-summary.json"]);
  return { r, outPath };
}

test("self-check 1: unresolved #[cfg(test)] mod fails with the file and candidates", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": ["#[cfg(test)]", "mod missing;"].join("\n"),
      "rust.lcov": lcovRecord("src/lib.rs", []),
    });
    const { r, outPath } = selfCheckHarness(dir, {});
    assert.equal(r.status, 1);
    assert.match(r.stderr, /rust-coverage-summary:.*resolves to no existing file/s);
    assert.match(r.stderr, /src[/\\]lib\.rs/);
    assert.equal(existsSync(outPath), false);
  } finally {
    cleanup(dir);
  }
});

test("self-check 2: a nested mod; inside an inline #[cfg(test)] block is unsupported", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": ["#[cfg(test)]", "mod tests {", "    mod nested;", "}"].join("\n"),
      "rust.lcov": lcovRecord("src/lib.rs", []),
    });
    const { r, outPath } = selfCheckHarness(dir, {});
    assert.equal(r.status, 1);
    assert.match(r.stderr, /unsupported: declare the module externally or inline its code/);
    assert.match(r.stderr, /src[/\\]lib\.rs:3/);
    assert.equal(existsSync(outPath), false);
  } finally {
    cleanup(dir);
  }
});

test("self-check 3: a file reachable from both a test-only and a production mod declaration is ambiguous", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": ["#[cfg(test)]", "mod shared_target;"].join("\n"),
      "src/shared_target.rs": "// test-only side\n",
      "src/other.rs": ["#[path = \"shared_target.rs\"]", "mod shared_target;"].join("\n"),
      "rust.lcov": lcovRecord("src/lib.rs", []),
    });
    const { r, outPath } = selfCheckHarness(dir, {});
    assert.equal(r.status, 1);
    assert.match(r.stderr, /ambiguous/);
    assert.match(r.stderr, /src[/\\]other\.rs/);
    assert.equal(existsSync(outPath), false);
  } finally {
    cleanup(dir);
  }
});

test("self-check 4: a span whose end line is not an item boundary fails", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      // A single-line fn body means the end line ("fn weird() { 1 }") is
      // neither exactly "}" nor ends with ";".
      "src/lib.rs": ["#[cfg(test)]", "fn weird() { 1 }", "fn other() {}"].join("\n"),
      "rust.lcov": lcovRecord("src/lib.rs", []),
    });
    const { r, outPath } = selfCheckHarness(dir, {});
    assert.equal(r.status, 1);
    assert.match(r.stderr, /span end not at an item boundary/);
    assert.match(r.stderr, /src[/\\]lib\.rs:2/);
    assert.equal(existsSync(outPath), false);
  } finally {
    cleanup(dir);
  }
});

test("self-check 5: an unterminated block comment is a lexical failure", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": ["/* never closed", "fn f() {}"].join("\n"),
      "rust.lcov": lcovRecord("src/lib.rs", []),
    });
    const { r, outPath } = selfCheckHarness(dir, {});
    assert.equal(r.status, 1);
    assert.match(r.stderr, /lexical failure/);
    assert.match(r.stderr, /src[/\\]lib\.rs/);
    assert.equal(existsSync(outPath), false);
  } finally {
    cleanup(dir);
  }
});

test("self-check 6: an SF: path under --root that doesn't exist on disk is a stale lcov", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": "pub fn f() {}\n",
      "rust.lcov": lcovRecord("src/gone.rs", [[1, 1]]),
    });
    const { r, outPath } = selfCheckHarness(dir, {});
    assert.equal(r.status, 1);
    assert.match(r.stderr, /stale lcov/);
    assert.match(r.stderr, /src[/\\]gone\.rs/);
    assert.equal(existsSync(outPath), false);
  } finally {
    cleanup(dir);
  }
});

test("self-check 7: an inline span that never closes (EOF) fails", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": ["#[cfg(test)]", "fn f() {", "    // never closes"].join("\n"),
      "rust.lcov": lcovRecord("src/lib.rs", []),
    });
    const { r, outPath } = selfCheckHarness(dir, {});
    assert.equal(r.status, 1);
    assert.match(r.stderr, /never closes/);
    assert.match(r.stderr, /src[/\\]lib\.rs:2/);
    assert.equal(existsSync(outPath), false);
  } finally {
    cleanup(dir);
  }
});

// ---- 7. zero-line files dropped; testOnlyDirs bucketing ----------------------

test("a file with zero remaining lines is absent from files[] and totals", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/all_test.rs": ["#[cfg(test)]", "fn only_test() {", "    1", "}"].join("\n"),
    });
    const lcovText = lcovRecord("src/all_test.rs", [[3, 5]]); // line 3 is inside the span
    const out = convert({ lcovText, root: "src", cwd: dir });
    assert.equal(
      out.data[0].files.find((f) => f.filename === "src/all_test.rs"),
      undefined,
    );
    assert.deepEqual(out.data[0].totals.lines, { count: 0, covered: 0, percent: 0 });
  } finally {
    cleanup(dir);
  }
});

test("testOnlyDirs names a dir whose every file is test-only, not a dir with one production file", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": [
        "#[cfg(test)]",
        "#[path = \"alltest/a.rs\"]",
        "mod alltest_a;",
        "#[cfg(test)]",
        "#[path = \"alltest/b.rs\"]",
        "mod alltest_b;",
        "#[cfg(test)]",
        "#[path = \"mixed/test_only.rs\"]",
        "mod mixed_test_only;",
        "#[path = \"mixed/prod.rs\"]",
        "mod mixed_prod;",
      ].join("\n"),
      "src/alltest/a.rs": "// every file in this dir is test-only\n",
      "src/alltest/b.rs": "// every file in this dir is test-only\n",
      "src/mixed/test_only.rs": "// one test-only file in a mixed dir\n",
      "src/mixed/prod.rs": "pub fn prod() {}\n",
    });
    const out = convert({ lcovText: "", root: "src", cwd: dir });
    assert.ok(out.testOnlyDirs.includes("src/alltest"), out.testOnlyDirs.join(","));
    assert.ok(!out.testOnlyDirs.includes("src/mixed"), out.testOnlyDirs.join(","));
  } finally {
    cleanup(dir);
  }
});

test("testOnlyDirs: a loose file under root contributes to the (root) bucket only if every loose file is test-only", () => {
  const positive = tmpTree();
  const negative = tmpTree();
  try {
    // Declaring file lives in a subdir (not loose) so it never pollutes the
    // (root) bucket; both declared files land loose under src/ via "../".
    writeFiles(positive, {
      "src/inner/decls.rs": [
        "#[cfg(test)]",
        "#[path = \"../loose_a.rs\"]",
        "mod loose_a;",
        "#[cfg(test)]",
        "#[path = \"../loose_b.rs\"]",
        "mod loose_b;",
      ].join("\n"),
      "src/loose_a.rs": "// loose, test-only\n",
      "src/loose_b.rs": "// loose, test-only\n",
    });
    const outPositive = convert({ lcovText: "", root: "src", cwd: positive });
    assert.ok(outPositive.testOnlyDirs.includes("src/(root)"), outPositive.testOnlyDirs.join(","));

    writeFiles(negative, {
      "src/inner/decls.rs": ["#[cfg(test)]", "#[path = \"../loose_a.rs\"]", "mod loose_a;"].join("\n"),
      "src/loose_a.rs": "// loose, test-only\n",
      "src/loose_prod.rs": "pub fn prod() {}\n", // loose, production — keeps the bucket mixed
    });
    const outNegative = convert({ lcovText: "", root: "src", cwd: negative });
    assert.ok(!outNegative.testOnlyDirs.includes("src/(root)"), outNegative.testOnlyDirs.join(","));
  } finally {
    cleanup(positive);
    cleanup(negative);
  }
});

// ---- 8. relocation invariant --------------------------------------------------

test("relocation invariant: moving inline tests to a #[path]-declared sibling file doesn't change production numbers", () => {
  const dirA = tmpTree();
  const dirB = tmpTree();
  try {
    writeFiles(dirA, {
      "src/a.rs": [
        "pub fn add(a: i32, b: i32) -> i32 {",
        "    a + b",
        "}",
        "",
        "#[cfg(test)]",
        "mod tests {",
        "    use super::*;",
        "",
        "    #[test]",
        "    fn it_adds() {",
        "        assert_eq!(add(1, 2), 3);",
        "    }",
        "}",
      ].join("\n"),
    });
    const lcovA = lcovRecord("src/a.rs", [
      [1, 5],
      [2, 5],
      [10, 3],
      [11, 3],
    ]);

    writeFiles(dirB, {
      "src/a.rs": [
        "pub fn add(a: i32, b: i32) -> i32 {",
        "    a + b",
        "}",
        "",
        "#[cfg(test)]",
        "#[path = \"a_tests.rs\"]",
        "mod a_tests;",
      ].join("\n"),
      "src/a_tests.rs": ["use super::add;", "", "#[test]", "fn it_adds() {", "    assert_eq!(add(1, 2), 3);", "}"].join("\n"),
    });
    const lcovB = [lcovRecord("src/a.rs", [[1, 5], [2, 5]]), lcovRecord("src/a_tests.rs", [[4, 3], [5, 3]])].join("\n");

    const outA = convert({ lcovText: lcovA, root: "src", cwd: dirA });
    const outB = convert({ lcovText: lcovB, root: "src", cwd: dirB });

    assert.deepEqual(outA.data[0].totals.lines, outB.data[0].totals.lines);
    const aFileA = outA.data[0].files.find((f) => f.filename === "src/a.rs");
    const aFileB = outB.data[0].files.find((f) => f.filename === "src/a.rs");
    assert.deepEqual(aFileA.summary.lines, aFileB.summary.lines);
    assert.equal(
      outB.data[0].files.find((f) => f.filename === "src/a_tests.rs"),
      undefined,
    );
  } finally {
    cleanup(dirA);
    cleanup(dirB);
  }
});

// ---- 9. real-tree guard --------------------------------------------------------

test("real tree: testOnlyFiles and cfgTestSpans complete with zero errors over src-tauri/src", () => {
  const root = path.join(REPO_ROOT, "src-tauri/src");
  const { testOnly, errors } = testOnlyFiles(root);
  assert.deepEqual(errors, []);
  assert.ok(testOnly.size > 0);
  // Silent non-detection (a fail-open bug) must redden here: a real count
  // floor, plus two known test-only files, plus at least one real inline span.
  assert.ok(testOnly.size >= 90, `expected >= 90 test-only files, got ${testOnly.size}`);
  const rels = [...testOnly].map((p) => path.relative(REPO_ROOT, p));
  assert.ok(rels.includes("src-tauri/src/test_support.rs"), rels.join(","));
  assert.ok(rels.includes(path.join("src-tauri", "src", "storage", "tests", "mod.rs")), rels.join(","));

  function walk(d, out = []) {
    for (const entry of readdirSync(d)) {
      const p = path.join(d, entry);
      if (statSync(p).isDirectory()) walk(p, out);
      else if (p.endsWith(".rs")) out.push(p);
    }
    return out;
  }
  for (const f of walk(root)) {
    assert.doesNotThrow(() => cfgTestSpans(f, readFileSync(f, "utf8")), f);
  }
  const libRs = path.join(root, "lib.rs");
  const libSpans = cfgTestSpans(libRs, readFileSync(libRs, "utf8"));
  assert.ok(libSpans.length >= 1, "expected at least one #[cfg(test)] span in lib.rs");
});

// ---- P1: comment/string content is not code — attribute/mod-decl detection ---

test("P1: an inline #[cfg(test)] hidden inside a block comment creates no span; production lines are untouched", () => {
  const src = ["/*", "#[cfg(test)]", "*/", "pub fn production() {", '    println!("prod");', "}"].join("\n");
  assert.deepEqual(cfgTestSpans("f.rs", src), []);
});

test("P1: #[cfg(test)] mod x; hidden inside a block comment does not make x.rs test-only", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": ["/*", "#[cfg(test)]", "mod helper;", "*/", "pub fn production() {}"].join("\n"),
      "src/helper.rs": "// would be test-only if the comment were code\n",
    });
    const { testOnly, errors } = testOnlyFiles(path.join(dir, "src"));
    assert.deepEqual(errors, []);
    const rels = [...testOnly].map((p) => path.relative(dir, p));
    assert.ok(!rels.includes("src/helper.rs"), rels.join(","));
  } finally {
    cleanup(dir);
  }
});

test("P1: a raw string containing #[cfg(test)] on its own line does not create a span", () => {
  const src = ['let s = r#"', "#[cfg(test)]", '"#;'].join("\n");
  assert.deepEqual(cfgTestSpans("f.rs", src), []);
});

// ---- P2: SF: resolved by root marker, not cwd-relative-only -------------------

test("P2: an SF: path with a foreign absolute workspace prefix resolves via the root marker (like coverage-ratchet's afterMarker), not as out-of-tree raw pass-through", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/a.rs": [
        "pub fn add(a: i32, b: i32) -> i32 {",
        "    a + b",
        "}",
        "",
        "#[cfg(test)]",
        "mod tests {",
        "    #[test]",
        "    fn it_adds() { assert_eq!(add(1, 2), 3); }",
        "}",
      ].join("\n"),
    });
    const das = [
      [1, 5],
      [2, 5],
      [7, 3],
      [8, 3],
    ];
    const localLcov = lcovRecord("src/a.rs", das);
    const foreignLcov = lcovRecord("/home/runner/work/brawler/brawler/src/a.rs", das);

    const outLocal = convert({ lcovText: localLcov, root: "src", cwd: dir });
    const outForeign = convert({ lcovText: foreignLcov, root: "src", cwd: dir });

    assert.deepEqual(outForeign.data[0].totals.lines, outLocal.data[0].totals.lines);
    assert.deepEqual(outLocal.data[0].totals.lines, { count: 2, covered: 2, percent: 100 });
  } finally {
    cleanup(dir);
  }
});

// ---- P3: const fn is a fn item (class2), not a `;`-terminated class1 item ----

test("P3: const fn is class2 (ends at its own closing brace); a following const item and production fn survive", () => {
  const src = ["#[cfg(test)]", "const fn helper() -> u8 {", "    1", "}", "const PROD: u8 = 2;", "pub fn production() {", "    2", "}"].join(
    "\n",
  );
  assert.deepEqual(cfgTestSpans("f.rs", src), [[1, 4]]);

  const dir = tmpTree();
  try {
    writeFiles(dir, { "src/a.rs": src });
    const lcovText = lcovRecord("src/a.rs", [
      [3, 5], // inside the const fn helper's body — must be stripped
      [7, 2], // production fn body — must survive with exact counts
    ]);
    const out = convert({ lcovText, root: "src", cwd: dir });
    const a = out.data[0].files.find((f) => f.filename === "src/a.rs");
    assert.deepEqual(a.summary.lines, { count: 1, covered: 1, percent: 100 });
  } finally {
    cleanup(dir);
  }
});

// ---- P4: unsupported cfg/cfg_attr forms fail closed ---------------------------

test("self-check 8: a multi-line #[cfg(...)] attribute is an unsupported form", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": ["#[cfg(", "    test", ")]", "fn f() {}"].join("\n"),
      "rust.lcov": lcovRecord("src/lib.rs", []),
    });
    const { r, outPath } = selfCheckHarness(dir, {});
    assert.equal(r.status, 1);
    assert.match(r.stderr, /unsupported cfg attribute form/);
    assert.equal(existsSync(outPath), false);
  } finally {
    cleanup(dir);
  }
});

test("self-check 9: #[cfg(all(test, feature = \"x\"))] is an unsupported form", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": ['#[cfg(all(test, feature = "x"))]', "fn f() {}"].join("\n"),
      "rust.lcov": lcovRecord("src/lib.rs", []),
    });
    const { r, outPath } = selfCheckHarness(dir, {});
    assert.equal(r.status, 1);
    assert.match(r.stderr, /unsupported cfg attribute form/);
    assert.equal(existsSync(outPath), false);
  } finally {
    cleanup(dir);
  }
});

test("P4: #[cfg(not(test))] is recognized — no error, no span", () => {
  const src = ["#[cfg(not(test))]", "fn f() {}"].join("\n");
  assert.doesNotThrow(() => cfgTestSpans("f.rs", src));
  assert.deepEqual(cfgTestSpans("f.rs", src), []);
});

test("self-check 10: #[cfg_attr(test, path = \"alternate.rs\")] is an unsupported form", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": ["#[cfg(test)]", '#[cfg_attr(test, path = "alternate.rs")]', "mod helper;"].join("\n"),
      "src/helper.rs": "// production\n",
      "src/alternate.rs": "// the real cfg_attr target\n",
      "rust.lcov": lcovRecord("src/lib.rs", []),
    });
    const { r, outPath } = selfCheckHarness(dir, {});
    assert.equal(r.status, 1);
    assert.match(r.stderr, /unsupported: cfg_attr\(test, path/);
    assert.equal(existsSync(outPath), false);
  } finally {
    cleanup(dir);
  }
});

// ---- P5: indented declarations inside inline modules; bin/ crate roots -------

test("self-check 11: an external mod declared inside a production inline module is unsupported", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": ["#[cfg(test)]", "mod shared;", "", "pub mod wrapper {", '    #[path = "../shared.rs"]', "    mod shared;", "}"].join(
        "\n",
      ),
      "src/shared.rs": "// shared\n",
      "rust.lcov": lcovRecord("src/lib.rs", []),
    });
    const { r, outPath } = selfCheckHarness(dir, {});
    assert.equal(r.status, 1);
    assert.match(r.stderr, /unsupported: external module declared inside an inline module/);
    assert.match(r.stderr, /src[/\\]lib\.rs:6/);
    assert.equal(existsSync(outPath), false);
  } finally {
    cleanup(dir);
  }
});

test("P5: a binary crate root (src/bin/tool.rs) resolves its declared modules under bin/, not bin/tool/", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/bin/tool.rs": ["#[cfg(test)]", "mod helpers;"].join("\n"),
      "src/bin/helpers.rs": "// sibling of tool.rs under bin/, not bin/tool/helpers.rs\n",
    });
    const { testOnly, errors } = testOnlyFiles(path.join(dir, "src"));
    assert.deepEqual(errors, []);
    const rels = [...testOnly].map((p) => path.relative(dir, p)).sort();
    assert.deepEqual(rels, ["src/bin/helpers.rs"]);
  } finally {
    cleanup(dir);
  }
});

test("self-check 12: a multi-line #[cfg_attr(...)] attribute is an unsupported form (astra r2 #4)", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": ["#[cfg(test)]", "#[cfg_attr(", "    test,", '    path = "alternate.rs"', ")]", "mod helper;"].join("\n"),
      "src/helper.rs": "// decoy\n",
      "src/alternate.rs": "// the real test file\n",
      "rust.lcov": lcovRecord("src/lib.rs", []),
    });
    const { r, outPath } = selfCheckHarness(dir, {});
    assert.equal(r.status, 1);
    assert.match(r.stderr, /unsupported: cfg_attr\(test, path/);
    assert.match(r.stderr, /src[/\\]lib\.rs:2/);
    assert.equal(existsSync(outPath), false);
  } finally {
    cleanup(dir);
  }
});

test("self-check 13: #[cfg (test)] (whitespace variant) is an unsupported form, not a silent no-span (astra r2 #4)", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": ["#[cfg (test)]", "mod tests {", "    fn t() {}", "}"].join("\n"),
      "rust.lcov": lcovRecord("src/lib.rs", [[3, 1]]),
    });
    const { r, outPath } = selfCheckHarness(dir, {});
    assert.equal(r.status, 1);
    assert.match(r.stderr, /unsupported cfg attribute form/);
    assert.match(r.stderr, /src[/\\]lib\.rs:1/);
    assert.equal(existsSync(outPath), false);
  } finally {
    cleanup(dir);
  }
});

test("P5b: an ordinary module directory named bin deeper in the tree is not a crate root (astra r2 #5)", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": "mod domain;\n",
      "src/domain/mod.rs": "mod bin;\n",
      "src/domain/bin/mod.rs": ["#[cfg(test)]", "mod helper;"].join("\n"),
      "src/domain/bin/helper.rs": "mod child;\n",
      "src/domain/bin/helper/child.rs": "// test helper code\n",
    });
    const { testOnly, errors } = testOnlyFiles(path.join(dir, "src"));
    assert.deepEqual(errors, []);
    const rels = [...testOnly].map((p) => path.relative(dir, p)).sort();
    assert.deepEqual(rels, ["src/domain/bin/helper.rs", "src/domain/bin/helper/child.rs"]);
  } finally {
    cleanup(dir);
  }
});

test("self-check 14: a `)]` inside a comment does not end a cfg_attr early (astra r3 #4)", () => {
  const dir = tmpTree();
  try {
    writeFiles(dir, {
      "src/lib.rs": ["#[cfg(test)]", "#[cfg_attr( /* )] */", "    test,", '    path = "alternate.rs"', ")]", "mod helper;"].join("\n"),
      "src/helper.rs": "// decoy\n",
      "src/alternate.rs": "// the real test file\n",
      "rust.lcov": lcovRecord("src/lib.rs", []),
    });
    const { r, outPath } = selfCheckHarness(dir, {});
    assert.equal(r.status, 1);
    assert.match(r.stderr, /unsupported: cfg_attr\(test, path/);
    assert.match(r.stderr, /src[/\\]lib\.rs:2/);
    assert.equal(existsSync(outPath), false);
  } finally {
    cleanup(dir);
  }
});

// ---- structuralTokens: a couple of direct sanity checks -----------------------

test("structuralTokens ignores brackets inside strings/comments and treats lifetimes as non-char", () => {
  const toks = structuralTokens('fn f() { let s = "{}"; let lt = \'a; }');
  const chars = toks.map((t) => t.ch).join("");
  // "{}" inside the string produces no tokens; both `;` statement
  // terminators and the outer fn braces are real structural tokens; the
  // lifetime `'a` consumes only its quote, no token, no false char literal.
  assert.equal(chars, "(){;;}");
});
