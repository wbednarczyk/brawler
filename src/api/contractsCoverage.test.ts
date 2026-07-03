import { describe, expect, it } from "vitest";

// Enforcement: every `callCommand("x")` invocation in src/api/*.ts is a frontend-facing
// contract that must be documented in docs/contracts.md. Without this gate a new command
// can ship with a working `src/api` wrapper and no spec entry, silently drifting the doc
// from the real command surface (Radicle 311a586).
//
// This is a hard gate, not a ratchet/baseline: docs/contracts.md is the source of intent
// for this spec-driven repo, so every command name found in src/api must appear there.

const apiModules = import.meta.glob("/src/api/*.ts", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const contractsModule = import.meta.glob("/docs/contracts.md", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

function commandNames(): Set<string> {
  const out = new Set<string>();
  for (const [path, src] of Object.entries(apiModules)) {
    if (/\.test\./.test(path)) continue;
    for (const match of src.matchAll(/callCommand(?:<[^>]*>)?\(\s*"([a-z_]+)"/g)) {
      out.add(match[1]);
    }
  }
  return out;
}

const contractsDoc = Object.values(contractsModule)[0] ?? "";

describe("docs/contracts.md command coverage", () => {
  it("documents every callCommand(...) invocation in src/api", () => {
    const commands = commandNames();
    const missing = [...commands].filter((name) => !contractsDoc.includes(name)).sort();
    expect(
      missing,
      `Commands called from src/api but missing from docs/contracts.md — add a contract entry:\n${missing.join("\n")}`,
    ).toEqual([]);
  });
});
