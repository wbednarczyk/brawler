// Guardrail (ADR 0080 decision 7 / ADR 0045): an orphaned `src/api` command
// wrapper must redden the gate instead of hiding. The class instance was
// `api/claimExtraction.ts` — a complete, UI-unwired command surface that
// slipped past knip because (a) `knip.json` excludes `exports`/`types` and
// (b) test files are knip entry points, so a wrapper exercised only by the
// mock-runtime tests counts as "used".
//
// The precise rule: every module under `src/api/` (except the generated DTOs)
// must be imported by at least one PRODUCTION file — a non-test file outside
// `src/test/`. A wrapper reachable only from tests/mocks is an unshipped
// command surface (CLAUDE.md: "a capability is not done until a user can
// reach it") and fails here. Export-level granularity was deliberately NOT
// used: live api modules legitimately carry a few not-yet-wired named exports,
// and flagging those would make the gate cry wolf (ADR 0045: never add a broad
// gate that flags legitimate code).
import { readdirSync, readFileSync, statSync, existsSync } from "node:fs";
import { join, dirname, resolve, relative, sep } from "node:path";

const ROOT = resolve(new URL("../..", import.meta.url).pathname);
const API_DIR = join(ROOT, "src", "api");

// Documented exemptions — each entry needs an ADR/owner-decision reference.
// (Empty: the one historical entry, `src/api/claimExtraction.ts`, was retired
// with the in-app AI analysis layer — ADR 0084, which reverses ADR 0080 §6.)
const PARKED_ALLOWLIST = new Set([]);

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

console.log(`✓ api-surface-guard: ${apiModules.length} src/api modules all reachable from production code (${PARKED_ALLOWLIST.size} documented parked exemption).`);
