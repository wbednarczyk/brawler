// Fact-quality display helpers (ADR 0093 dec. 2), extracted from
// FundamentalsPanel.tsx (ADR 0103 file-size ratchet, dogfooding wave 2026-09)
// so FundamentalsFactsMatrix.tsx can share them without a circular import.

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
