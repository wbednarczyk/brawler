// Fact provenance/quality display helpers, extracted from
// FundamentalsPanel.tsx (ADR 0103 file-size ratchet, dogfooding wave 2026-09)
// so FundamentalsFactsMatrix.tsx can share them without a circular import.

/**
 * Human-readable source tier (ADR 0061). ESEF is the tagged source of truth;
 * `ai` (Radicle 4fde931) is the AI-confirmed-fact tier the deterministic pool
 * falls through to. A pure module-level function so it is unit-testable
 * without rendering.
 */
export function tierLabel(tier: string, text: (value: string) => string): string {
  switch (tier) {
    case "esef":
      return text("ESEF (tagged)");
    case "structured_xhtml":
      return text("Structured HTML");
    case "espi_cover_note":
      return text("ESPI cover note");
    case "pdf":
      return text("Structured read (xHTML)");
    case "agent":
      return text("Agent (MCP)");
    case "html_aggregator":
      return text("Aggregator");
    case "ai_text":
    case "ai":
      return text("AI");
    default:
      return tier;
  }
}

/**
 * Human-readable `data_quality` label (ADR 0093 dec. 2: `final | preliminary
 * | estimated`, canonical vocabulary). A pure module-level function so it is
 * unit-testable without rendering. Any unrecognized token (a future addition
 * to the vocabulary, or a legacy row) falls back to "Final" — matching the
 * storage-layer default and never blocking the value.
 */
export function factQualityLabel(quality: string, text: (value: string) => string): string {
  switch (quality) {
    case "preliminary":
      return text("Preliminary");
    case "estimated":
      return text("Estimated");
    default:
      return text("Final");
  }
}

/** Chip tone for {@link factQualityLabel} — caution for preliminary (issuer-
 * published, pending the audited figure), accent for estimated (third-party
 * derived), neutral for the default final. */
export function factQualityTone(quality: string): "warn" | "accent" | "neutral" {
  switch (quality) {
    case "preliminary":
      return "warn";
    case "estimated":
      return "accent";
    default:
      return "neutral";
  }
}
