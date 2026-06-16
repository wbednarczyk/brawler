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
