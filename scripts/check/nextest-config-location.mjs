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
  } else if (!nextestConfig.includes("loopback-sockets")) {
    errors.push(
      "`src-tauri/.config/nextest.toml` is missing the `loopback-sockets` test group (guardrail\n" +
        "    2026-07-12) — the socket-binding tests it serializes would race each other again.",
    );
  }

  return errors;
}
