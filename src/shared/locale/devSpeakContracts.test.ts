import { describe, expect, it } from "vitest";
import { en } from "./resources/en";
import { pl } from "./resources/pl";
import { plText } from "./resources/plText";

// Dev-speak guard (v0.50 UX audit K6, ADR 0045 harvest): user-facing copy must
// speak the user's language, not the implementation's. "Database" leaked into
// Settings once; this pins the class shut. Keep the list tight and justified —
// a broad list would flag legitimate copy and erode trust in the gate.
const FORBIDDEN = /\b(database|databases|IPC|SQL|runtime|backend|jsdom|localhost)\b/i;

function offenders(strings: string[], source: string): string[] {
  return strings
    .filter((value) => FORBIDDEN.test(value))
    .map((value) => `${source}: ${value}`);
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
