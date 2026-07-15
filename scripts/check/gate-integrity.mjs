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
// This guard parses the `check` and `check-epic` recipes in the Makefile and
// FAILS the gate when the anti-rot contract is violated:
//   1. No recipe line in either target is `-`-prefixed (exit-ignored). A gate
//      step that cannot fail the gate is not a gate.
//   2. The `check` target contains every mandatory-suite marker, so no suite can
//      be silently dropped from the gate.
//
// Pure Makefile read + string checks; no dependencies. Run as the last step of
// `make check` (so it also guards itself).

import { readFileSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const makefilePath = resolve(repoRoot, "Makefile");

// Every deterministic/hermetic suite that MUST be a hard-fail step of `make
// check`. The value is a human label; the key is a substring that must appear in
// the `check` recipe. `npm run check` covers the frontend (typecheck/lint/
// stylelint/vitest/build) and rust (fmt/clippy/nextest/doc) sub-gates.
const MANDATORY_MARKERS = {
  "npm run check": "frontend + rust core gate (npm run check)",
  "npm run knip": "dead-code audit (knip)",
  "types-check": "ts-rs generated-DTO drift guard (make types-check)",
  "npm run test:browser": "Playwright browser UI suite (full)",
  "gate-integrity": "this meta-guard (self-referential)",
  "docs-drift": "spec↔code drift gate (ADR 0065)",
};

// Targets whose recipes must never contain an exit-ignored (`-`-prefixed) step.
const GUARDED_TARGETS = ["check", "check-epic", "check-docs"];

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
          `    not belong in \`check\`/\`check-epic\` at all — move it to a dedicated periodic target.`,
      );
    }
  }
}

// (2) Every mandatory suite is present in `check`.
const checkRecipe = recipeLines(makefile, "check") ?? [];
const checkBody = checkRecipe.join("\n");
for (const [marker, label] of Object.entries(MANDATORY_MARKERS)) {
  if (!checkBody.includes(marker)) {
    errors.push(
      `Mandatory suite missing from \`make check\`: ${label} (expected marker "${marker}").\n` +
        `    Every deterministic suite must be a hard-fail step of the single gate — see ADR 0062.\n` +
        `    Do not remove a suite from the gate to make it pass; fix the suite.`,
    );
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
  "CLAUDE.md": 18432,
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
  const HOOK_MARKERS = [/rtk/, /CLAUDE\.md/, /engineering-workflow\.md/, /spec-driven/i];
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

const releaseSkillContent = readIfExists(".claude/skills/brawler-release/SKILL.md");
if (releaseSkillContent === null) {
  contextArchErrors.push(
    "`.claude/skills/brawler-release/SKILL.md` not found — the release skill is part of the enforcement" +
      " surface and must not drift from the closure contract.",
  );
} else {
  const RELEASE_SKILL_MARKERS = ["make release", "check-epic", "retrospective", "rad issue state --solved"];
  for (const marker of RELEASE_SKILL_MARKERS) {
    if (!releaseSkillContent.includes(marker)) {
      contextArchErrors.push(
        `\`.claude/skills/brawler-release/SKILL.md\` is missing the required literal "${marker}" (ADR 0062/0038).\n` +
          `    The release skill is part of the enforcement surface and must not drift from the closure contract.`,
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
