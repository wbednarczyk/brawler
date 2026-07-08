import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { CAPABILITY_DESCRIPTORS } from "./CapabilityRoutingSettings";

// Guardrail (ADR 0045): every backend `AiCapability` variant must be routable
// from Settings → AI, or the user cannot assign a provider pool to it. This has
// regressed twice ("new AiCapability not added to the settings list"), most
// recently QualitativeAssessment (ADR 0075, T7-C). The Rust `AiCapability::key`
// match arms are the canonical enumeration; parse them and assert the frontend
// routing list covers every one.
//
// If a capability is ever made deliberately non-routable from the UI, add its
// key to NON_ROUTABLE_CAPABILITY_KEYS below with a comment justifying it — do
// not silently drop it from the assertion.
const NON_ROUTABLE_CAPABILITY_KEYS: ReadonlyArray<string> = [];

function backendCapabilityKeys(): string[] {
  const source = readFileSync(
    join(process.cwd(), "src-tauri", "src", "providers", "analysis", "capabilities.rs"),
    "utf8",
  );
  // Isolate the `fn key(self) -> &'static str { match self { ... } }` body so
  // we only capture the wire keys, not doc-comment or test string literals.
  const keyFnMatch = source.match(/fn key\(self\)[^{]*\{\s*match self \{(?<arms>[\s\S]*?)\}\s*\}/);
  const arms = keyFnMatch?.groups?.arms;
  if (!arms) {
    throw new Error("could not locate AiCapability::key match arms in capabilities.rs");
  }
  return [...arms.matchAll(/=>\s*"(?<key>[a-z_]+)"/g)].map((match) => match.groups!.key);
}

describe("capability routing list covers every backend AiCapability", () => {
  const backendKeys = backendCapabilityKeys();
  const frontendKeys = new Set(CAPABILITY_DESCRIPTORS.map((descriptor) => descriptor.key));

  it("parses a plausible set of backend capability keys", () => {
    // Sanity: the parse found the enum, not zero arms.
    expect(backendKeys.length).toBeGreaterThanOrEqual(7);
    expect(backendKeys).toContain("kpi_extraction");
  });

  it.each(backendKeys)("routable capability %s appears in the settings routing list", (key) => {
    if (NON_ROUTABLE_CAPABILITY_KEYS.includes(key)) {
      expect(frontendKeys.has(key)).toBe(false);
      return;
    }
    expect(frontendKeys.has(key)).toBe(true);
  });

  it("has no frontend routing entry without a matching backend capability", () => {
    const backendKeySet = new Set(backendKeys);
    for (const key of frontendKeys) {
      expect(backendKeySet.has(key)).toBe(true);
    }
  });
});
