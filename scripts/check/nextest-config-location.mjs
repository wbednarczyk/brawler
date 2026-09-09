// Nextest resolves `.config/nextest.toml` relative to the Cargo workspace
// root, NOT the repo root — a repo-root copy is silently ignored by nextest.
// That is exactly how the `loopback-sockets` test group (the guardrail that
// serializes tests binding real loopback sockets) went inactive from
// 2026-07-12 until caught in PR #491 review: `cargo nextest show-config
// test-groups` from src-tauri printed no groups the whole time. This check
// guards both halves of the fix: no stray config at the repo root, and the
// config nextest actually reads (`src-tauri/.config/nextest.toml`) still
// carries the group. Pure and side-effect-free so it is unit-testable
// against a fixture tree — see gate-integrity.test.mjs.
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

export function checkNextestConfigLocation(repoRoot) {
  const errors = [];

  if (existsSync(resolve(repoRoot, ".config/nextest.toml"))) {
    errors.push(
      "`.config/nextest.toml` exists at the repo root — nextest resolves its config relative to the\n" +
        "    Cargo workspace root (`src-tauri/`), so a repo-root copy is silently ignored (this is exactly\n" +
        "    how the loopback-sockets test group went inactive, 2026-07-12 to PR #491). Delete it.",
    );
  }

  let nextestConfig;
  try {
    nextestConfig = readFileSync(resolve(repoRoot, "src-tauri/.config/nextest.toml"), "utf8");
  } catch {
    nextestConfig = null;
  }
  if (nextestConfig === null) {
    errors.push(
      "`src-tauri/.config/nextest.toml` not found — nextest reads its config relative to the Cargo\n" +
        "    workspace root, so the `loopback-sockets` test group must live there.",
    );
    return errors;
  }

  // A plain `.includes("loopback-sockets")` string check passes on a
  // commented-out group, or a group with no override actually assigning
  // tests to it — either leaves the serialization guardrail inert while
  // still "mentioning" the string. Strip whole-line `#` comments first (no
  // TOML parser, just a line scan), then require BOTH halves for real:
  //   (a) an uncommented `loopback-sockets = {...}` line inside [test-groups]
  //   (b) an override block ([[profile.default.overrides]]) that assigns
  //       `test-group = 'loopback-sockets'` AND has its own `filter =` line
  //       — an override with a test-group but no filter matches nothing.
  const lines = nextestConfig
    .split("\n")
    .map((line) => (line.trim().startsWith("#") ? "" : line));

  let inTestGroups = false;
  let groupDeclared = false;
  let inOverrides = false;
  let overrideHasTestGroup = false;
  let overrideHasFilter = false;
  let overrideQualifies = false;

  for (const line of lines) {
    // A header may carry a trailing comment (`[test-groups] # socket limits`)
    // — TOML ignores it, so must this scan.
    const trimmed = line.replace(/\s*#.*$/, "").trim();
    if (trimmed.startsWith("[")) {
      if (inOverrides && overrideHasTestGroup && overrideHasFilter) {
        overrideQualifies = true;
      }
      inTestGroups = trimmed === "[test-groups]";
      inOverrides = trimmed === "[[profile.default.overrides]]";
      overrideHasTestGroup = false;
      overrideHasFilter = false;
      continue;
    }
    if (inTestGroups && /^\s*loopback-sockets\s*=\s*\{/.test(line)) {
      groupDeclared = true;
    }
    if (inOverrides && /^\s*test-group\s*=\s*['"]loopback-sockets['"]/.test(line)) {
      overrideHasTestGroup = true;
    }
    if (inOverrides && /^\s*filter\s*=/.test(line)) {
      overrideHasFilter = true;
    }
  }
  if (inOverrides && overrideHasTestGroup && overrideHasFilter) {
    overrideQualifies = true;
  }

  if (!groupDeclared) {
    errors.push(
      "`src-tauri/.config/nextest.toml` has no uncommented `loopback-sockets = {...}` line inside\n" +
        "    `[test-groups]` (guardrail 2026-07-12) — a commented-out group, or one under the wrong\n" +
        "    section, leaves the socket-binding tests it should serialize free to race again.",
    );
  }
  if (!overrideQualifies) {
    errors.push(
      "`src-tauri/.config/nextest.toml` has no `[[profile.default.overrides]]` block that both sets\n" +
        "    `test-group = 'loopback-sockets'` AND has its own `filter =` line — the group exists but\n" +
        "    nothing is actually assigned to it, so the socket-binding tests still run unserialized.",
    );
  }

  return errors;
}
