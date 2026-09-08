#!/usr/bin/env node
// Command classifier for one-heavy-build.sh (ADR 0045 harvest, B3 fix,
// 2026-09-08). Extracted to JS because getting quote-stripping + operator
// splitting right in bash regex is exactly the kind of hand-rolled parsing
// this repo's own guards (source_tree_guards.rs) warn is "heuristic, not a
// parser" — safer to write once here than reinvent in POSIX regex.
//
// Precision contract: classify a Bash command as "heavy" (starts a
// cargo/nextest/vitest/playwright/make-check/npm-check build or test run)
// by looking ONLY at the head tokens of each `;`/`&&`/`||`/`|`/newline
// segment, after stripping quoted strings and leading `env VAR=val…`, `rtk`,
// `nix develop … -c`, and `cd <dir>` prefixes. A heavy keyword that only
// appears inside a quoted string (a commit message, a --body value) must
// never trip this — that was the exact B3 false-positive class.

/** Split `cmd` into segments on unquoted `;`, `&&`, `||`, `|`, and newlines. */
export function splitSegments(cmd) {
  const segments = [];
  let cur = "";
  let quote = null;
  for (let i = 0; i < cmd.length; i++) {
    const c = cmd[i];
    // Backslash escapes: outside single quotes the next char is literal
    // (`\"` never opens/closes a string, `\;` never splits).
    if (c === "\\" && quote !== "'" && i + 1 < cmd.length) {
      cur += c + cmd[i + 1];
      i++;
      continue;
    }
    if (quote) {
      cur += c;
      if (c === quote) quote = null;
      continue;
    }
    if (c === '"' || c === "'") {
      quote = c;
      cur += c;
      continue;
    }
    if (c === "\n" || c === ";") {
      segments.push(cur);
      cur = "";
      continue;
    }
    if (c === "&") {
      segments.push(cur);
      cur = "";
      if (cmd[i + 1] === "&") i++;
      continue;
    }
    if (c === "|") {
      segments.push(cur);
      cur = "";
      if (cmd[i + 1] === "|") i++;
      continue;
    }
    cur += c;
  }
  segments.push(cur);
  return segments;
}

/** Drop the content of every quoted string (a heavy word inside one is not a command). */
function stripQuotes(segment) {
  let out = "";
  let quote = null;
  for (let i = 0; i < segment.length; i++) {
    const c = segment[i];
    if (c === "\\" && quote !== "'" && i + 1 < segment.length) {
      // an escaped char is never a quote delimiter; keep it opaque
      out += quote ? "" : "_";
      i++;
      continue;
    }
    if (quote) {
      if (c === quote) quote = null;
      continue;
    }
    if (c === '"' || c === "'") {
      quote = c;
      out += "__quoted__";
      continue;
    }
    out += c;
  }
  return out;
}

const ASSIGNMENT_RE = /^[A-Za-z_][A-Za-z0-9_]*=/;

/** Strip leading `env VAR=val…`, `rtk`, `nix develop … -c`, and `cd <dir>` prefixes. */
function stripPrefixes(tokens) {
  tokens = tokens.slice();
  for (let guard = 0; guard < 10; guard++) {
    let changed = false;
    if (tokens[0] === "env") {
      tokens.shift();
      changed = true;
    }
    while (tokens.length && ASSIGNMENT_RE.test(tokens[0])) {
      tokens.shift();
      changed = true;
    }
    if (tokens[0] === "rtk") {
      tokens.shift();
      if (tokens[0] === "proxy") tokens.shift();
      changed = true;
    }
    // `timeout [opts] <duration> cmd`, `nice [-n N] cmd`, `time cmd`: transparent wrappers.
    if (tokens[0] === "timeout") {
      tokens.shift();
      while (tokens.length && tokens[0].startsWith("-")) {
        if (/^(-s|-k|--signal|--kill-after)$/.test(tokens[0])) tokens.shift();
        tokens.shift();
      }
      tokens.shift(); // the duration
      changed = true;
    }
    if (tokens[0] === "nice") {
      tokens.shift();
      if (tokens[0] === "-n") tokens.splice(0, 2);
      else if (/^-\d+$/.test(tokens[0] ?? "")) tokens.shift();
      changed = true;
    }
    if (tokens[0] === "time") {
      tokens.shift();
      changed = true;
    }
    if (tokens[0] === "nix") {
      const cIdx = tokens.indexOf("-c");
      if (cIdx !== -1) {
        tokens = tokens.slice(cIdx + 1);
        changed = true;
      }
    }
    if (tokens[0] === "cd" && tokens.length >= 2) {
      tokens = tokens.slice(2);
      changed = true;
    }
    if (!changed) break;
  }
  return tokens;
}

const MAKE_HEAVY_TARGET_RE = /^(check|test|ui-smoke|coverage|build|tauri-build|package-)/;
const NPM_RUN_HEAVY_RE = /^(test|check|build|coverage)/;
const BIN_HEAVY_RE = /^\.?\/?node_modules\/\.bin\/(vitest|playwright)$/;

/**
 * Classify a command's head tokens (post-prefix-stripping):
 *  - "cargo": any cargo build/test/clippy/… — compiles the crate (the OOM
 *    class is two rustc builds at once), denied only while cargo/rustc is alive;
 *  - "full": an UNSCOPED JS suite or a composite target (`vitest` with no file,
 *    `playwright test` with no spec, `make check*`/coverage/build, `npm test`,
 *    `npm run check|build|coverage`, `yarn test|build`) — denied while anything
 *    heavy is alive;
 *  - null: light — including SCOPED vitest/playwright runs (a file or pattern
 *    argument), which may run alongside anything (owner 2026-09-08).
 */
// JS runner options that take a VALUE (the value is not a scope argument).
const VALUE_OPTIONS = new Set([
  "--workers", "-w", "--maxWorkers", "--max-workers", "--minWorkers", "--project", "--reporter",
  "--retries", "--timeout", "--shard", "--config", "-c", "--root", "--dir", "--outputDir", "--output",
  "--pool", "--poolOptions", "--coverage.provider", "--environment", "--browser", "--repeat-each",
]);
// Options that NARROW a run to a subset — they count as scope.
const SCOPE_OPTIONS = new Set(["--grep", "-g", "-t", "--testNamePattern", "--testPathPattern", "--grep-invert", "--last-failed", "--only-failed", "--changed", "--related"]);

/** Whether the args after a JS runner's subcommand narrow the run (a file/pattern/grep). */
function hasScope(args) {
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === "--") continue;
    const eq = a.indexOf("=");
    const name = eq === -1 ? a : a.slice(0, eq);
    if (SCOPE_OPTIONS.has(name)) return true;
    if (VALUE_OPTIONS.has(name)) {
      if (eq === -1) i++; // consume the value
      continue;
    }
    if (a.startsWith("-")) continue; // flag without scope meaning
    return true; // positional (file, pattern, quoted name) = scope
  }
  return false;
}

// Cargo global options that take a value; other `-…` tokens and `+toolchain` are flags.
const CARGO_VALUE_OPTIONS = new Set(["--manifest-path", "--config", "-Z", "--color", "-C", "--target-dir", "-j", "--jobs"]);
function cargoSubcommand(tokens) {
  for (let i = 1; i < tokens.length; i++) {
    const t = tokens[i];
    if (t.startsWith("+")) continue;
    const eq = t.indexOf("=");
    const name = eq === -1 ? t : t.slice(0, eq);
    if (CARGO_VALUE_OPTIONS.has(name)) {
      if (eq === -1) i++;
      continue;
    }
    if (t.startsWith("-")) continue;
    return t;
  }
  return "";
}

/**
 * Classify a command's head tokens (post-prefix-stripping):
 *  - "cargo": any cargo build/test/clippy/… — compiles the crate (the OOM
 *    class is two rustc builds at once), denied only while cargo/rustc is alive;
 *  - "full": an UNSCOPED JS suite or a composite target (`vitest` with no file,
 *    `playwright test` with no spec, `make check*`/coverage/build, `npm test`,
 *    `npm run check|build|coverage`, `yarn test|build`) — denied while anything
 *    heavy is alive;
 *  - null: light — including SCOPED vitest/playwright runs (a file, pattern or
 *    grep argument), which may run alongside anything (owner 2026-09-08).
 */
function classifyTokens(tokens) {
  const [t0, t1, t2] = tokens;
  if (!t0) return null;
  const jsRun = (from) => (hasScope(tokens.slice(from)) ? null : "full");
  switch (t0) {
    case "cargo":
      return /^(build|test|nextest|clippy|llvm-cov|check|mutants|bench|run)$/.test(cargoSubcommand(tokens)) ? "cargo" : null;
    case "cargo-nextest":
      return "cargo";
    case "nextest":
      return t1 === "run" ? "cargo" : null;
    case "vitest":
      return t1 === "run" ? jsRun(2) : jsRun(1);
    case "playwright":
      return t1 === "test" ? jsRun(2) : null;
    case "yarn":
      return /^(test|build)$/.test(t1 ?? "") ? "full" : null;
    case "npm":
      if (t1 === "test") return jsRun(2);
      if (t1 === "run" && NPM_RUN_HEAVY_RE.test(t2 ?? "")) return t2.startsWith("test") ? jsRun(3) : "full";
      return null;
    case "npx": {
      // npx flags (`--no-install`, `-y`, `--package x`) may precede the runner.
      let i = 1;
      while (i < tokens.length && tokens[i].startsWith("-")) {
        if (/^(--package|-p)$/.test(tokens[i])) i++;
        i++;
      }
      const runner = tokens[i];
      const next = tokens[i + 1];
      if (runner === "vitest") return next === "run" ? jsRun(i + 2) : jsRun(i + 1);
      if (runner === "playwright") return next === "test" ? jsRun(i + 2) : null;
      return null;
    }
    case "make": {
      let i = 1;
      while (i < tokens.length) {
        if (/^(-j\d+|--jobs=\d+|--directory=.+|-C.+)$/.test(tokens[i])) {
          i += 1;
          continue;
        }
        if (tokens[i] === "-j" || tokens[i] === "-C" || tokens[i] === "--jobs" || tokens[i] === "--directory") {
          i += 2;
          continue;
        }
        break;
      }
      return MAKE_HEAVY_TARGET_RE.test(tokens[i] ?? "") ? "full" : null;
    }
    default: {
      const m = t0.match(BIN_HEAVY_RE);
      if (!m) return null;
      if (m[1] === "vitest") return t1 === "run" ? jsRun(2) : jsRun(1);
      return t1 === "test" ? jsRun(2) : null;
    }
  }
}

/** Put whitespace around redirection operators so `run>/tmp/x` tokenizes like `run > /tmp/x`. */
function spaceOperators(text) {
  return text.replace(/(\d*>>?&?\d*|&>|<)/g, " $1 ");
}

/** Drop shell redirections (`> f`, `>>f`, `2>&1`, `< f`, `&> f`) — never scope. */
function stripRedirections(tokens) {
  const out = [];
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i];
    if (/^(\d*>>?|<|&>|\d*>&\d*)$/.test(t)) {
      if (!/&\d+$/.test(t)) i++; // bare operator: skip its target too
      continue;
    }
    if (/^(\d*>>?|<|&>)[^\s]+$/.test(t)) continue; // attached target: `>file`
    out.push(t);
  }
  return out;
}

/** Quote-aware tokenizer keeping the INNER text of quoted spans (for wrapper bodies). */
export function tokenizeRaw(segment) {
  const tokens = [];
  let cur = "";
  let quote = null;
  let sawQuote = false;
  let inToken = false;
  const flush = () => {
    if (inToken) tokens.push({ text: cur, quoted: sawQuote });
    cur = "";
    sawQuote = false;
    inToken = false;
  };
  for (let i = 0; i < segment.length; i++) {
    const c = segment[i];
    if (c === "\\" && quote !== "'" && i + 1 < segment.length) {
      cur += segment[i + 1];
      inToken = true;
      i++;
      continue;
    }
    if (quote) {
      if (c === quote) quote = null;
      else cur += c;
      continue;
    }
    if (c === '"' || c === "'") {
      quote = c;
      sawQuote = true;
      inToken = true;
      continue;
    }
    if (/\s/.test(c)) {
      flush();
      continue;
    }
    cur += c;
    inToken = true;
  }
  flush();
  return tokens;
}

const SHELL_WRAPPERS = new Set(["bash", "sh", "zsh", "dash"]);

/**
 * A `bash -c "<script>"`-style wrapper (also behind env/rtk/nix/cd prefixes):
 * return the script text so it can be classified as a shell program itself —
 * quoting must not hide a compile (review 2026-09-08).
 */
export function wrappedScript(segment) {
  const raw = tokenizeRaw(segment);
  const texts = stripPrefixes(
    raw.map((t) => {
      if (!t.quoted) return t.text;
      const m = t.text.match(/^([A-Za-z_][A-Za-z0-9_]*=)/);
      return m ? `${m[1]}__quoted__` : "__quoted__";
    }),
  );
  const offset = raw.length - texts.length;
  if (!SHELL_WRAPPERS.has(texts[0] ?? "")) return null;
  for (let i = 1; i < texts.length; i++) {
    const t = texts[i];
    if (/^-[a-zA-Z]*c[a-zA-Z]*$/.test(t)) {
      const body = raw[offset + i + 1];
      return body ? body.text : null;
    }
    if (!t.startsWith("-")) return null; // `bash script.sh` — not an inline body
  }
  return null;
}

/** The strictest class among the command's segments: "full" > "cargo" > null. */
export function classifyCommand(cmd) {
  // "full" dominates "cargo": its denial condition (anything heavy alive) is
  // broader than cargo's (a compile alive) — a chain `cargo test && npm test`
  // must be judged by its strictest member.
  let worst = null;
  for (const segment of splitSegments(cmd)) {
    const inner = wrappedScript(segment);
    const c =
      inner !== null
        ? classifyCommand(inner)
        : classifyTokens(stripPrefixes(stripRedirections(spaceOperators(stripQuotes(segment)).trim().split(/\s+/).filter(Boolean))));
    if (c === "full") return "full";
    if (c === "cargo") worst = "cargo";
  }
  return worst;
}

/** Back-compat: whether `cmd` starts any heavy run. */
export function isHeavyCommand(cmd) {
  return classifyCommand(cmd) !== null;
}

// CLI mode (invoked from one-heavy-build.sh): read the command from stdin,
// exit 0 if heavy, 1 if not.
if (import.meta.url === `file://${process.argv[1]}`) {
  let input = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (chunk) => (input += chunk));
  process.stdin.on("end", () => {
    process.stdout.write(classifyCommand(input) ?? "");
    process.exit(0);
  });
}
