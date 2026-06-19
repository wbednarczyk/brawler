// ============================================================================
// API DTOs
//
// Most types below are GENERATED from the Rust source via ts-rs (ADR 0048):
// change the struct in src-tauri and run `make types`. The handful of
// FRONTEND-ONLY types (string-literal unions and input shapes with no backing
// Rust DTO) are declared here by hand — there is no struct to generate them
// from. Do not hand-edit a shape that has a generated binding; edit the Rust
// struct and regenerate.
// ============================================================================

// --- Frontend-only union + input types (no Rust source) ---

export type Theme = "dark" | "light" | "system";
export type AccentPalette = "night-neon" | "midnight-horizon";
export type AppLocale = "en" | "pl";
export type SourceRefreshTrigger = "manual" | "scheduler";
export type DiagnosticSeverity = "debug" | "info" | "warning" | "error";

// Typed ESPI/EBI classification signal categories (ADR 0034). The Rust
// `CompanySignal.category` is a `String`; this union narrows it for the UI and
// is mirrored by an inline `#[ts(type = ...)]` override on the generated DTO.
export type CompanySignalCategory =
  | "insider_transaction"
  | "dividend"
  | "profit_warning"
  | "significant_contract"
  | "own_shares"
  | "guidance_change"
  | "general_meeting"
  | "other";

export type CompanyForm = {
  exchange: string;
  ticker: string;
  displayName: string;
  isin: string;
};

export type NotebookDraftOrigin = {
  sourceType: string;
  sourceId: string | null;
  sourceUrl: string | null;
  label: string | null;
};

export type ShortcutKeyBinding = {
  key: string;
  altKey?: boolean;
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
};

// --- Generated DTOs (re-exported from ./generated; run `make types`) ---

export type { HealthResponse } from "./generated/HealthResponse";
export type { DatabaseStatus } from "./generated/DatabaseStatus";

export type { FeedItem } from "./generated/FeedItem";
export type { FeedItemAttachment } from "./generated/FeedItemAttachment";
export type { FeedDeleteResult } from "./generated/FeedDeleteResult";
export type { FeedPruneResult } from "./generated/FeedPruneResult";

export type { AiAnalysisJob } from "./generated/AiAnalysisJob";
export type { AiAnalysisResult } from "./generated/AiAnalysisResult";
export type { AiAnalysisSourceReference } from "./generated/AiAnalysisSourceReference";

export type { DiagnosticScope } from "./generated/DiagnosticScope";
export type { DiagnosticEvent } from "./generated/DiagnosticEvent";
export type { DiagnosticSummary } from "./generated/DiagnosticSummary";
export type { ClearDiagnosticEventsResult } from "./generated/ClearDiagnosticEventsResult";

export type { LogStatus } from "./generated/LogStatus";
export type { LogEntry } from "./generated/LogEntry";

export type { MetricKind } from "./generated/MetricKind";
export type { MetricUnit } from "./generated/MetricUnit";
export type { MetricLabel } from "./generated/MetricLabel";
export type { MetricSample } from "./generated/MetricSample";
export type { LocalMetricsSnapshot } from "./generated/LocalMetricsSnapshot";

export type { LicenseStatusKind } from "./generated/LicenseStatusKind";
export type { LicenseDisplayMetadata } from "./generated/LicenseDisplayMetadata";
export type { LicenseStatus } from "./generated/LicenseStatus";

export type { SourceIngestionResult } from "./generated/SourceIngestionResult";
export type { UnmatchedSourceItem } from "./generated/UnmatchedSourceItem";
export type { SourceAdapter } from "./generated/SourceAdapter";
export type { CompanyRegistryEntry } from "./generated/CompanyRegistryEntry";
export type { CompanyRegistryRefreshResult } from "./generated/CompanyRegistryRefreshResult";

export type { Company } from "./generated/Company";
export type { CompanyLookupResult } from "./generated/CompanyLookupResult";
export type { CompanyEvent } from "./generated/CompanyEvent";
export type { CompanySignal } from "./generated/CompanySignal";

export type { Watchlist } from "./generated/Watchlist";
export type { WatchlistMembership } from "./generated/WatchlistMembership";

export type { NotebookOrigin } from "./generated/NotebookOrigin";
export type { NotebookEntry } from "./generated/NotebookEntry";

export type { AiSignalClassificationSummary } from "./generated/AiSignalClassificationSummary";
export type { AiEventDerivationSummary } from "./generated/AiEventDerivationSummary";
export type { BackfillProgress } from "./generated/BackfillProgress";

export type { TranscriptJob } from "./generated/TranscriptJob";
export type { TranscriptSegment } from "./generated/TranscriptSegment";

export type { UserSettings } from "./generated/UserSettings";
export type { AiProviderSettings } from "./generated/AiProviderSettings";
export type { LogSettings } from "./generated/LogSettings";
export type { DatabaseSettings } from "./generated/DatabaseSettings";
export type { ShortcutBindingSetting } from "./generated/ShortcutBindingSetting";
export type { AiProviderCatalogEntry } from "./generated/AiProviderCatalogEntry";
export type { CredentialStatus } from "./generated/CredentialStatus";
