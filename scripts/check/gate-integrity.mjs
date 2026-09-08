#!/usr/bin/env node
// Gate-integrity meta-guard (ADR 0062, extends ADR 0045/0038).
//
// The recurring failure this closes: a deterministic test suite that is not a
// hard-fail step of the single mandatory gate ROTS. It happened two ways —
//   (a) a suite (full Playwright browser, knip) lived only in `check-epic`, run
//       at closure cadence, so per-change work never exercised it; and
//   (b) its step was prefixed with `-` in the Makefile, so make IGNORED the
//       exit code — the target printed FAILURES and still exited 0 (silent red).
// The composable-shell→cockpit migration left the browser suite 28-red for two
// sessions exactly this way.
//
// This guard parses the `check` and `check-docs` recipes in the Makefile and
// FAILS the gate when the anti-rot contract is violated:
//   1. No recipe line in either target is `-`-prefixed (exit-ignored). A gate
//      step that cannot fail the gate is not a gate.
//   2. The `check` target contains every mandatory-suite marker, so no suite can
//      be silently dropped from the gate.
//
// Pure Makefile read + string checks; no dependencies. Run as the last step of
// `make check` (so it also guards itself).

import { readdirSync, readFileSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const makefilePath = resolve(repoRoot, "Makefile");

// Every deterministic/hermetic suite that MUST be a hard-fail step of `make
// check`. After the ADR 0090 CI/Makefile-parity decomposition, `make check`
// composes granular `$(MAKE) <target>` wrappers instead of inlining npm/cargo,
// so this guard checks TWO layers and the guarantee is unweakened:
//   (a) `make check` invokes each mandatory sub-`target`, and
//   (b) that sub-target's own recipe still contains the underlying suite
//       `marker` — so a suite cannot be silently gutted inside a wrapper.
// A `marker` of null means the target IS the suite (types-check).
const MANDATORY_SUITES = [
  { target: "check-rust-lint", marker: "cargo fmt --check", label: "rust fmt check" },
  { target: "check-rust-lint", marker: "cargo clippy", label: "rust clippy lint" },
  { target: "check-rust-test", marker: "cargo nextest run", label: "rust nextest suite" },
  { target: "check-rust-test", marker: "cargo test --doc", label: "rust doc-tests" },
  { target: "check-frontend-static", marker: "npm run typecheck", label: "TS typecheck" },
  { target: "check-frontend-static", marker: "npm run lint", label: "ESLint" },
  { target: "check-frontend-static", marker: "npm run stylelint", label: "Stylelint" },
  { target: "check-frontend-static", marker: "npm run knip", label: "knip dead-code + api-surface guard" },
  { target: "check-frontend-test", marker: "npm run test", label: "Vitest suite" },
  { target: "check-frontend-build", marker: "npm run build", label: "production build" },
  { target: "types-check", marker: null, label: "ts-rs generated-DTO drift guard" },
  { target: "check-browser", marker: "test:browser", label: "Playwright browser UI suite (full)" },
  { target: "check-docs-gates", marker: "gate-integrity", label: "this meta-guard (self-referential)" },
  { target: "check-docs-gates", marker: "docs-drift", label: "spec↔code drift gate (ADR 0065)" },
  { target: "coverage-frontend", marker: "npm run test:coverage", label: "frontend coverage ratchet (ADR 0096)" },
  { target: "coverage-rust", marker: "cargo llvm-cov", label: "rust coverage ratchet (ADR 0096)" },
  { target: "check-docs-gates", marker: 'node --test "scripts/check/*.test.mjs"', label: "check-script unit tests (the full scripts/check glob)" },
  { target: "check-docs-gates", marker: "retired-surface", label: "retired-surface manifest gate (live docs vs retired ADR surface)" },
  { target: "check-docs-gates", marker: "file-size-ratchet", label: "file-size ratchet — oversized-file fitness function (ADR 0103)" },
  { target: "check-visual", marker: "--project=chromium-visual --project=chromium-visual-light", label: "pinned-renderer visual baselines (both projects, #448)" },
  { target: "check-docs-gates", marker: '"scripts/ux/*.test.mjs"', label: "scripts/ux unit tests (pinned-renderer predicate, contact sheet)" },
  { target: "check-docs-gates", marker: "escaped-defects-report.mjs --validate", label: "escaped-defect table schema gate (ADR 0081 Q7 follow-up, G4)" },
];

// Targets whose recipes must never contain an exit-ignored (`-`-prefixed) step.
const GUARDED_TARGETS = ["check", "check-docs", "check-docs-gates", "check-tests-touched"];

/**
 * Extract the recipe lines (tab-indented commands) for a Makefile target. The
 * recipe is the run of tab-indented lines immediately following the `name:`
 * header, ending at the first line that is not tab-indented.
 */
function recipeLines(makefile, target) {
  const lines = makefile.split("\n");
  const headerRe = new RegExp(`^${target}:`);
  const start = lines.findIndex((l) => headerRe.test(l));
  if (start === -1) return null;
  const recipe = [];
  for (let i = start + 1; i < lines.length; i++) {
    const line = lines[i];
    if (line.startsWith("\t")) {
      recipe.push(line.slice(1)); // drop the leading tab
    } else if (line.trim() === "") {
      break; // a blank line ends the rule
    } else {
      break; // a non-recipe line ends the rule
    }
  }
  return recipe;
}

/**
 * A recipe line is exit-ignored when, after an optional leading `@` (silent)
 * modifier, its command begins with `-` (make's ignore-errors prefix).
 */
function isExitIgnored(line) {
  const command = line.replace(/^@/, "");
  return command.startsWith("-");
}

const makefile = readFileSync(makefilePath, "utf8");
const errors = [];

// (1) No exit-ignored steps in the guarded gate targets.
for (const target of GUARDED_TARGETS) {
  const recipe = recipeLines(makefile, target);
  if (recipe === null) {
    errors.push(`Target \`${target}\` not found in Makefile — the gate structure changed unexpectedly.`);
    continue;
  }
  for (const line of recipe) {
    if (isExitIgnored(line)) {
      errors.push(
        `Target \`${target}\` has an exit-ignored (\`-\`-prefixed) step: "${line.trim()}".\n` +
          `    A gate step whose exit code is ignored can print FAILURES and still exit 0 (silent red).\n` +
          `    Remove the leading \`-\` so the step hard-fails the gate. If a suite genuinely cannot be\n` +
          `    a hard-fail gate step (non-deterministic / credentialed / network / OS-specific), it does\n` +
          `    not belong in \`check\` at all — move it to a dedicated advisory/audit target.`,
      );
    }
  }
}

// (2) Every mandatory suite is present in `make check` — two layers (see
//     MANDATORY_SUITES): the `check` recipe invokes the sub-target, AND the
//     sub-target's own recipe still contains the underlying suite marker.
const checkRecipe = recipeLines(makefile, "check") ?? [];
const checkBody = checkRecipe.join("\n");
const targetRecipeCache = new Map();
function targetBody(target) {
  if (!targetRecipeCache.has(target)) {
    targetRecipeCache.set(target, (recipeLines(makefile, target) ?? []).join("\n"));
  }
  return targetRecipeCache.get(target);
}
for (const { target, marker, label } of MANDATORY_SUITES) {
  if (!checkBody.includes(target)) {
    errors.push(
      `Mandatory suite missing from \`make check\`: ${label} — the \`check\` recipe does not invoke \`${target}\`.\n` +
        `    Every deterministic suite must be a hard-fail step of the single gate — see ADR 0062.\n` +
        `    Do not remove a suite from the gate to make it pass; fix the suite.`,
    );
    continue;
  }
  if (marker !== null && !targetBody(target).includes(marker)) {
    errors.push(
      `Mandatory suite gutted inside its wrapper: ${label} — target \`${target}\` no longer runs "${marker}".\n` +
        `    The CI/Makefile-parity decomposition (ADR 0090) still requires the underlying suite to run.\n` +
        `    Restore the step; do not hollow out a gate wrapper to make it pass.`,
    );
  }
}

// (2b) CI/Makefile parity (ADR 0090): every check-EXECUTING `run:` step in
//      full-check.yml must invoke `make <target>` — CI carries zero bespoke
//      logic, so the same thing runs identically locally and in CI. `uses:`
//      steps (checkout/cache/setup/paths-filter actions) are exempt by nature;
//      a small allowlist of infra `run:` steps (disk reclaim) is exempt too.
// Both gate-carrying workflows are held to the make-parity contract: the full
// gate and the label check split out of it (release-label.yml exists solely so
// labeled/unlabeled events re-run a 4s job instead of the whole gate).
const GATE_WORKFLOW_PATHS = [
  ".github/workflows/full-check.yml",
  ".github/workflows/release-label.yml",
];
// Substrings (lower-cased) of infra run-step NAMES that legitimately do not call
// make. Keep this list tiny and specific — it is the only bespoke-CI escape.
const INFRA_RUN_STEP_NAMES = ["reclaim disk", "free disk space", "fix workspace ownership", "install webview2"];
for (const FULL_CHECK_PATH of GATE_WORKFLOW_PATHS) {
const workflow = readIfExists(FULL_CHECK_PATH);
if (workflow === null) {
  errors.push(
    `\`${FULL_CHECK_PATH}\` not found — the CI/Makefile-parity contract (ADR 0090) cannot be verified.`,
  );
} else {
  const wfLines = workflow.split("\n");
  let currentName = "";
  for (let i = 0; i < wfLines.length; i++) {
    const nameMatch = wfLines[i].match(/^\s*-?\s*name:\s*(.+?)\s*$/);
    if (nameMatch) currentName = nameMatch[1].replace(/^["']|["']$/g, "");
    const runMatch = wfLines[i].match(/^(\s*)-?\s*run:\s*(.*)$/);
    if (!runMatch) continue;
    let cmd = runMatch[2].trim();
    // Fold a block scalar (`run: |` / `run: >`) into the command text.
    if (["", "|", ">", "|-", ">-", "|+", ">+"].includes(cmd)) {
      const baseIndent = runMatch[1].length;
      for (let j = i + 1; j < wfLines.length; j++) {
        if (wfLines[j].trim() === "") continue;
        const indent = wfLines[j].match(/^\s*/)[0].length;
        if (indent <= baseIndent) break;
        cmd += "\n" + wfLines[j].trim();
      }
    }
    // A `make <target>` invocation at a COMMAND position: line start, after a
    // shell separator (; && ||), or after `nix develop … -c`. Anchoring this way
    // avoids false-passing prose like `echo "please make it"`.
    const invokesMake = /(^|\n|[;&|]\s*|nix develop\b[^\n]*-c\s+)\s*make\s+[a-z]/m.test(cmd);
    const isInfra = INFRA_RUN_STEP_NAMES.some((n) => currentName.toLowerCase().includes(n));
    if (!invokesMake && !isInfra) {
      errors.push(
        `\`${FULL_CHECK_PATH}\` step "${currentName || "(unnamed)"}" runs bespoke logic instead of \`make <target>\` (ADR 0090).\n` +
          `    Offending command: ${cmd.split("\n")[0]}\n` +
          `    Every check-executing CI step must be a thin \`make <target>\` wrapper (or a setup \`uses:\` action).\n` +
          `    If this is genuine infra (disk reclaim), name it accordingly and add it to INFRA_RUN_STEP_NAMES deliberately.`,
      );
    }
  }
}
}

// (2c) Bare-checkout imports: `check-docs-gates` runs every scripts/check and
// scripts/ux module (incl. the test globs) in CI WITHOUT `npm ci`, so a bare
// package import there loads fine locally and dies on the runner ("Cannot find
// module" — #449, pinned-renderer.mjs requiring @playwright/test). Only `node:`
// builtins and relative paths are allowed; a script that genuinely needs a
// dependency is listed here with the target that installs first.
const BARE_CHECKOUT_IMPORT_EXEMPT = new Set([
  "scripts/check/engines-floor.mjs", // semver; not run by check-docs-gates
]);
for (const dir of ["scripts/check", "scripts/ux"]) {
  for (const name of readdirSync(resolve(repoRoot, dir))) {
    if (!name.endsWith(".mjs")) continue;
    const rel = `${dir}/${name}`;
    if (BARE_CHECKOUT_IMPORT_EXEMPT.has(rel)) continue;
    const src = readFileSync(resolve(repoRoot, rel), "utf8");
    for (const m of src.matchAll(/(?:^import\s[^;]*?\sfrom\s*|^import\s*|\bimport\(\s*|createRequire\([^)]*\)\(\s*)["']([^"']+)["']/gm)) {
      const spec = m[1];
      if (spec.startsWith("node:") || spec.startsWith(".") || spec.startsWith("/")) continue;
      errors.push(
        `\`${rel}\` imports \`${spec}\` — check-docs-gates runs on a bare checkout (no node_modules), so this dies in CI.\n` +
          `    Use a node: builtin or a relative module; if the dependency is essential, add the file to BARE_CHECKOUT_IMPORT_EXEMPT with the target that installs it.`,
      );
    }
  }
}

// (2d) Advisory workflows must have an addressee (G13, ADR 0096 dec. 5 +
// owner 2026-09-07): "advisory" must never mean "silent" — mutation-audit
// runs 33203998627/32237111937 each reported 2 missed mutants with no card
// filed, because an advisory result had nowhere to go. A workflow is
// "advisory" when its `name:` contains "advisory" (case-insensitive) OR its
// header comment (the first 10 lines) does (finding 4, ADR 0045 harvest
// 2026-09-08: bench-audit.yml says "Advisory" only in its header comment —
// keying on `name:` alone missed it entirely). Either way it must carry a
// `gh issue create` step so a finding always gets a tracked addressee.
const workflowsDir = resolve(repoRoot, ".github/workflows");
for (const name of readdirSync(workflowsDir)) {
  if (!name.endsWith(".yml") && !name.endsWith(".yaml")) continue;
  const rel = `.github/workflows/${name}`;
  const content = readFileSync(resolve(repoRoot, rel), "utf8");
  const nameMatch = content.match(/^name:\s*(.+)$/m);
  const workflowName = (nameMatch ? nameMatch[1] : "").trim();
  const headerLines = content.split("\n").slice(0, 10).join("\n");
  const isAdvisory = /advisory/i.test(workflowName) || /advisory/i.test(headerLines);
  if (!isAdvisory) continue;
  if (!content.includes("gh issue create")) {
    errors.push(
      `\`${rel}\` is an advisory workflow (name: "${workflowName}", or its header comment says so) but ` +
        `has no \`gh issue create\` step.\n` +
        `    Advisory ≠ silent (ADR 0096 dec. 5 + owner 2026-09-07): a finding with no addressee rots\n` +
        `    unaddressed. Add a carding step that opens/reuses a tracked issue for its findings.`,
    );
  }
}

// (2e) G14 tests-touched.yml (hard gates wave 2): the workflow must exist,
// re-evaluate on EVERY relevant PR event (a fresh push AND a label change —
// adding `tests:not-needed` must flip a red PR green without a new push, the
// release-label.yml rationale), its check-executing step must genuinely (not
// commented out) invoke the gate's own make target with no `if:`/
// `continue-on-error:` escape hatch, labels must travel as a JSON array (not
// a comma list a label's own name could fragment), and the Makefile recipe it
// calls must still shell out to the real script. Scoped to this one workflow
// rather than folded into GATE_WORKFLOW_PATHS/(2b) above, same reason
// release-label.yml lives outside full-check.yml: it deliberately carries its
// own trigger `types:`.
const TESTS_TOUCHED_PATH = ".github/workflows/tests-touched.yml";
const testsTouchedContent = readIfExists(TESTS_TOUCHED_PATH);
if (testsTouchedContent === null) {
  errors.push(`\`${TESTS_TOUCHED_PATH}\` not found — the tests-touched gate (G14) is not wired up.`);
} else {
  const typesMatch = testsTouchedContent.match(/^\s*types:\s*\[(.+)\]\s*$/m);
  const types = typesMatch ? typesMatch[1].split(",").map((t) => t.trim()) : [];
  for (const required of ["opened", "synchronize", "reopened", "labeled", "unlabeled"]) {
    if (!types.includes(required)) {
      errors.push(
        `\`${TESTS_TOUCHED_PATH}\` pull_request \`types:\` does not include \`${required}\` — it must\n` +
          `    re-evaluate on every relevant PR event (a fresh push AND a label change), never a subset (G14).`,
      );
    }
  }
  // Line-anchored: a `# run: make check-tests-touched` comment must not count.
  if (!/^\s*run:\s*make check-tests-touched\b/m.test(testsTouchedContent)) {
    errors.push(
      `\`${TESTS_TOUCHED_PATH}\` has no active (non-commented) \`run: make check-tests-touched\` step —\n` +
        `    every check-executing CI step must be a thin \`make <target>\` wrapper (ADR 0090).`,
    );
  }
  if (/^\s*if:/m.test(testsTouchedContent)) {
    errors.push(
      `\`${TESTS_TOUCHED_PATH}\` carries an \`if:\` condition — this workflow's one job/step must always\n` +
        `    run unconditionally; a conditionally-skipped gate is not a gate (G14).`,
    );
  }
  if (/continue-on-error:/.test(testsTouchedContent)) {
    errors.push(
      `\`${TESTS_TOUCHED_PATH}\` carries \`continue-on-error:\` — a gate step whose failure is ignored is\n` +
        `    not a gate (G14, same class as an exit-ignored Makefile step).`,
    );
  }
  if (!/gh api\s+"?repos\/\$\{\{\s*github\.repository\s*\}\}\/issues\/.*\/labels"?/.test(testsTouchedContent)) {
    errors.push(
      `\`${TESTS_TOUCHED_PATH}\` has no \`gh api repos/.../issues/.../labels\` step — labels must be fetched\n` +
        `    LIVE at check time, not trusted from the (possibly stale, on a manual re-run) triggering event\n` +
        `    payload (G14).`,
    );
  }
  if (!/PR_LABELS:\s*\$\{\{\s*steps\.labels\.outputs\.labels\s*\}\}/.test(testsTouchedContent)) {
    errors.push(
      `\`${TESTS_TOUCHED_PATH}\` does not pass \`PR_LABELS\` from \`steps.labels.outputs.labels\` — the check\n` +
        `    step must consume the live-fetched labels step's output, not the event payload directly (G14).`,
    );
  }
}

const checkTestsTouchedRecipe = recipeLines(makefile, "check-tests-touched");
const testsTouchedInvocationLine = checkTestsTouchedRecipe?.find((l) => l.includes("scripts/check/tests-touched.mjs")) ?? null;
if (checkTestsTouchedRecipe === null || testsTouchedInvocationLine === null) {
  errors.push(
    "`Makefile`'s `check-tests-touched` recipe does not invoke `scripts/check/tests-touched.mjs` — the\n" +
      "    gate's Make wrapper must still shell out to the real script (G14).",
  );
} else {
  // (a) the invocation line itself must be a live, hard-fail step — not
  // exit-ignored (covered generically by GUARDED_TARGETS above, re-asserted
  // here scoped to the exact line), not commented out, not swapped for a
  // no-op (`@true`/`true`).
  if (isExitIgnored(testsTouchedInvocationLine)) {
    errors.push(
      "`Makefile`'s `check-tests-touched` recipe's `tests-touched.mjs` invocation line is `-`-prefixed\n" +
        "    (exit-ignored) — a gate step whose exit code is ignored can print FAILURES and still exit 0 (G14).",
    );
  }
  const bareInvocation = testsTouchedInvocationLine.trim().replace(/^@/, "");
  if (bareInvocation.startsWith("#")) {
    errors.push(
      "`Makefile`'s `check-tests-touched` recipe's `tests-touched.mjs` invocation line is commented out —\n" +
        "    the gate's Make wrapper must actually run the script, not just mention it (G14).",
    );
  } else if (/^true\b/.test(bareInvocation)) {
    errors.push(
      "`Makefile`'s `check-tests-touched` recipe's `tests-touched.mjs` invocation has been replaced with a\n" +
        "    no-op (`@true`/`true`) — the gate's Make wrapper must actually run the script (G14).",
    );
  }
}

// (2f) T3 retries->0 (hard gates wave 2, owner 2026-09-08): a CI flake must be
// seen and fixed at once, never masked by a retry. Positive assertion: every
// `retries:` occurrence anywhere in playwright.config.ts must be literally
// `retries: 0` — catches a NEW non-zero retries creeping back in, not just
// the one CI-conditional branch this replaced.
const PLAYWRIGHT_CONFIG_PATH = "playwright.config.ts";
const playwrightConfig = readIfExists(PLAYWRIGHT_CONFIG_PATH);
if (playwrightConfig === null) {
  errors.push(`\`${PLAYWRIGHT_CONFIG_PATH}\` not found — cannot verify the retries:0 rule (T3).`);
} else {
  // Strip `//` line comments first (this file's only comment style) so a
  // comment merely MENTIONING "retries: N" (e.g. explaining the rule) is
  // never mistaken for a live setting.
  const playwrightConfigCode = playwrightConfig
    .split("\n")
    .map((line) => {
      const idx = line.indexOf("//");
      return idx === -1 ? line : line.slice(0, idx);
    })
    .join("\n");
  const retriesMatches = playwrightConfigCode.match(/retries:\s*[^,\n]+/g) ?? [];
  if (retriesMatches.length === 0) {
    errors.push(`\`${PLAYWRIGHT_CONFIG_PATH}\` has no \`retries:\` setting — expected \`retries: 0\` (T3).`);
  }
  for (const m of retriesMatches) {
    if (!/^retries:\s*0\s*$/.test(m.trim())) {
      errors.push(
        `\`${PLAYWRIGHT_CONFIG_PATH}\` has \`${m.trim()}\` — every \`retries:\` must be literally \`retries: 0\`\n` +
          `    (owner 2026-09-08: a CI flake is red at once; fix the class or card it with the signature, never a retry).`,
      );
    }
  }
}

// (2g) T2 mutation-audit trigger-path parity (hard gates wave 2, ADR 0096
// dec. 5 amendment 2026-09-08): every path in mutation-audit.yml's push
// `paths:` trigger must be named in docs/testing.md § Mutation testing scope
// — the workflow's TRIGGER paths (what re-runs the sweep on a master push)
// and the mutation EXECUTION scope (the `-f` flags in `make audit-mutants`,
// which may legitimately list a different set) are related but distinct;
// this guard only binds the docs to the workflow's triggers, so the two
// cannot drift silently again (they had: `entity_resolution.rs` documented
// but not a trigger path, `storage/ingestion.rs` a trigger path but undocumented).
const MUTATION_AUDIT_PATH = ".github/workflows/mutation-audit.yml";
const mutationAuditContent = readIfExists(MUTATION_AUDIT_PATH);
const testingMdForMutationScope = readIfExists("docs/testing.md");
if (mutationAuditContent === null) {
  errors.push(`\`${MUTATION_AUDIT_PATH}\` not found — cannot verify the mutation-audit trigger-path parity (T2).`);
} else if (testingMdForMutationScope === null) {
  errors.push("`docs/testing.md` not found — cannot verify the mutation-audit trigger-path parity (T2).");
} else {
  const scopeHeaderIdx = testingMdForMutationScope.indexOf("### Mutation testing scope");
  if (scopeHeaderIdx === -1) {
    errors.push('`docs/testing.md` has no "### Mutation testing scope" section (T2).');
  } else {
    const nextHeaderIdx = testingMdForMutationScope.indexOf("\n## ", scopeHeaderIdx);
    const scopeSection = testingMdForMutationScope.slice(scopeHeaderIdx, nextHeaderIdx === -1 ? undefined : nextHeaderIdx);
    const pathsBlockMatch = mutationAuditContent.match(/paths:\n((?:\s*-\s*'[^']+'\n?)+)/);
    const triggerPaths = pathsBlockMatch ? [...pathsBlockMatch[1].matchAll(/-\s*'([^']+)'/g)].map((m) => m[1]) : [];
    if (triggerPaths.length === 0) {
      errors.push(`\`${MUTATION_AUDIT_PATH}\` has no parsable \`paths:\` list under its push trigger (T2).`);
    }
    for (const p of triggerPaths) {
      if (!scopeSection.includes(p)) {
        errors.push(
          `\`${MUTATION_AUDIT_PATH}\` trigger path \`${p}\` is not named in docs/testing.md § Mutation testing\n` +
            `    scope (T2) — keep the workflow's trigger paths and the docs in sync.`,
        );
      }
    }
  }
}

// ---------------------------------------------------------------------------
// context-architecture (ADR 0063)
//
// The Claude-native context layering (CLAUDE.md + session hook + a mandatory-
// read engineering-workflow.md) only stays lean if its byte budgets and
// enforcement-parity markers are checked, not just asserted in prose. This
// group fails the gate when a doc grows back past its budget, or when a
// required rule/marker silently drops out of the always-loaded surface.
// ---------------------------------------------------------------------------

// (3) Byte budgets for the L0/L1 context layers (ADR 0063 Decision 1).
const CONTEXT_ARCH_BUDGETS = {
  // 18432 → 20480 (2026-08-06, ADR 0063 amendment): agent-memory nuances
  // consolidated INTO CLAUDE.md (net always-on context shrinks — the private
  // memory index lost more than this file gained); deliberate +2 KiB.
  "CLAUDE.md": 20480,
  ".claude/hooks/session-context.sh": 2560,
  // 26624 → 27648 (2026-07-11, ADR 0063 amendment): the doc was 34 bytes from
  // the ceiling when the disk-hygiene guardrail (ADR 0045 harvest) had to join
  // the mandatory-read layer; deliberate +1 KiB, not silent growth.
  "docs/engineering-workflow.md": 27648,
  "AGENTS.md": 1024,
};

const contextArchErrors = [];

for (const [relPath, budget] of Object.entries(CONTEXT_ARCH_BUDGETS)) {
  const absPath = resolve(repoRoot, relPath);
  let size;
  try {
    size = statSync(absPath).size;
  } catch {
    contextArchErrors.push(
      `\`${relPath}\` not found — expected by the ADR 0063 context-architecture byte budget.`,
    );
    continue;
  }
  if (size > budget) {
    contextArchErrors.push(
      `\`${relPath}\` is ${size} bytes, over its ADR 0063 budget of ${budget} bytes (${size - budget} over).\n` +
        `    Trim it back under budget, or raise the budget deliberately (with an ADR 0063 update) — do not\n` +
        `    let the always-loaded/mandatory-read context grow back unchecked.`,
    );
  }
}

// (4) Marker assertions — enforcement parity (ADR 0063 Decision 4).
function readIfExists(relPath) {
  try {
    return readFileSync(resolve(repoRoot, relPath), "utf8");
  } catch {
    return null;
  }
}

const hookContent = readIfExists(".claude/hooks/session-context.sh");
if (hookContent === null) {
  contextArchErrors.push(
    "`.claude/hooks/session-context.sh` not found — the session re-grounding hook (ADR 0063) is missing.",
  );
} else {
  const HOOK_MARKERS = [/rtk/, /CLAUDE\.md/, /engineering-workflow\.md/, /spec-driven/i, /git-boundaries/];
  for (const marker of HOOK_MARKERS) {
    if (!marker.test(hookContent)) {
      contextArchErrors.push(
        `\`.claude/hooks/session-context.sh\` is missing the required re-grounding marker ${marker} (ADR 0063).\n` +
          `    The hook is the only permitted short restatement of the always-on rules — it must keep\n` +
          `    surfacing rtk discipline, the CLAUDE.md/engineering-workflow.md read order, and the\n` +
          `    spec-driven posture after compaction.`,
      );
    }
  }
}

// (4b) Hard gates G1 (git-boundaries) and G2 (one-heavy-build), tests audit
// waves 1-2: both PreToolUse Bash hooks must stay wired — they are the
// mechanical form of testing.md § Resource discipline and CLAUDE.md § Working
// Rules (T1) for every agent/subagent. Parses hooks.PreToolUse rather than a
// plain substring test so a Bash entry with the WRONG matcher can't fool it.
// The wiring check requires the EXACT command string (not a substring
// anywhere in some unrelated command — a `.includes(base)` check let a
// command that merely MENTIONED the filename, e.g. in a comment, satisfy it),
// and each `.sh` wrapper must be executable and `exec node` its paired `.mjs`
// — a hook that is present but chmod-stripped or gutted silently no-ops
// instead of running (hard gates wave 2 harvest, 2026-09-08).
const REQUIRED_BASH_HOOKS = [".claude/hooks/one-heavy-build.sh", ".claude/hooks/git-boundaries.sh"];
const preToolUseSettings = readIfExists(".claude/settings.json");
if (preToolUseSettings === null) {
  contextArchErrors.push(
    "`.claude/settings.json` not found — cannot verify the PreToolUse Bash hook wiring (hard gates G1/G2).",
  );
} else {
  let parsedPreToolUse;
  try {
    parsedPreToolUse = JSON.parse(preToolUseSettings);
  } catch {
    parsedPreToolUse = null;
  }
  const bashEntry = Array.isArray(parsedPreToolUse?.hooks?.PreToolUse)
    ? parsedPreToolUse.hooks.PreToolUse.find((m) => m.matcher === "Bash")
    : null;
  const bashCommands = (bashEntry?.hooks ?? []).map((h) => h.command).filter((c) => typeof c === "string");
  for (const hookFile of REQUIRED_BASH_HOOKS) {
    const expected = `bash "$CLAUDE_PROJECT_DIR/${hookFile}"`;
    if (!bashCommands.includes(expected)) {
      contextArchErrors.push(
        `\`.claude/settings.json\` does not wire \`${hookFile}\` as a Bash PreToolUse hook with the exact\n` +
          `    command \`${expected}\` (hard gates G1/G2, ADR 0038) — a command that merely MENTIONS the\n` +
          `    filename elsewhere (a comment, an unrelated string) does not count.`,
      );
    }
  }
}
for (const hookFile of REQUIRED_BASH_HOOKS) {
  const abs = resolve(repoRoot, hookFile);
  let stat;
  try {
    stat = statSync(abs);
  } catch {
    contextArchErrors.push(`\`${hookFile}\` not found (hard gate G1/G2).`);
    continue;
  }
  if ((stat.mode & 0o111) === 0) {
    contextArchErrors.push(
      `\`${hookFile}\` is not executable (missing chmod +x) — a non-executable PreToolUse Bash hook\n` +
        `    silently no-ops instead of running (hard gate G1/G2).`,
    );
  }
  const mjsName = hookFile.split("/").pop().replace(/\.sh$/, ".mjs");
  const content = readIfExists(hookFile) ?? "";
  // The approved structure: every non-comment, non-blank line (shebang and
  // `#`-comments excluded) is EXACTLY one line, the bare exec of the paired
  // .mjs — no other logic, no unanchored substring match that a stray extra
  // command elsewhere in the file could slip past.
  const codeLines = content
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l !== "" && !l.startsWith("#"));
  const expectedExecLine = `exec node "$(dirname "$0")/${mjsName}"`;
  if (codeLines.length !== 1 || codeLines[0] !== expectedExecLine) {
    contextArchErrors.push(
      `\`${hookFile}\` is not a pure exec shim for \`${mjsName}\` — the thin-wrapper contract (hard gate\n` +
        `    G1/G2) requires its non-comment, non-blank lines to be EXACTLY one line: \`${expectedExecLine}\`\n` +
        `    (the logic must live in the .mjs; the .sh does nothing else).`,
    );
  }
}
for (const hookFile of [
  ".claude/hooks/one-heavy-build.mjs",
  ".claude/hooks/one-heavy-build-classify.mjs",
  ".claude/hooks/git-boundaries.mjs",
]) {
  if (readIfExists(hookFile) === null) {
    contextArchErrors.push(`\`${hookFile}\` not found (hard gate G1/G2).`);
  }
}

const claudeMdContent = readIfExists("CLAUDE.md");
if (claudeMdContent === null) {
  contextArchErrors.push("`CLAUDE.md` not found — the canonical agent contract (ADR 0063) is missing.");
} else {
  const CLAUDE_MD_MARKERS = ["Three Always-On Rules", "Single Source Of Truth", "Required Reading"];
  for (const marker of CLAUDE_MD_MARKERS) {
    if (!claudeMdContent.includes(marker)) {
      contextArchErrors.push(
        `\`CLAUDE.md\` is missing the required section "${marker}" (ADR 0063).\n` +
          `    This section is part of the enforcement-parity contract for the canonical agent contract.`,
      );
    }
  }
}

const kanbanContent = readIfExists("docs/kanban.md");
if (kanbanContent === null) {
  contextArchErrors.push(
    "`docs/kanban.md` not found — the epic-closure checklist is part of the enforcement surface" +
      " and must not drift from the closure contract (ADR 0096).",
  );
} else {
  const KANBAN_CLOSURE_MARKERS = ["Epic closure", "retrospective", "gh issue close"];
  for (const marker of KANBAN_CLOSURE_MARKERS) {
    if (!kanbanContent.includes(marker)) {
      contextArchErrors.push(
        `\`docs/kanban.md\` is missing the required literal "${marker}" (ADR 0096) — epic closure is a` +
          ` post-delivery audit, and this checklist is part of the enforcement surface.`,
      );
    }
  }
}

const settingsContent = readIfExists(".claude/settings.json");
if (settingsContent === null) {
  contextArchErrors.push("`.claude/settings.json` not found — cannot verify the session-hook wiring (ADR 0063).");
} else {
  let settings;
  try {
    settings = JSON.parse(settingsContent);
  } catch (e) {
    contextArchErrors.push(`\`.claude/settings.json\` failed to parse as JSON (ADR 0063): ${e.message}`);
    settings = null;
  }
  if (settings) {
    const REQUIRED_MATCHERS = ["startup", "resume", "clear", "compact"];
    const sessionStart = settings?.hooks?.SessionStart;
    if (!Array.isArray(sessionStart)) {
      contextArchErrors.push(
        "`.claude/settings.json` has no `hooks.SessionStart` array — the session re-grounding hook (ADR 0063)" +
          " is not wired up.",
      );
    } else {
      for (const matcher of REQUIRED_MATCHERS) {
        const entry = sessionStart.find((m) => m.matcher === matcher);
        if (!entry) {
          contextArchErrors.push(
            `\`.claude/settings.json\` SessionStart is missing the "${matcher}" matcher (ADR 0063) — the hook\n` +
              `    must re-ground the always-on rules on start/resume/clear/compact, not a subset.`,
          );
          continue;
        }
        const invokesHook = (entry.hooks ?? []).some(
          (h) => typeof h.command === "string" && h.command.includes("session-context.sh"),
        );
        if (!invokesHook) {
          contextArchErrors.push(
            `\`.claude/settings.json\` SessionStart matcher "${matcher}" does not invoke session-context.sh` +
              " (ADR 0063).",
          );
        }
      }
    }
  }
}

if (errors.length > 0 || contextArchErrors.length > 0) {
  if (errors.length > 0) {
    console.error("✖ gate-integrity: the mandatory-gate contract is violated (ADR 0062):\n");
    for (const e of errors) console.error(`  - ${e}\n`);
  }
  if (contextArchErrors.length > 0) {
    console.error("✖ gate-integrity: the context-architecture contract is violated (ADR 0063):\n");
    for (const e of contextArchErrors) console.error(`  - ${e}\n`);
  }
  process.exit(1);
}

console.log(
  "✓ gate-integrity: single mandatory gate intact — no exit-ignored steps, all suites present, " +
    "context-architecture budgets and markers intact.",
);
