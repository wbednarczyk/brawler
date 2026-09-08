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
  for (const c of segment) {
    if (quote) {
      if (c === quote) quote = null;
      out += " ";
      continue;
    }
    if (c === '"' || c === "'") {
      quote = c;
      out += " ";
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

/** Whether a command's head tokens (post-prefix-stripping) start a heavy run. */
function isHeavyTokens(tokens) {
  const [t0, t1, t2] = tokens;
  if (!t0) return false;
  switch (t0) {
    case "cargo":
      return /^(build|test|nextest|clippy|llvm-cov|check|mutants|bench|run)$/.test(t1 ?? "");
    case "cargo-nextest":
      return true;
    case "nextest":
      return t1 === "run";
    case "vitest":
      return true;
    case "playwright":
      return t1 === "test";
    case "yarn":
      return /^(test|build)$/.test(t1 ?? "");
    case "npm":
      if (t1 === "test") return true;
      if (t1 === "run" && NPM_RUN_HEAVY_RE.test(t2 ?? "")) return true;
      return false;
    case "npx":
      return /^(vitest|playwright)$/.test(t1 ?? "");
    case "make": {
      let i = 1;
      while (i < tokens.length) {
        if (/^-j\d*$/.test(tokens[i])) {
          i += 1;
          continue;
        }
        if (tokens[i] === "-j" || tokens[i] === "-C") {
          i += 2;
          continue;
        }
        break;
      }
      return MAKE_HEAVY_TARGET_RE.test(tokens[i] ?? "");
    }
    default:
      return BIN_HEAVY_RE.test(t0);
  }
}

/** Whether `cmd` (a raw Bash command string) starts a heavy build/test run. */
export function isHeavyCommand(cmd) {
  return splitSegments(cmd).some((segment) => {
    const tokens = stripQuotes(segment).trim().split(/\s+/).filter(Boolean);
    return isHeavyTokens(stripPrefixes(tokens));
  });
}

// CLI mode (invoked from one-heavy-build.sh): read the command from stdin,
// exit 0 if heavy, 1 if not.
if (import.meta.url === `file://${process.argv[1]}`) {
  let input = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (chunk) => (input += chunk));
  process.stdin.on("end", () => {
    process.exit(isHeavyCommand(input) ? 0 : 1);
  });
}
