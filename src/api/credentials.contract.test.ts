import { describe, it, expect } from "vitest";
import credentialProviderIds from "../test/scenarios/credentialProviders.json";

// Guardrail (owner report 2026-07-14, origin: a market-data credential once
// existed backend-side with NO Settings field — the key was physically
// un-enterable). The shared fixture is pinned to the Rust
// `CREDENTIAL_PROVIDER_IDS` enumeration (providers::credentials tests); this
// side asserts every enumerated provider has a Settings entry point.
//
// ADR 0084: transcription is the only remaining model-backed capability, so
// `provider_gemini` and its Settings → Credentials form are the whole surface.
// Adding a backend credential descriptor without a Settings form must redden
// here, not ship un-enterable.
const PROVIDERS_WITH_A_SETTINGS_FORM: ReadonlyArray<string> = [
  // Settings → Credentials, the dedicated Gemini API-key form.
  "provider_gemini",
];

describe("credential providers ↔ Settings forms contract", () => {
  it("every backend credential descriptor is enterable from Settings", () => {
    const formIds = new Set(PROVIDERS_WITH_A_SETTINGS_FORM);
    const missing = credentialProviderIds.filter((id) => !formIds.has(id));
    expect(
      missing,
      `providers with a backend credential descriptor but NO Settings form: ${missing.join(", ")}`,
    ).toEqual([]);
  });
});
