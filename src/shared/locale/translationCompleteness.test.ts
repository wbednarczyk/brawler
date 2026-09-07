import { describe, expect, it } from "vitest";

// Enforcement (ADR 0037): a `text("…")` literal with no `plText` entry silently renders English
// in the Polish UI — the "Clear"/"Credential status" mixed-language regression.
//
// This is a RATCHET, deliberately not a hard "everything must be translated" gate (there is a
// real backlog of ~100 untranslated strings, mostly in diagnostics/dev screens). The current
// backlog is captured in untranslated-baseline.json. The test fails only when a NEW untranslated
// string appears. The fix is to translate it (preferred) or, for an intentionally-English string
// (acronyms, brand names), add it to the baseline as a conscious choice. The baseline should
// shrink over time, never grow casually.

// All source files (raw text), excluding tests, via Vite's glob — avoids node fs typings.
const sourceModules = import.meta.glob("/src/**/*.{ts,tsx}", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const baselineModule = import.meta.glob("/src/shared/locale/untranslated-baseline.json", {
  import: "default",
  eager: true,
});

const identicalEntriesModule = import.meta.glob("/src/shared/locale/identicalEntries.json", {
  import: "default",
  eager: true,
});

function textLiterals(): Set<string> {
  const out = new Set<string>();
  for (const [path, src] of Object.entries(sourceModules)) {
    if (/\.test\.|\/test\//.test(path)) continue;
    for (const match of src.matchAll(/\btext\(\s*"((?:[^"\\]|\\.)*)"/g)) {
      out.add(match[1]);
    }
  }
  return out;
}

function plTextKeys(): Set<string> {
  const src = sourceModules["/src/shared/locale/resources/plText.ts"] ?? "";
  return new Set([...src.matchAll(/^\s*"((?:[^"\\]|\\.)*)"\s*:/gm)].map((m) => m[1]));
}

const baseline = new Set(
  Object.values(baselineModule)[0] as string[],
);

const identicalEntries = Object.values(identicalEntriesModule)[0] as Record<string, string>;

// G12 (English-in-PL detector, dogfooding #8 class — "Official report" shipped
// in English inside the PL UI and the retro F4c fix was instance-only): a PL
// resource entry byte-identical to its EN key is either a deliberate
// loanword/brand/acronym (allowlisted with a reason) or a missed translation
// (a real defect — fix the PL text, don't allowlist it). Scans plText.ts
// entries directly (key === value), independent of whether a static
// `text("…")` literal reaches them, so it also catches entries reached only
// via a dynamic key (e.g. `text(formatEvidenceType(x))`).
function plResourceEntries(): Map<string, string> {
  const src = sourceModules["/src/shared/locale/resources/plText.ts"] ?? "";
  const entries = new Map<string, string>();
  for (const match of src.matchAll(/"((?:[^"\\]|\\.)*)"\s*:\s*"((?:[^"\\]|\\.)*)"/g)) {
    entries.set(match[1], match[2]);
  }
  return entries;
}

describe("Polish translation completeness", () => {
  const literals = textLiterals();
  const translated = plTextKeys();

  it("has no NEW untranslated text() strings beyond the known backlog", () => {
    const missing = [...literals].filter((s) => !translated.has(s) && !baseline.has(s)).sort();
    expect(
      missing,
      `New untranslated UI strings — add a plText entry, or add to untranslated-baseline.json if intentionally English:\n${missing.join("\n")}`,
    ).toEqual([]);
  });

  it("keeps the backlog honest (no stale baseline entries now translated or unused)", () => {
    const stale = [...baseline].filter((s) => translated.has(s) || !literals.has(s)).sort();
    expect(
      stale,
      `Baseline entries that are now translated or no longer used — remove them from untranslated-baseline.json:\n${stale.join("\n")}`,
    ).toEqual([]);
  });
});

describe("English-in-PL detector (G12, dogfooding #8 class)", () => {
  const resourceEntries = plResourceEntries();
  const identical = [...resourceEntries].filter(([key, value]) => key === value).map(([key]) => key);

  it("has no NEW PL entry identical to its EN key beyond identicalEntries.json", () => {
    const missing = identical.filter((key) => !(key in identicalEntries)).sort();
    expect(
      missing,
      `PL entry identical to EN — translate it or add it to identicalEntries.json with a reason:\n${missing.join("\n")}`,
    ).toEqual([]);
  });

  it("keeps identicalEntries.json honest (no stale entries that are no longer identical)", () => {
    const identicalSet = new Set(identical);
    const stale = Object.keys(identicalEntries)
      .filter((key) => !identicalSet.has(key))
      .sort();
    expect(
      stale,
      `identicalEntries.json entries no longer identical to their EN key (or no longer present) — remove them:\n${stale.join("\n")}`,
    ).toEqual([]);
  });
});
