import { describe, expect, it } from "vitest";

// Enforcement (ADR 0076 Decision 4): `src/shared/format/` is the ONLY formatting
// layer. This gate fails when:
//   1. anything imports the two removed formatters (`shared/formatting/date`,
//      `shared/format/timestamp`) — their split brain caused the dual-formatter
//      and "oday 09:12" bugs;
//   2. render code under `src/screens/**` or `src/shared/components/**` calls
//      `toLocaleString` / `toISOString` / `new Intl.NumberFormat` directly — the
//      format layer (financialValue / datetime) is the only sanctioned caller.

const sourceModules = import.meta.glob("/src/**/*.{ts,tsx}", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const isTest = (path: string) => /\.test\.|\/test\//.test(path);

describe("format-layer contract (ADR 0076 D4)", () => {
  it("nothing imports the removed date formatters", () => {
    const offenders: string[] = [];
    for (const [path, src] of Object.entries(sourceModules)) {
      if (isTest(path)) continue;
      if (/from\s+["'][^"']*shared\/formatting\/date["']/.test(src)) {
        offenders.push(`${path} → shared/formatting/date`);
      }
      if (/from\s+["'][^"']*shared\/format\/timestamp["']/.test(src)) {
        offenders.push(`${path} → shared/format/timestamp`);
      }
    }
    expect(
      offenders,
      `Removed formatters still imported — use src/shared/format/datetime instead:\n${offenders.join("\n")}`,
    ).toEqual([]);
  });

  it("no direct locale/number formatting in screens or shared components", () => {
    const banned = /\.toLocaleString\(|\.toISOString\(|new\s+Intl\.NumberFormat/;
    const offenders: string[] = [];
    for (const [path, src] of Object.entries(sourceModules)) {
      if (isTest(path)) continue;
      if (!/^\/src\/(screens|shared\/components)\//.test(path)) continue;
      if (banned.test(src)) offenders.push(path);
    }
    expect(
      offenders,
      `Direct date/number formatting found — route through src/shared/format (datetime / financialValue):\n${offenders.join("\n")}`,
    ).toEqual([]);
  });
});
