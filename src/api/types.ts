export type Theme = "dark" | "light" | "system";
export type AppLocale = "en" | "pl";

export type HealthResponse = {
  status: string;
  version: string;
};

export type DatabaseStatus = {
  appliedMigrations: number;
  companies: number;
  sourceAdapters: number;
  settings: number;
};

export type SourceRefreshTrigger = "manual" | "scheduler";

export type FeedItem = {
  id: string;
  company: string;
  type: string;
  source: string;
  time: string;
  title: string;
  unread: boolean;
  saved: boolean;
  sourceUrl: string;
  language: string;
  publishedAt: string;
  fetchedAt: string;
  attribution: string;
  summary: string;
  bodyText: string;
  attachments: FeedItemAttachment[];
};

export type FeedItemAttachment = {
  id: string;
  label: string;
  url: string;
};

export type AiAnalysisJob = {
  id: string;
  feedItemId: string;
  promptPresetId: string;
  customQuestion: string | null;
  providerId: string;
  model: string;
  promptVersion: string;
  status: "queued" | "running" | "succeeded" | "failed" | "cancelled";
  errorCode: string | null;
  error: string | null;
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
  result: AiAnalysisResult | null;
};

export type AiAnalysisResult = {
  id: string;
  aiAnalysisJobId: string | null;
  feedItemId: string;
  providerId: string;
  model: string;
  promptVersion: string;
  summary: string;
  significance: "low" | "medium" | "high" | "unknown";
  reasoning: string;
  language: string | null;
  tags: string[];
  sourceReferences: AiAnalysisSourceReference[];
  createdAt: string;
};

export type AiAnalysisSourceReference = {
  id: string;
  sourceUrl: string;
  label: string | null;
  createdAt: string;
};

export type DiagnosticSeverity = "debug" | "info" | "warning" | "error";

export type DiagnosticScope = {
  type: string;
  id: string | null;
};

export type DiagnosticEvent = {
  id: string;
  occurredAt: string;
  module: string;
  scope: DiagnosticScope | null;
  stage: string;
  severity: DiagnosticSeverity;
  message: string;
  metadata: Record<string, unknown>;
  createdAt: string;
};

export type DiagnosticSummary = {
  summary: string;
  eventCount: number;
};

export type ClearDiagnosticEventsResult = {
  eventsDeleted: number;
};

export type LogStatus = {
  logsDir: string;
  currentFileBytes: number;
  rotatedFileCount: number;
  level: string;
  maxFiles: number;
  maxFileBytes: number;
};

export type LogEntry = {
  fileName: string;
  lineNumber: number;
  record: Record<string, unknown>;
};

export type SourceIngestionResult = {
  adapterId: string;
  itemsFetched: number;
  itemsCreated: number;
  itemsMatched: number;
  itemsUnmatched: number;
  detailItemsAttempted: number;
  detailItemsStored: number;
  detailItemsFailed: number;
  fetchedAt: string | null;
};

export type FeedDeleteResult = {
  itemsDeleted: number;
  deletedAt: string;
};

export type FeedPruneResult = {
  retentionDays: number;
  itemsDeleted: number;
  prunedAt: string;
};

export type CompanyRegistryRefreshResult = {
  adapterId: string;
  entriesFetched: number;
  entriesUpserted: number;
  entriesDeactivated: number;
  fetchedAt: string;
};

export type UnmatchedSourceItem = {
  id: string;
  adapterId: string;
  companyName: string;
  title: string;
  sourceUrl: string;
  publishedAt: string;
  fetchedAt: string;
};

export type Company = {
  id: string;
  exchange: string;
  ticker: string;
  qualifiedTicker: string;
  displayName: string;
  isin: string | null;
  cik: string | null;
  lei: string | null;
};

export type CompanyForm = {
  exchange: string;
  ticker: string;
  displayName: string;
  isin: string;
};

export type CompanyLookupResult = {
  exchange: string;
  ticker: string;
  qualifiedTicker: string;
  displayName: string;
  isin: string;
  source: string;
};

export type Watchlist = {
  id: string;
  name: string;
  description: string | null;
  companyCount: number;
};

export type WatchlistMembership = {
  watchlistId: string;
  watchlistName: string;
  companyId: string;
};

export type NotebookOrigin = {
  id: string;
  sourceType: string;
  sourceId: string | null;
  sourceUrl: string | null;
  label: string | null;
  createdAt: string;
};

export type NotebookEntry = {
  id: string;
  companyId: string;
  title: string;
  body: string;
  bodyFormat: string;
  tags: string[];
  kind: string;
  claimStatus: string | null;
  eventDate: string | null;
  followUpAfter: string | null;
  followUpDate: string | null;
  createdAt: string;
  updatedAt: string;
  origins: NotebookOrigin[];
};

export type NotebookDraftOrigin = {
  sourceType: string;
  sourceId: string | null;
  sourceUrl: string | null;
  label: string | null;
};

export type SourceAdapter = {
  id: string;
  displayName: string;
  sourceType: string;
  fetchMode: string;
  enabled: boolean;
  defaultPollIntervalSeconds: number;
  sourceUrl: string;
  rateLimitPolicy: string;
  policyNote: string;
  lastAttemptAt: string | null;
  lastTrigger: string | null;
  lastSuccessAt: string | null;
  lastErrorAt: string | null;
  lastError: string | null;
  lastItemsFetched: number | null;
  lastItemsCreated: number | null;
  lastItemsMatched: number | null;
  lastItemsUnmatched: number | null;
  lastDetailItemsAttempted: number | null;
  lastDetailItemsStored: number | null;
  lastDetailItemsFailed: number | null;
  lastDetailWarning: string | null;
  markets: string[];
};

export type CompanyRegistryEntry = {
  exchange: string;
  ticker: string;
  qualifiedTicker: string;
  displayName: string;
  isin: string | null;
  sourceUrl: string;
  fetchedAt: string;
  tracked: boolean;
};

export type CompanyEvent = {
  id: string;
  companyId: string;
  company: string;
  companyName: string;
  eventType: string;
  title: string;
  eventDate: string;
  eventTime: string | null;
  status: string;
  sourceType: string;
  sourceAdapterId: string | null;
  sourceEventKey: string | null;
  sourceUrl: string | null;
  attribution: string | null;
  fetchedAt: string | null;
  manual: boolean;
  createdAt: string;
  updatedAt: string;
};

export type TranscriptJob = {
  id: string;
  companyId: string | null;
  company: string | null;
  companyName: string | null;
  providerId: string;
  sourceType: string;
  sourceUrl: string;
  sourceLabel: string | null;
  companyResolutionStatus: string;
  recognizedCompanyCandidates: CompanyLookupResult[];
  status: string;
  errorCode: string | null;
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
  error: string | null;
};

export type TranscriptSegment = {
  id: string;
  transcriptJobId: string;
  companyId: string | null;
  startSeconds: number | null;
  endSeconds: number | null;
  speaker: string | null;
  text: string;
  language: string | null;
  createdAt: string;
};

export type ShortcutKeyBinding = {
  key: string;
  altKey?: boolean;
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
};

export type ShortcutBindingSetting = ShortcutKeyBinding & {
  disabled?: boolean;
};

export type UserSettings = {
  theme: Theme;
  locale: AppLocale;
  accentPalette: string;
  developerMode: boolean;
  pollIntervalSeconds: number;
  settingsSource: string;
  settingsImportExportFormat: string;
  yamlImportExportStatus: string;
  aiProviders: {
    youtubeTranscriptionProvider: string;
    youtubeTranscriptionModel: string;
    youtubeTranscriptionTimeoutSeconds: number;
    generalAnalysisProvider: string | null;
    generalAnalysisModel: string;
    generalAnalysisTimeoutSeconds: number;
  };
  aiAnalysisMode: string;
  logs: {
    level: string;
    maxFiles: number;
    maxFileBytes: number;
  };
  shortcutBindings: Record<string, ShortcutBindingSetting>;
};

export type CredentialStatus = {
  providerId: string;
  purpose: string;
  secretKind: string;
  configured: boolean;
  storage: string;
  label: string;
  devFallbackAvailable: boolean;
  error: string | null;
};
