// Guardrail (ADR 0080 decision 7 / ADR 0045): an orphaned `src/api` command
// wrapper must redden the gate instead of hiding. The class instance was
// `api/claimExtraction.ts` — a complete, UI-unwired command surface that
// slipped past knip because (a) `knip.json` excludes `exports`/`types` and
// (b) test files are knip entry points, so a wrapper exercised only by the
// mock-runtime tests counts as "used".
//
// The precise rule, two levels (issue #131 closed the export-level gap):
// 1. MODULE level: every module under `src/api/` (except the generated DTOs)
//    must be imported by at least one PRODUCTION file — a non-test file outside
//    `src/test/`. A wrapper reachable only from tests/mocks is an unshipped
//    command surface (CLAUDE.md: "a capability is not done until a user can
//    reach it") and fails here.
// 2. EXPORT level: `knip --include exports` must report no unused value export
//    under `src/api/` (types stay excluded). Deliberately parked wrappers go in
//    EXPORT_ALLOWLIST with an issue/ADR reference instead, so the gate stays
//    precise (ADR 0045: never a broad gate that flags legitimate code).
import { readdirSync, readFileSync, statSync, existsSync } from "node:fs";
import { join, dirname, resolve, relative, sep } from "node:path";
import { spawnSync } from "node:child_process";

const ROOT = resolve(new URL("../..", import.meta.url).pathname);
const API_DIR = join(ROOT, "src", "api");

// Documented exemptions — each entry needs an ADR/owner-decision reference.
// intentionally empty
const PARKED_ALLOWLIST = new Set([]);

// Export-level exemptions: "src/api/<file>.ts#<exportName>" — a wrapper kept
// on purpose for an approved upcoming slice. Each entry needs an issue/ADR
// reference in a comment beside it.
// intentionally empty
const EXPORT_ALLOWLIST = new Set([]);

/** Recursively collect .ts/.tsx files under dir. */
function walk(dir) {
  const out = [];
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) out.push(...walk(path));
    else if (/\.(ts|tsx)$/.test(name)) out.push(path);
  }
  return out;
}

function isProductionFile(path) {
  const rel = relative(ROOT, path).split(sep).join("/");
  if (!rel.startsWith("src/")) return false;
  if (rel.startsWith("src/test/")) return false;
  if (/\.(test|spec)\.(ts|tsx)$/.test(rel)) return false;
  return true;
}

// The guarded surface: top-level src/api modules (generated DTOs are types
// emitted by ts-rs, not command wrappers).
const apiModules = readdirSync(API_DIR)
  .filter((name) => /\.ts$/.test(name) && !/\.(test|spec)\.ts$/.test(name))
  .map((name) => join(API_DIR, name));

// Resolve every static import specifier in every production file to an
// absolute module path; count imports into src/api.
const importedApiModules = new Set();
const allSrc = walk(join(ROOT, "src")).filter(isProductionFile);
const importPattern = /(?:from|import)\s+"([^"]+)"/g;
for (const file of allSrc) {
  const source = readFileSync(file, "utf8");
  for (const match of source.matchAll(importPattern)) {
    const spec = match[1];
    if (!spec.startsWith(".")) continue;
    const base = resolve(dirname(file), spec);
    for (const candidate of [`${base}.ts`, `${base}.tsx`, join(base, "index.ts")]) {
      if (candidate.startsWith(API_DIR) && existsSync(candidate)) {
        importedApiModules.add(candidate);
      }
    }
  }
}

const orphans = apiModules.filter((module) => {
  if (importedApiModules.has(module)) return false;
  const rel = relative(ROOT, module).split(sep).join("/");
  // Self-imports don't count (a module importing its own siblings is fine, but
  // an api module used only by other orphaned api modules is still orphaned —
  // kept simple: any production import counts, which matches the class).
  return !PARKED_ALLOWLIST.has(rel);
});

if (orphans.length > 0) {
  console.error(
    "✖ Orphaned src/api command wrapper(s) — no production (non-test) file imports them.\n" +
      "  A command surface no user can reach is not done (CLAUDE.md / ADR 0080 decision 7).\n" +
      "  Wire a UI entry point, remove the wrapper, or add a documented parked exemption:\n" +
      orphans.map((p) => `    ${relative(ROOT, p).split(sep).join("/")}`).join("\n"),
  );
  process.exit(1);
}

// Ratchet hygiene: a stale allowlist entry (file gone or now wired) must be
// removed so the exemption list never rots.
const stale = [...PARKED_ALLOWLIST].filter((rel) => {
  const abs = join(ROOT, rel);
  return !existsSync(abs) || importedApiModules.has(abs);
});
if (stale.length > 0) {
  console.error(
    "✖ Stale api-surface exemption(s) — remove from PARKED_ALLOWLIST in scripts/check/api-surface-guard.mjs:\n" +
      stale.map((s) => `    ${s}`).join("\n"),
  );
  process.exit(1);
}

// Export level (issue #131): knip's unused-exports analysis, filtered to
// src/api value exports. Knip resolves namespace-member usage (`import * as
// api` + `api.fn()`), so this is stronger than a textual grep; the global
// `exclude: ["exports"]` in knip.json stays (outside src/api the rule IS
// noisy) and is overridden here per-run.
const knip = spawnSync("npx", ["knip", "--include", "exports", "--reporter", "json", "--no-exit-code"], {
  cwd: ROOT,
  encoding: "utf8",
  maxBuffer: 64 * 1024 * 1024,
});
if (knip.error || knip.stdout.trim() === "") {
  console.error("✖ api-surface-guard: knip export analysis failed to run.", knip.error ?? knip.stderr);
  process.exit(1);
}
const report = JSON.parse(knip.stdout);
const exportHits = [];
for (const issue of report.issues ?? []) {
  const file = issue.file ?? "";
  if (!file.startsWith("src/api/") || file.startsWith("src/api/generated/")) continue;
  for (const hit of issue.exports ?? []) {
    const key = `${file}#${hit.name}`;
    if (!EXPORT_ALLOWLIST.has(key)) exportHits.push(`${file}:${hit.line} ${hit.name}`);
  }
}
if (exportHits.length > 0) {
  console.error(
    "✖ Orphaned src/api export(s) — no production or test code uses them (knip exports rule, issue #131).\n" +
      "  Wire a caller, remove the wrapper (headless commands need no frontend wrapper),\n" +
      "  or add a documented entry to EXPORT_ALLOWLIST in scripts/check/api-surface-guard.mjs:\n" +
      exportHits.map((h) => `    ${h}`).join("\n"),
  );
  process.exit(1);
}

// Export-allowlist hygiene mirrors the module allowlist: entries must exist.
const staleExports = [...EXPORT_ALLOWLIST].filter((key) => {
  const [file] = key.split("#");
  return !existsSync(join(ROOT, file));
});
if (staleExports.length > 0) {
  console.error(
    "✖ Stale EXPORT_ALLOWLIST entr(ies) — file gone; remove from scripts/check/api-surface-guard.mjs:\n" +
      staleExports.map((s) => `    ${s}`).join("\n"),
  );
  process.exit(1);
}

console.log(
  `✓ api-surface-guard: ${apiModules.length} src/api modules all reachable from production code ` +
    `(${PARKED_ALLOWLIST.size} module exemption, ${EXPORT_ALLOWLIST.size} export exemption); no orphaned src/api exports.`,
);
