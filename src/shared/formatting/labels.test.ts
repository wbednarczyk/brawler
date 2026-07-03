import { describe, expect, it } from "vitest";
import { formatAiProvider } from "./labels";

describe("formatAiProvider", () => {
  it("labels every registered provider id and the capability-routed sentinel", () => {
    expect(formatAiProvider("provider_gemini")).toBe("Gemini");
    expect(formatAiProvider("provider_anthropic")).toBe("Claude (Anthropic)");
    expect(formatAiProvider("provider_openai")).toBe("OpenAI (ChatGPT)");
    expect(formatAiProvider("provider_openai_compatible")).toBe("OpenAI-compatible (custom)");
    // A capability-routed job row stores this sentinel instead of a concrete
    // provider (ADR 0060); it must never leak raw into the UI.
    expect(formatAiProvider("capability_routed")).toBe("Routed by capability");
  });

  it("falls back to the raw value and handles missing config", () => {
    expect(formatAiProvider("provider_future")).toBe("provider_future");
    expect(formatAiProvider(null)).toBe("Not configured");
    expect(formatAiProvider(undefined)).toBe("Not configured");
  });
});
