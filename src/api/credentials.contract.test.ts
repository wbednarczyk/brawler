import { describe, it, expect } from "vitest";
import { ADDITIONAL_CREDENTIAL_PROVIDERS } from "./credentials";
import credentialProviderIds from "../test/scenarios/credentialProviders.json";

// Guardrail (owner report 2026-07-14, origin: a market-data credential once
// existed backend-side with NO Settings field — the key was physically
// un-enterable). The shared fixture is pinned to the Rust
// `CREDENTIAL_PROVIDER_IDS` enumeration (providers::credentials tests); this
// side asserts every enumerated provider has a Settings entry point: either
// the dedicated Gemini form or a row in ADDITIONAL_CREDENTIAL_PROVIDERS.
describe("credential providers ↔ Settings forms contract", () => {
  it("every backend credential descriptor is enterable from Settings", () => {
    const formIds = new Set(ADDITIONAL_CREDENTIAL_PROVIDERS.map((entry) => entry.providerId));
    const missing = credentialProviderIds.filter(
      (id) => id !== "provider_gemini" && !formIds.has(id),
    );
    expect(
      missing,
      `providers with a backend credential descriptor but NO Settings form: ${missing.join(", ")}`,
    ).toEqual([]);
  });
});
