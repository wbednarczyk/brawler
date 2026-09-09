#!/usr/bin/env node
// Converts cargo-llvm-cov's --lcov output into a production-only coverage
// summary in the shape coverage-ratchet.mjs already reads (data[0].files[]/
// totals). Rust TEST CODE — a top-level #[cfg(test)] item (attribute + its
// item, item-aware end detection) and any file reachable only via a
// #[cfg(test)] mod declaration (transitively) — is stripped from the DA line
// counts, so moving tests between files never changes a coverage number. A
// NESTED (indented) #[cfg(test)] inside a fn/impl/struct body is NOT stripped
// (it counts as production — a documented, narrow limitation: ~14 small seams
// today; see docs/testing.md § Coverage ratchet). Only single-line
// #[cfg(test)]/#[cfg(not(test))] and non-test cfg_attr forms are recognized;
// anything else that plausibly gates on `test` fails closed rather than
// silently mis-detecting. An SF: path resolves against `root` by marker (like
// coverage-ratchet.mjs's afterMarker), so a foreign absolute workspace prefix
// (a CI artifact's lcov) still gets production-only filtering.
// measurement 2 = physical DA lines after exclusion (#488).

import { readFileSync, readdirSync, statSync, existsSync, writeFileSync } from "node:fs";
import path from "node:path";

const BRACKETS_OPEN = "([{";
const BRACKETS_CLOSE = ")]}";
const STRUCTURAL = "(){}[];";
const MOD_DECL_RE = /^(?:pub(?:\((?:crate|super|self|in\s+[\w:]+)\))?\s+)?mod\s+(\w+)\s*;\s*$/;
const PATH_ATTR_RE = /^#\[path\s*=\s*"([^"]+)"\]$/;

function loc(file, line) {
  return line === undefined ? ` (${file})` : ` (${file}:${line})`;
}

// ---- tokenizer --------------------------------------------------------------
// Emits one token per structural bracket/semicolon character, skipping
// comments/strings/char-lifetime ambiguity per the language grammar. Throws
// (message + .line) on an unterminated comment/string/raw-string.

function structuralTokens(src) {
  const out = [];
  const codeLines = new Set();
  let i = 0;
  let line = 1;
  const n = src.length;
  while (i < n) {
    const c = src[i];
    const d = src[i + 1];
    if (c === "\n") {
      line++;
      i++;
      continue;
    }
    if (c === "/" && d === "/") {
      while (i < n && src[i] !== "\n") i++;
      continue;
    }
    if (c === "/" && d === "*") {
      const startLine = line;
      let depth = 1;
      i += 2;
      while (i < n && depth > 0) {
        if (src[i] === "/" && src[i + 1] === "*") {
          depth++;
          i += 2;
          continue;
        }
        if (src[i] === "*" && src[i + 1] === "/") {
          depth--;
          i += 2;
          continue;
        }
        if (src[i] === "\n") line++;
        i++;
      }
      if (depth > 0) {
        const err = new Error("lexical failure: unterminated block comment");
        err.line = startLine;
        throw err;
      }
      continue;
    }
    if (c === "r" || (c === "b" && d === "r")) {
      const j0 = i + (c === "b" ? 2 : 1);
      let j = j0;
      let hashes = 0;
      while (src[j] === "#") {
        hashes++;
        j++;
      }
      if (src[j] === '"' && (i === 0 || !/[A-Za-z0-9_]/.test(src[i - 1]))) {
        const startLine = line;
        const close = '"' + "#".repeat(hashes);
        const k = src.indexOf(close, j + 1);
        if (k === -1) {
          const err = new Error("lexical failure: unterminated raw string");
          err.line = startLine;
          throw err;
        }
        for (let m = i; m < k + close.length; m++) if (src[m] === "\n") line++;
        i = k + close.length;
        continue;
      }
    }
    if (c === '"') {
      const startLine = line;
      i++;
      let closed = false;
      while (i < n) {
        if (src[i] === "\\") {
          i++;
          if (src[i] === "\n") line++;
          i++;
          continue;
        }
        if (src[i] === '"') {
          i++;
          closed = true;
          break;
        }
        if (src[i] === "\n") line++;
        i++;
      }
      if (!closed) {
        const err = new Error("lexical failure: unterminated string");
        err.line = startLine;
        throw err;
      }
      continue;
    }
    if (c === "'") {
      if (d === "\\") {
        let k = i + 2;
        while (k < n && src[k] !== "'") {
          if (src[k] === "\n") line++;
          k++;
        }
        i = k + 1;
        continue;
      }
      if (src[i + 2] === "'") {
        i += 3;
        continue;
      }
      i++; // lifetime: consume just the quote
      continue;
    }
    if (!/\s/.test(c)) codeLines.add(line);
    if (STRUCTURAL.includes(c)) out.push({ line, ch: c, pos: i });
    i++;
  }
  out.codeLines = codeLines;
  return out;
}

// ---- item classification -----------------------------------------------------
// class1 (static/use/type/extern-crate, and `const` NOT followed by `fn`)
// ends at the first depth-0 `;`. class2 (everything else: fn/impl/mod/struct/
// enum/union/trait/macro items, including `const fn`) ends at the matching
// `}` of its first depth-0 `{`, or at a depth-0 `;` reached before any `{`
// (tuple/unit structs, macro_rules!(...);  etc).

function classifyItem(text) {
  let t = text;
  t = t.replace(/^pub(?:\((?:crate|super|self|in\s+[\w:]+)\))?\s+/, "");
  let sawConst = false;
  for (;;) {
    if (/^const\s+/.test(t)) {
      sawConst = true;
      t = t.replace(/^const\s+/, "");
      continue;
    }
    if (/^unsafe\s+/.test(t)) {
      t = t.replace(/^unsafe\s+/, "");
      continue;
    }
    if (/^async\s+/.test(t)) {
      t = t.replace(/^async\s+/, "");
      continue;
    }
    const externAbi = t.match(/^extern\s+"[^"]*"\s+/);
    if (externAbi) {
      t = t.slice(externAbi[0].length);
      continue;
    }
    break;
  }
  const m = t.match(/^([A-Za-z_][A-Za-z0-9_]*)/);
  const keyword = m ? m[1] : "";
  const kind = sawConst ? (keyword === "fn" ? "class2" : "class1") : ["static", "use", "type", "extern"].includes(keyword) ? "class1" : "class2";
  return { kind, keyword };
}

/** Scan tokens from startTokenIdx for the item's end. Returns
 * { endLine, endTokenIdx, sawOpenBrace, openBraceTokenIdx } or null if the
 * token stream is exhausted before the item closes. */
function scanItemEnd(tokens, startTokenIdx, kind) {
  let depth = 0;
  let sawOpenBrace = false;
  let openBraceTokenIdx = -1;
  for (let t = startTokenIdx; t < tokens.length; t++) {
    const tok = tokens[t];
    if (BRACKETS_OPEN.includes(tok.ch)) {
      if (kind === "class2" && tok.ch === "{" && depth === 0) {
        sawOpenBrace = true;
        openBraceTokenIdx = t;
      }
      depth++;
      continue;
    }
    if (BRACKETS_CLOSE.includes(tok.ch)) {
      depth--;
      if (depth === 0 && kind === "class2" && sawOpenBrace) {
        return { endLine: tok.line, endTokenIdx: t, sawOpenBrace: true, openBraceTokenIdx };
      }
      continue;
    }
    // tok.ch === ";"
    if (depth === 0 && (kind === "class1" || !sawOpenBrace)) {
      return { endLine: tok.line, endTokenIdx: t, sawOpenBrace: false, openBraceTokenIdx: -1 };
    }
  }
  return null;
}

// ---- unsupported cfg/cfg_attr forms: fail closed -----------------------------
// Only `#[cfg(test)]` and `#[cfg(not(test))]`, each on one line, are
// recognized; only `#[cfg_attr(...)]` forms that don't combine `test` with
// `path` are. Anything else that plausibly means "gate on test" is rejected
// rather than silently mis-detected (a multi-line #[cfg(...)], an
// `all(test, ...)`, or a cfg_attr(test, path = "...") redirect).
const TEST_WORD_RE = /\btest\b/;

const CFG_START_RE = /^#\[cfg\s*\(/;
const CFG_ATTR_START_RE = /^#\[cfg_attr\s*\(/;

/** The full text of the column-0 attribute starting at `idx`, joining
 * continuation lines up to the `]` that closes the attribute's opening `[`
 * per the TOKENIZER (so a `)]` inside a comment or string cannot end it
 * early), or null when the bracket never closes. */
function attrText(lines, tokens, idx) {
  const lineNo = idx + 1;
  let t = 0;
  while (t < tokens.length && (tokens[t].line < lineNo || (tokens[t].line === lineNo && tokens[t].ch !== "["))) t++;
  if (t >= tokens.length || tokens[t].line !== lineNo) return null;
  let depth = 0;
  for (; t < tokens.length; t++) {
    const ch = tokens[t].ch;
    if (BRACKETS_OPEN.includes(ch)) depth++;
    else if (BRACKETS_CLOSE.includes(ch) && --depth === 0) return lines.slice(idx, tokens[t].line).join(" ");
  }
  return null;
}

function validateAttrLine(filePath, lines, tokens, idx) {
  const first = lines[idx];
  const isCfg = CFG_START_RE.test(first);
  if (!isCfg && !CFG_ATTR_START_RE.test(first)) return;
  const text = attrText(lines, tokens, idx);
  const lineNo = idx + 1;
  if (text === null) throw new Error(`unterminated attribute${loc(filePath, lineNo)}`);
  if (isCfg) {
    const isExact = text === "#[cfg(test)]" || text === "#[cfg(not(test))]";
    if (!isExact && TEST_WORD_RE.test(text)) {
      throw new Error(`unsupported cfg attribute form (only #[cfg(test)] on one line is recognized)${loc(filePath, lineNo)}`);
    }
    return;
  }
  if (TEST_WORD_RE.test(text) && /\bpath\b/.test(text)) {
    throw new Error(`unsupported: cfg_attr(test, path = …)${loc(filePath, lineNo)}`);
  }
}

// ---- shared attribute/comment skipping ---------------------------------------
// Skips a run of top-level (column-0) `#[...]` attribute lines (bracket-depth
// aware, so a multi-line attribute is handled by the tokenizer, not a regex)
// and `//` comment lines starting at `idx`. Returns the resulting line index
// plus whether a `#[cfg(test)]` line and/or a `#[path = "..."]` line were seen.
// A text-matched `#[cfg(test)]` only counts when the tokenizer confirms the
// line is CODE (not inside a comment/string/raw-string) — a bare text match
// alone (e.g. inside a block comment) is ignored.
function collectLeadingAttrs(lines, tokens, startIdx, filePath) {
  let idx = startIdx;
  let pathAttr = null;
  let cfgTest = false;
  let cursor = 0;
  function firstTokenAtOrAfter(oneBasedLine) {
    while (cursor < tokens.length && tokens[cursor].line < oneBasedLine) cursor++;
    return cursor;
  }
  while (idx < lines.length) {
    const line = lines[idx];
    const isCode = tokens.codeLines.has(idx + 1);
    if (isCode && line.startsWith("#[")) validateAttrLine(filePath, lines, tokens, idx);
    if (isCode && line === "#[cfg(test)]") {
      cfgTest = true;
      idx++;
      continue;
    }
    if (line.startsWith("//")) {
      idx++;
      continue;
    }
    if (isCode && line.startsWith("#[")) {
      const m = line.match(PATH_ATTR_RE);
      if (m) pathAttr = m[1];
      let t = firstTokenAtOrAfter(idx + 1);
      while (t < tokens.length && tokens[t].line === idx + 1 && tokens[t].ch !== "[") t++;
      if (t >= tokens.length || tokens[t].ch !== "[" || tokens[t].line !== idx + 1) {
        idx++; // malformed/no bracket token found — bail out conservatively
        continue;
      }
      let depth = 0;
      let closedAtLine = null;
      for (; t < tokens.length; t++) {
        const ch = tokens[t].ch;
        if (BRACKETS_OPEN.includes(ch)) depth++;
        else if (BRACKETS_CLOSE.includes(ch)) {
          depth--;
          if (depth === 0) {
            closedAtLine = tokens[t].line;
            break;
          }
        }
      }
      idx = closedAtLine ?? lines.length;
      continue;
    }
    break;
  }
  return { idx, pathAttr, cfgTest };
}

// ---- (a) inline #[cfg(test)] spans within one file ---------------------------

function cfgTestSpans(filePath, src) {
  const lines = src.split("\n");
  let tokens;
  try {
    tokens = structuralTokens(src);
  } catch (err) {
    throw new Error(`${err.message}${loc(filePath, err.line)}`);
  }
  const spans = [];
  let cursor = 0;
  function tokenIndexAtOrAfterLine(oneBasedLine) {
    while (cursor < tokens.length && tokens[cursor].line < oneBasedLine) cursor++;
    return cursor;
  }
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!tokens.codeLines.has(i + 1)) continue; // comment/string content, not code
    if (line.startsWith("#[")) validateAttrLine(filePath, lines, tokens, i);
    if (line !== "#[cfg(test)]") continue;
    const attrStartLine = i + 1;
    const { idx: j } = collectLeadingAttrs(lines, tokens, i + 1, filePath);
    if (j >= lines.length) throw new Error(`cfg(test) attribute has no following item${loc(filePath, attrStartLine)}`);
    const itemLine = j + 1;
    const { kind, keyword } = classifyItem(lines[j].trimStart());
    const startTokenIdx = tokenIndexAtOrAfterLine(itemLine);
    const result = scanItemEnd(tokens, startTokenIdx, kind);
    if (!result) throw new Error(`inline span never closes, reached EOF${loc(filePath, itemLine)}`);
    const endLineContent = lines[result.endLine - 1] ?? "";
    const isBraceLine = endLineContent === "}";
    const endsWithSemi = endLineContent.replace(/\s+$/, "").endsWith(";");
    if (!isBraceLine && !endsWithSemi) {
      throw new Error(`span end not at an item boundary${loc(filePath, result.endLine)}`);
    }
    const isExternalModDecl = keyword === "mod" && !result.sawOpenBrace;
    if (isExternalModDecl) continue; // handled entirely by testOnlyFiles()
    if (keyword === "mod" && result.sawOpenBrace) {
      checkNoNestedModDecl(filePath, lines, tokens, result.openBraceTokenIdx, result.endLine);
    }
    spans.push([attrStartLine, result.endLine]);
  }
  return spans;
}

/** Self-check 2: a `mod y;` at the top level of an inline `#[cfg(test)] mod x
 * { ... }` block is unsupported (resolving it would duplicate testOnlyFiles'
 * job for a construct that doesn't exist in this codebase today). */
function checkNoNestedModDecl(filePath, lines, tokens, openBraceTokenIdx, endLine) {
  const openLine = tokens[openBraceTokenIdx].line;
  let depth = 1;
  let ti = openBraceTokenIdx + 1;
  for (let ln = openLine; ln <= endLine; ln++) {
    const depthAtStart = depth;
    while (ti < tokens.length && tokens[ti].line === ln) {
      const ch = tokens[ti].ch;
      if (BRACKETS_OPEN.includes(ch)) depth++;
      else if (BRACKETS_CLOSE.includes(ch)) depth--;
      ti++;
    }
    if (ln === openLine || ln === endLine) continue;
    if (depthAtStart === 1 && MOD_DECL_RE.test(lines[ln - 1].trim())) {
      throw new Error(`unsupported: declare the module externally or inline its code${loc(filePath, ln)}`);
    }
  }
}

// ---- (b) whole test-only files, transitively ---------------------------------

function walk(dir, out = []) {
  for (const entry of readdirSync(dir)) {
    const p = path.join(dir, entry);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (p.endsWith(".rs")) out.push(p);
  }
  return out;
}

function moduleDirFor(f, root) {
  const base = path.basename(f);
  const dir = path.dirname(f);
  // A binary crate root (<root>/bin/tool.rs — cargo's auto-discovered bin
  // targets) resolves its submodules under bin/, like main.rs/lib.rs/mod.rs —
  // not under bin/tool/. An ordinary module directory that merely happens to
  // be named `bin` deeper in the tree is NOT a crate root.
  const isBinCrateRoot = dir === path.join(root, "bin");
  return ["mod.rs", "lib.rs", "main.rs"].includes(base) || isBinCrateRoot ? dir : path.join(dir, base.replace(/\.rs$/, ""));
}

/** Full-tree scan for the module graph: which files are test-only (declared,
 * directly or transitively, only by a #[cfg(test)] mod ...;). Returns
 * { testOnly: Set<path>, errors: string[] } — paths are in whatever form
 * `root` was given (absolute in, absolute out). Self-checks 1 and 3 are
 * collected into `errors` (not thrown) so the whole tree is diagnosed in one
 * pass; a structural/lexical failure (self-check 5) still throws. */
function testOnlyFiles(root) {
  const files = walk(root);
  const fileSet = new Set(files);
  const errors = [];
  const testOnlyDirect = new Set();
  const modDecls = new Map(); // file -> [{ target, isCfgTest, line }]

  for (const f of files) {
    const src = readFileSync(f, "utf8");
    const lines = src.split("\n");
    let tokens;
    try {
      tokens = structuralTokens(src);
    } catch (err) {
      throw new Error(`${err.message}${loc(f, err.line)}`);
    }
    const dir = moduleDirFor(f, root);

    // Self-check: an indented (non-column-0) mod declaration is only
    // supported inside an inline #[cfg(test)] block (already rejected by
    // checkNoNestedModDecl via cfgTestSpans below); anywhere else in an
    // inline module it's unsupported rather than silently invisible to the
    // module graph (none exist in the tree today).
    const spans = cfgTestSpans(f, src);
    for (let ln = 1; ln <= lines.length; ln++) {
      const raw = lines[ln - 1];
      if (!tokens.codeLines.has(ln) || !/^[ \t]/.test(raw)) continue;
      if (!MOD_DECL_RE.test(raw.trim())) continue;
      if (spans.some(([a, b]) => ln >= a && ln <= b)) continue;
      throw new Error(`unsupported: external module declared inside an inline module${loc(f, ln)}`);
    }

    const decls = [];
    let i = 0;
    while (i < lines.length) {
      const { idx: j, pathAttr, cfgTest } = collectLeadingAttrs(lines, tokens, i, f);
      const modMatch = j < lines.length && tokens.codeLines.has(j + 1) ? lines[j].match(MOD_DECL_RE) : null;
      if (!modMatch) {
        i = j > i ? j : i + 1;
        continue;
      }
      const name = modMatch[1];
      const candidates = pathAttr
        ? [path.join(path.dirname(f), pathAttr)]
        : [path.join(dir, `${name}.rs`), path.join(dir, name, "mod.rs")];
      const target = candidates.find((c) => fileSet.has(c));
      if (cfgTest) {
        if (!target) {
          errors.push(`#[cfg(test)] mod ${name}; resolves to no existing file — tried: ${candidates.join(", ")}${loc(f, j + 1)}`);
        } else {
          testOnlyDirect.add(target);
          decls.push({ target, isCfgTest: true, line: j + 1 });
        }
      } else if (target) {
        decls.push({ target, isCfgTest: false, line: j + 1 });
      }
      i = j + 1;
    }
    modDecls.set(f, decls);
  }

  // Transitive fixpoint: a plain (non-cfg-test) mod declared inside a
  // test-only file is also test-only.
  const testOnly = new Set(testOnlyDirect);
  let changed = true;
  while (changed) {
    changed = false;
    for (const f of testOnly) {
      for (const d of modDecls.get(f) ?? []) {
        if (!d.isCfgTest && !testOnly.has(d.target)) {
          testOnly.add(d.target);
          changed = true;
        }
      }
    }
  }

  // Self-check 3: a file reachable from both a test-only declaration and a
  // production (non-test-only-file) plain mod declaration is ambiguous.
  for (const [f, decls] of modDecls) {
    if (testOnly.has(f)) continue; // f itself is test-only — its decls are legitimately transitive
    for (const d of decls) {
      if (!d.isCfgTest && testOnly.has(d.target)) {
        errors.push(`${d.target} is reachable from both a test-only declaration and a production mod declaration — ambiguous${loc(f, d.line)}`);
      }
    }
  }

  return { testOnly, errors };
}

// ---- lcov parsing + conversion ------------------------------------------------

function parseLcov(text) {
  const records = [];
  let sf = null;
  let das = [];
  for (const rawLine of text.split("\n")) {
    if (rawLine.startsWith("SF:")) {
      sf = rawLine.slice(3);
      das = [];
    } else if (rawLine.startsWith("DA:")) {
      const parts = rawLine.slice(3).split(",");
      das.push([Number(parts[0]), Number(parts[1])]);
    } else if (rawLine === "end_of_record") {
      if (sf !== null) records.push({ sf, das });
      sf = null;
      das = [];
    }
  }
  return records;
}

function lineSummary(covered, count) {
  return { lines: { count, covered, percent: count === 0 ? 0 : (covered / count) * 100 } };
}

/** The suffix of `rawPath` starting right after `marker`, whether rawPath is
 * already relative (starts with marker) or absolute with a FOREIGN prefix
 * (e.g. a CI artifact's lcov produced under a different checkout root) —
 * boundary-safe, same idiom as coverage-ratchet.mjs's afterMarker. Returns
 * null if marker isn't found at a path-segment boundary (truly out-of-tree). */
function afterMarker(rawPath, marker) {
  const p = rawPath.replace(/\\/g, "/");
  const m = marker.replace(/\\/g, "/");
  if (p === m) return "";
  if (p.startsWith(`${m}/`)) return p.slice(m.length + 1);
  const idx = p.indexOf(`/${m}/`);
  return idx === -1 ? null : p.slice(idx + m.length + 2);
}

function computeTestOnlyDirs(root, absRoot, testOnlyAbsSet) {
  const byDir = new Map();
  for (const f of walk(absRoot)) {
    const rel = path.relative(absRoot, f);
    const slash = rel.indexOf(path.sep);
    const key = slash === -1 ? `${root}/(root)` : `${root}/${rel.slice(0, slash)}`;
    const entry = byDir.get(key) ?? { total: 0, testOnly: 0 };
    entry.total++;
    if (testOnlyAbsSet.has(f)) entry.testOnly++;
    byDir.set(key, entry);
  }
  const dirs = [];
  for (const [key, e] of byDir) if (e.total > 0 && e.total === e.testOnly) dirs.push(key);
  return dirs.sort();
}

function convert({ lcovText, root, cwd }) {
  const absRoot = path.resolve(cwd, root);
  const { testOnly: testOnlyAbs, errors } = testOnlyFiles(absRoot);
  if (errors.length > 0) throw new Error(errors.join("\n"));
  const testOnlyRel = new Set([...testOnlyAbs].map((p) => path.relative(cwd, p)));
  const rootRel = path.relative(cwd, absRoot);

  const filesOut = [];
  const spanCache = new Map();
  let totC = 0;
  let totT = 0;

  for (const rec of parseLcov(lcovText)) {
    let sfAbs = path.isAbsolute(rec.sf) ? rec.sf : path.resolve(cwd, rec.sf);
    let rel = path.relative(cwd, sfAbs);
    let inTree = rel === rootRel || rel.startsWith(rootRel + path.sep);

    if (!inTree) {
      // Not in-tree relative to OUR cwd — but a foreign absolute workspace
      // prefix (a CI artifact's lcov) still names an in-tree file once the
      // root marker is found at a segment boundary; resolve it locally
      // instead of treating it as pass-through raw numbers.
      const suffix = afterMarker(rec.sf, root);
      if (suffix !== null) {
        sfAbs = path.resolve(absRoot, suffix);
        rel = path.relative(cwd, sfAbs);
        inTree = true;
      }
    }

    if (!inTree) {
      const t = rec.das.length;
      const c = rec.das.filter(([, cnt]) => cnt > 0).length;
      totT += t;
      totC += c;
      if (t > 0) filesOut.push({ filename: rec.sf, summary: lineSummary(c, t) });
      continue;
    }
    if (!existsSync(sfAbs)) throw new Error(`stale lcov: ${rel} does not exist on disk`);
    if (testOnlyRel.has(rel)) continue;

    let spans = spanCache.get(rel);
    if (spans === undefined) {
      const src = readFileSync(sfAbs, "utf8");
      spans = cfgTestSpans(rel, src);
      spanCache.set(rel, spans);
    }
    const keep = rec.das.filter(([ln]) => !spans.some(([a, b]) => ln >= a && ln <= b));
    const t = keep.length;
    const c = keep.filter(([, cnt]) => cnt > 0).length;
    totT += t;
    totC += c;
    if (t > 0) filesOut.push({ filename: rec.sf, summary: lineSummary(c, t) });
  }

  return {
    measurement: 2,
    testOnlyDirs: computeTestOnlyDirs(root, absRoot, testOnlyAbs),
    data: [{ files: filesOut, totals: { lines: { count: totT, covered: totC, percent: totT === 0 ? 0 : (totC / totT) * 100 } } }],
    type: "rust-coverage-summary",
    version: "2",
  };
}

// ---- CLI ----------------------------------------------------------------------

function parseArgs(argv) {
  const opts = { lcov: "coverage/rust.lcov", root: "src-tauri/src", out: "coverage/rust-summary.json" };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const eq = a.match(/^--(lcov|root|out)=(.*)$/);
    if (eq) {
      opts[eq[1]] = eq[2];
      continue;
    }
    const flag = a.match(/^--(lcov|root|out)$/);
    if (flag) {
      opts[flag[1]] = argv[++i];
    }
  }
  return opts;
}

function main() {
  const opts = parseArgs(process.argv.slice(2));
  try {
    const lcovText = readFileSync(opts.lcov, "utf8");
    const output = convert({ lcovText, root: opts.root, cwd: process.cwd() });
    writeFileSync(opts.out, `${JSON.stringify(output, null, 2)}\n`);
    console.log(
      `rust-coverage-summary: wrote ${opts.out} (${output.data[0].files.length} files, ${output.data[0].totals.lines.percent.toFixed(2)}% global)`,
    );
  } catch (err) {
    console.error(`rust-coverage-summary: ${err.message}`);
    process.exit(1);
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main();
}

export { structuralTokens, cfgTestSpans, testOnlyFiles, convert };
