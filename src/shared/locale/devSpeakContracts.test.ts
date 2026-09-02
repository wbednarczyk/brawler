import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { en } from "./resources/en";
import { pl } from "./resources/pl";
import { plText } from "./resources/plText";

// Dev-speak guard (v0.50 UX audit K6, ADR 0045 harvest): user-facing copy must
// speak the user's language, not the implementation's. "Database" leaked into
// Settings once; this pins the class shut. Keep the list tight and justified —
// a broad list would flag legitimate copy and erode trust in the gate.
// F4b S2 (docs/plans/f4b-contracts/s2-transcripts.md item 6): `job`/`jobs`
// added — the Transcripts redesign retires the "transcript job" vocabulary
// ("a transcript", never "a job"); Settings/Diagnostics/KPI copy uses
// worker/task/ingest instead, so this stays a precise addition.
// F4c S1 (docs/plans/f4c-contracts/s1-guardrails.md item 2, plan § Decisions
// 7a): the Settings language pass's vocabulary added — `pool`, `worker(s)`,
// `thread(s)`, `adapter`, `stdio`, `backfill`, `telemetry`, `keyring`,
// `migration`, `endpoint`, `JSON`. Expected red today against the pre-F4c
// Settings copy (S4 makes it green).
const FORBIDDEN =
  /\b(database|databases|IPC|SQL|runtime|backend|jsdom|localhost|jobs?|pool|workers?|threads?|adapter|stdio|backfill|telemetry|keyring|migration|endpoint|JSON)\b/i;

function offenders(strings: string[], source: string, pattern: RegExp = FORBIDDEN): string[] {
  return strings.filter((value) => pattern.test(value)).map((value) => `${source}: ${value}`);
}

// Developer-gated Diagnostics may use implementation terms (ui-authoring.md
// § product language). A locale table cannot tell where a key is used, so the
// table scan carries a NARROW allowlist of keys whose only live call site is
// under `src/screens/Diagnostics/**` — and the test below proves that
// property, so the allowlist can never hide a string that leaks elsewhere.
const DIAGNOSTICS_ONLY_KEYS = new Set(["Pre-migration snapshot"]);

describe("dev-speak guard over user-facing copy", () => {
  it("keeps implementation vocabulary out of every locale surface", () => {
    const keys = Object.keys(plText).filter((key) => !DIAGNOSTICS_ONLY_KEYS.has(key));
    const found = [
      // plText keys ARE the English user-facing strings (text() passthrough).
      ...offenders(keys, "plText key"),
      ...offenders(keys.map((key) => plText[key as keyof typeof plText]), "plText pl value"),
      ...offenders(Object.values(en), "en value"),
      ...offenders(Object.values(pl), "pl value"),
    ];
    expect(found, `Dev-speak in user-facing copy:\n${found.join("\n")}`).toEqual([]);
  });

  it("allowlisted Diagnostics-only keys are called from Diagnostics and nowhere else", () => {
    const files = listTextCallSiteFiles(process.cwd(), SCAN_ROOT, [], { includeDiagnostics: true });
    for (const key of DIAGNOSTICS_ONLY_KEYS) {
      const callers = files.filter((rel) => readFileSync(join(process.cwd(), rel), "utf8").includes(`"${key}"`));
      expect(callers.length, `${key}: no live call site — drop it from the table and the allowlist`).toBeGreaterThan(0);
      expect(
        callers.filter((rel) => !rel.startsWith("src/screens/Diagnostics/")),
        `${key} is allowlisted as Diagnostics-only but is used elsewhere`,
      ).toEqual([]);
    }
  });
});

// Retired-vocabulary guard (owner decision 2026-08-26): management claims are
// "teza/tezy" everywhere in Polish copy, never "obietnica/obietnice/obietnic"
// (promise) or "deklaracja/deklaracje" (declaration) — pins the unification
// shut so the old word can't creep back into a new string.
const RETIRED_CLAIM_VOCAB = /obietnic\w*|deklaracj\w*/i;

describe("retired claim-vocabulary guard over Polish copy", () => {
  it("keeps 'obietnica'/'deklaracja' out of every Polish locale surface", () => {
    const hits = [
      ...offenders(Object.values(plText), "plText pl value", RETIRED_CLAIM_VOCAB),
      ...offenders(Object.values(pl), "pl value", RETIRED_CLAIM_VOCAB),
    ];
    expect(hits, `Retired claim vocabulary in Polish copy:\n${hits.join("\n")}`).toEqual([]);
  });
});

// F4c S1 (docs/plans/f4c-contracts/s1-guardrails.md item 2): the table scan
// above only sees strings that made it into a locale resource table — a
// `text("…")` literal sitting in `untranslated-baseline.json` (the
// translation ratchet's allowance for not-yet-translated English passthrough
// copy) is invisible to it. This scan reads every `text("…")` call-site
// literal directly out of `src/**` source (scanner shape from
// `retiredKeys.test.ts:87-108`), catching dev-speak before it ever reaches a
// table. Exemptions: `src/screens/Diagnostics/**` (developer-gated,
// ui-authoring.md:193 — implementation language allowed there) and the
// locale resource files themselves (they carry the retired/forbidden tokens
// as literal data during other guards' migration windows, not as live copy).
const SCAN_ROOT = "src";
const SCAN_EXTENSIONS = [".ts", ".tsx"];
const SCAN_EXCLUDE_PREFIXES = ["src/screens/Diagnostics/"];
const SCAN_EXCLUDE_SUFFIXES = [
  "shared/locale/resources/plText.ts",
  "shared/locale/resources/en.ts",
  "shared/locale/resources/pl.ts",
];

function listTextCallSiteFiles(
  root: string,
  dir: string,
  out: string[],
  opts: { includeDiagnostics?: boolean } = {},
): string[] {
  for (const entry of readdirSync(join(root, dir))) {
    const rel = dir ? `${dir}/${entry}` : entry;
    const full = join(root, rel);
    if (statSync(full).isDirectory()) {
      listTextCallSiteFiles(root, rel, out, opts);
    } else if (
      SCAN_EXTENSIONS.some((ext) => rel.endsWith(ext)) &&
      !rel.endsWith(".test.ts") &&
      !rel.endsWith(".test.tsx")
    ) {
      if (!opts.includeDiagnostics && SCAN_EXCLUDE_PREFIXES.some((prefix) => rel.startsWith(prefix))) continue;
      if (SCAN_EXCLUDE_SUFFIXES.some((suffix) => rel.endsWith(suffix))) continue;
      out.push(rel);
    }
  }
  return out;
}

// Every `text("…")`/`text('…')` literal argument across the scanned files —
// not a substring search, so an unrelated identifier that merely contains a
// forbidden word (e.g. `formatBackfillProgress`) never gets flagged.
function collectTextCallSiteLiterals(): { file: string; value: string }[] {
  const files = listTextCallSiteFiles(process.cwd(), SCAN_ROOT, []);
  // Quoted strings AND static template literals (no `${…}`); dynamic template
  // literals, variables, formatter output and lookup maps are the table scan's job.
  const literalPattern = /\btext\(\s*(["'`])((?:(?!\1)(?!\$\{)[^\\]|\\.)*)\1/g;
  const literals: { file: string; value: string }[] = [];
  for (const rel of files) {
    const content = readFileSync(join(process.cwd(), rel), "utf8");
    for (const match of content.matchAll(literalPattern)) {
      literals.push({ file: rel, value: match[2] });
    }
  }
  return literals;
}

describe("dev-speak guard over every text() call site (F4c S1)", () => {
  it("keeps implementation vocabulary out of live src/** text() literals", () => {
    const found = collectTextCallSiteLiterals()
      .filter((literal) => FORBIDDEN.test(literal.value))
      .map((literal) => `${literal.file}: ${literal.value}`);
    expect(found, `Dev-speak in text() call sites:\n${found.join("\n")}`).toEqual([]);
  });
});
