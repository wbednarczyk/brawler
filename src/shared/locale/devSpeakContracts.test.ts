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
const FORBIDDEN = /\b(database|databases|IPC|SQL|runtime|backend|jsdom|localhost|jobs?)\b/i;

function offenders(strings: string[], source: string, pattern: RegExp = FORBIDDEN): string[] {
  return strings.filter((value) => pattern.test(value)).map((value) => `${source}: ${value}`);
}

describe("dev-speak guard over user-facing copy", () => {
  it("keeps implementation vocabulary out of every locale surface", () => {
    const found = [
      // plText keys ARE the English user-facing strings (text() passthrough).
      ...offenders(Object.keys(plText), "plText key"),
      ...offenders(Object.values(plText), "plText pl value"),
      ...offenders(Object.values(en), "en value"),
      ...offenders(Object.values(pl), "pl value"),
    ];
    expect(found, `Dev-speak in user-facing copy:\n${found.join("\n")}`).toEqual([]);
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
