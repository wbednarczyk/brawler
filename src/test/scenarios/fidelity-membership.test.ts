import { describe, expect, it } from "vitest";

import baseline from "./fidelity-membership.baseline.json";
import corpus from "./fidelity-corpus.json";
import headlessOnly from "./headless-only.json";

// Fidelity-corpus MEMBERSHIP gate (ADR 0049 / testing.md § Mock-runtime
// fidelity) — distinct from fidelity.test.ts (which replays the corpus).
// This test asks a narrower question: does every `#[tauri::command]` in the
// Rust source have an accounted-for fate? A command is accounted for when it
// is one of:
//   1. a corpus step (`fidelity-corpus.json`) — the dual-execution contract
//      actually exercises it;
//   2. declared `headless-only.json` — no mock half exists (MCP-only or a
//      headless acquisition driver), with a one-line reason;
//   3. a `fidelity-membership.baseline.json` entry — today's known offender,
//      the ratchet floor. The baseline may only SHRINK: a command that has
//      since gained a corpus step must be removed from it in the same change.
// A command missing from all three fails the build with the three ways out.

// Rust sources read through Vite's glob (the repo's pattern for source-scan
// tests — the frontend tsconfig carries no Node types), eager + raw.
const rustSources = import.meta.glob("/src-tauri/src/**/*.rs", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

function extractCommandNames(): string[] {
  const names: string[] = [];
  for (const [, content] of Object.entries(rustSources)) {
    for (const match of content.matchAll(/#\[tauri::command\]/g)) {
      // The attribute is followed by 0-3 lines of doc comments/other
      // attributes before `fn <name>` — scan the next few lines rather than
      // assuming it is the very next token.
      const after = content.slice(match.index! + match[0].length);
      const window = after.split("\n").slice(0, 4).join("\n");
      const fnMatch = window.match(/fn\s+([A-Za-z0-9_]+)/);
      if (fnMatch) names.push(fnMatch[1]);
    }
  }
  return names;
}

function corpusCommandNames(): Set<string> {
  const names = new Set<string>();
  const walk = (value: unknown): void => {
    if (Array.isArray(value)) {
      value.forEach(walk);
    } else if (value && typeof value === "object") {
      const obj = value as Record<string, unknown>;
      if (typeof obj.command === "string") names.add(obj.command);
      Object.values(obj).forEach(walk);
    }
  };
  walk(corpus);
  return names;
}

// B1 (owner-approved hard gate 2026-09-07): the baseline is today's known
// offender list, not a way for a NEW offender to go green — the only ways
// out of the "every command must be accounted for" test below are a corpus
// step or a headless-only declaration. This ceiling is the second lock: it
// may only be LOWERED as entries are fixed, never raised to fit a new one.
const BASELINE_CEILING = 117;

describe("fidelity-corpus membership (ADR 0049)", () => {
  const registered = new Set(extractCommandNames());
  const inCorpus = corpusCommandNames();
  const headlessNames = new Set(Object.keys(headlessOnly as Record<string, string>));
  const baselineNames = new Set(baseline as string[]);

  it("found the expected commands and manifest sizes (sanity check)", () => {
    expect(registered.size).toBeGreaterThan(0);
    expect(inCorpus.size).toBeGreaterThan(0);
  });

  it("every #[tauri::command] is a corpus step, declared headless-only, or a known baseline offender", () => {
    const unaccounted = [...registered].filter(
      (name) => !inCorpus.has(name) && !headlessNames.has(name) && !baselineNames.has(name),
    );
    expect(
      unaccounted,
      `${unaccounted.length} command(s) have no fidelity-corpus step, no headless-only.json ` +
        "entry, and are not in the baseline. The baseline is NOT a way to go green for a NEW " +
        "offender — fix by (a) adding a journey step to src/test/scenarios/fidelity-corpus.json, " +
        "or (b) declaring the command in src/test/scenarios/headless-only.json with a one-line " +
        "reason (no mock half — MCP-only or a headless acquisition driver). " +
        `Unaccounted: ${unaccounted.join(", ")}`,
    ).toEqual([]);
  });

  it("the baseline ratchet only shrinks (an entry now covered by the corpus must be removed)", () => {
    const stale = [...baselineNames].filter((name) => inCorpus.has(name));
    expect(
      stale,
      `${stale.length} baseline entr(y/ies) already have a fidelity-corpus step ` +
        `and must be removed from fidelity-membership.baseline.json: ${stale.join(", ")}`,
    ).toEqual([]);
  });

  it("the baseline ceiling only shrinks, never grows (BASELINE_CEILING is the floor for new offenders)", () => {
    expect(
      baselineNames.size,
      `fidelity-membership.baseline.json has ${baselineNames.size} entries, above ` +
        `BASELINE_CEILING (${BASELINE_CEILING}) in fidelity-membership.test.ts. The ceiling may ` +
        "only be lowered as entries are fixed, never raised — a new offender must go through the " +
        "corpus or headless-only.json instead, never the baseline.",
    ).toBeLessThanOrEqual(BASELINE_CEILING);
  });

  it("every baseline entry is still a real #[tauri::command] (stale entries must be removed)", () => {
    const stale = [...baselineNames].filter((name) => !registered.has(name));
    expect(
      stale,
      `${stale.length} baseline entr(y/ies) no longer name a #[tauri::command] — remove them ` +
        `from fidelity-membership.baseline.json: ${stale.join(", ")}`,
    ).toEqual([]);
  });

  it("every headless-only.json key is a real #[tauri::command]", () => {
    const stale = [...headlessNames].filter((name) => !registered.has(name));
    expect(
      stale,
      `${stale.length} headless-only.json entr(y/ies) no longer name a #[tauri::command] — ` +
        `remove them: ${stale.join(", ")}`,
    ).toEqual([]);
  });

  it("every headless-only.json value is a non-empty reason", () => {
    const empty = Object.entries(headlessOnly as Record<string, string>)
      .filter(([, reason]) => reason.trim().length === 0)
      .map(([name]) => name);
    expect(
      empty,
      `headless-only.json entr(y/ies) with an empty reason: ${empty.join(", ")}`,
    ).toEqual([]);
  });
});
