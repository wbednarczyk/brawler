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
function splitSegments(cmd) {
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
      out += " __quoted__ ";
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
function classifyTokens(tokens) {
  const [t0, t1, t2] = tokens;
  if (!t0) return null;
  const rest = (from) => tokens.slice(from).filter((t) => !t.startsWith("-") && t !== "__quoted__");
  const scopedJs = (from) => rest(from).length > 0;
  switch (t0) {
    case "cargo":
      return /^(build|test|nextest|clippy|llvm-cov|check|mutants|bench|run)$/.test(t1 ?? "") ? "cargo" : null;
    case "cargo-nextest":
      return "cargo";
    case "nextest":
      return t1 === "run" ? "cargo" : null;
    case "vitest":
      return t1 === "run" ? (scopedJs(2) ? null : "full") : scopedJs(1) ? null : "full";
    case "playwright":
      return t1 === "test" ? (scopedJs(2) ? null : "full") : null;
    case "yarn":
      return /^(test|build)$/.test(t1 ?? "") ? "full" : null;
    case "npm":
      if (t1 === "test") return scopedJs(2) ? null : "full";
      if (t1 === "run" && NPM_RUN_HEAVY_RE.test(t2 ?? "")) return t2.startsWith("test") && scopedJs(3) ? null : "full";
      return null;
    case "npx":
      if (t1 === "vitest") return t2 === "run" ? (scopedJs(3) ? null : "full") : scopedJs(2) ? null : "full";
      if (t1 === "playwright") return t2 === "test" ? (scopedJs(3) ? null : "full") : null;
      return null;
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
      if (m[1] === "vitest") return t1 === "run" ? (scopedJs(2) ? null : "full") : scopedJs(1) ? null : "full";
      return t1 === "test" ? (scopedJs(2) ? null : "full") : null;
    }
  }
}

/** The heaviest class among the command's segments: "cargo" > "full" > null. */
export function classifyCommand(cmd) {
  let worst = null;
  for (const segment of splitSegments(cmd)) {
    const tokens = stripQuotes(segment).trim().split(/\s+/).filter(Boolean);
    const c = classifyTokens(stripPrefixes(tokens));
    if (c === "cargo") return "cargo";
    if (c === "full") worst = "full";
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
