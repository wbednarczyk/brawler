import {
  type ReactNode,
  type CSSProperties,
  type FormEvent,
  type KeyboardEvent,
  type PointerEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Activity,
  BookOpenText,
  Building2,
  CalendarDays,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ExternalLink,
  FileText,
  Inbox,
  LocateFixed,
  Moon,
  Plus,
  RefreshCw,
  Save,
  Search,
  Settings,
  Sun,
  Trash2,
  Mail,
  MailOpen,
  X,
  Video,
} from "lucide-react";

type Theme = "dark" | "light" | "system";

type HealthResponse = {
  status: string;
  version: string;
};

type DatabaseStatus = {
  appliedMigrations: number;
  companies: number;
  sourceAdapters: number;
  settings: number;
};

type Section = "Inbox" | "Companies" | "Notebooks" | "Events" | "Transcripts" | "Sources" | "Settings";
type CompanyWorkspaceTab = "Feed" | "Notebook" | "Claims" | "Transcripts" | "Metadata";

type InboxStatusFilter = "all" | "unread" | "saved";
type DbRefreshState = "idle" | "refreshing" | "done";
type SourceRefreshState = "idle" | "refreshing" | "done";
type SourceRefreshTrigger = "manual" | "scheduler";
const gpwRegistryAdapterId = "gpw-company-registry";
const eventSourceAdapterIds = ["gpw-market-events-rss", "bankier-kalendarium-html"];
const feedPruneRetentionDays = 30;
const feedPruneIntervalMs = 24 * 60 * 60 * 1000;
const feedPruneInitialDelayMs = 2 * 60 * 1000;
const companyEventTypeOptions = [
  "periodic_report",
  "corporate_action",
  "dividend",
  "shareholder_meeting",
  "conference_call",
  "investor_conference",
  "market_making",
  "listing_change",
  "other_market_event",
  "custom",
];
const companyEventStatusOptions = [
  "scheduled",
  "confirmed",
  "tentative",
  "changed",
  "cancelled",
  "completed",
];

type FeedItem = {
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

type FeedItemAttachment = {
  id: string;
  label: string;
  url: string;
};

type SourceIngestionResult = {
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

type FeedDeleteResult = {
  itemsDeleted: number;
  deletedAt: string;
};

type FeedPruneResult = {
  retentionDays: number;
  itemsDeleted: number;
  prunedAt: string;
};

type CompanyRegistryRefreshResult = {
  adapterId: string;
  entriesFetched: number;
  entriesUpserted: number;
  entriesDeactivated: number;
  fetchedAt: string;
};

type UnmatchedSourceItem = {
  id: string;
  adapterId: string;
  companyName: string;
  title: string;
  sourceUrl: string;
  publishedAt: string;
  fetchedAt: string;
};

type Company = {
  id: string;
  exchange: string;
  ticker: string;
  qualifiedTicker: string;
  displayName: string;
  isin: string | null;
  cik: string | null;
  lei: string | null;
};

type CompanyForm = {
  exchange: string;
  ticker: string;
  displayName: string;
  isin: string;
};

type CompanyLookupResult = {
  exchange: string;
  ticker: string;
  qualifiedTicker: string;
  displayName: string;
  isin: string;
  source: string;
};

type Watchlist = {
  id: string;
  name: string;
  description: string | null;
  companyCount: number;
};

type WatchlistMembership = {
  watchlistId: string;
  watchlistName: string;
  companyId: string;
};

type WatchlistFeedback = {
  companyId: string;
  message: string;
};

type NotebookOrigin = {
  id: string;
  sourceType: string;
  sourceId: string | null;
  sourceUrl: string | null;
  label: string | null;
  createdAt: string;
};

type NotebookEntry = {
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

type NotebookForm = {
  title: string;
  body: string;
  tags: string;
  kind: string;
  claimStatus: string;
  eventDate: string;
  followUpAfter: string;
  followUpDate: string;
};

type NotebookDraftOrigin = {
  sourceType: string;
  sourceId: string | null;
  sourceUrl: string | null;
  label: string | null;
};

type SourceAdapter = {
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

type CompanyRegistryEntry = {
  exchange: string;
  ticker: string;
  qualifiedTicker: string;
  displayName: string;
  isin: string | null;
  sourceUrl: string;
  fetchedAt: string;
  tracked: boolean;
};

type CompanyEvent = {
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

type CompanyEventMode = "upcoming" | "historical" | "all";
type CompanyEventViewMode = "week" | "list";
type CompanyEventForm = {
  companyId: string;
  eventType: string;
  title: string;
  eventDate: string;
  eventTime: string;
  status: string;
};

type TranscriptJob = {
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

type TranscriptSegment = {
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

type TranscriptJobForm = {
  url: string;
  label: string;
  companyQuery: string;
  companyId: string;
};

type UserSettings = {
  theme: Theme;
  accentPalette: string;
  pollIntervalSeconds: number;
  settingsSource: string;
  settingsImportExportFormat: string;
  yamlImportExportStatus: string;
  aiProviders: {
    youtubeTranscriptionProvider: string;
    youtubeTranscriptionModel: string;
    youtubeTranscriptionTimeoutSeconds: number;
    generalAnalysisProvider: string | null;
  };
  aiAnalysisMode: string;
};

type CredentialStatus = {
  providerId: string;
  purpose: string;
  secretKind: string;
  configured: boolean;
  storage: string;
  label: string;
  devFallbackAvailable: boolean;
  error: string | null;
};

function databaseIndicatorClass(status: DatabaseStatus | null, error: string | null) {
  if (error) {
    return "status-dot status-danger";
  }

  if (status) {
    return "status-dot status-ok";
  }

  return "status-dot status-warn";
}

const sections = [
  { label: "Inbox" as const, icon: Inbox },
  { label: "Companies" as const, icon: Building2 },
  { label: "Notebooks" as const, icon: BookOpenText },
  { label: "Events" as const, icon: CalendarDays },
  { label: "Transcripts" as const, icon: Video },
  { label: "Sources" as const, icon: Activity },
  { label: "Settings" as const, icon: Settings },
];

const detailPaneMinWidth = 300;
const detailPaneMaxWidth = 620;
const detailPaneDefaultWidth = 360;

function resolveTheme(theme: Theme) {
  if (theme === "system") {
    return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
  }

  return theme;
}

function notebookFormFromEntry(entry: NotebookEntry): NotebookForm {
  return {
    title: entry.title,
    body: entry.body,
    tags: entry.tags.join(", "),
    kind: entry.kind,
    claimStatus: entry.claimStatus ?? "",
    eventDate: entry.eventDate ?? "",
    followUpAfter: entry.followUpAfter ?? "",
    followUpDate: entry.followUpDate ?? "",
  };
}

function emptyNotebookForm(): NotebookForm {
  return {
    title: "",
    body: "",
    tags: "",
    kind: "manual",
    claimStatus: "",
    eventDate: "",
    followUpAfter: "",
    followUpDate: "",
  };
}

function emptyCompanyEventForm(): CompanyEventForm {
  return {
    companyId: "",
    eventType: "custom",
    title: "",
    eventDate: formatLocalDate(new Date()),
    eventTime: "",
    status: "scheduled",
  };
}

function emptyTranscriptJobForm(): TranscriptJobForm {
  return {
    url: "",
    label: "",
    companyQuery: "",
    companyId: "",
  };
}

function manualNotebookOrigins(): NotebookDraftOrigin[] {
  return [
    {
      sourceType: "manual",
      sourceId: null,
      sourceUrl: null,
      label: "Manual note",
    },
  ];
}

function formatTimestamp(value: string | null | undefined, emptyLabel = "Not set") {
  const trimmed = value?.trim();

  if (!trimmed) {
    return emptyLabel;
  }

  const isoTimestamp = trimmed.match(
    /^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}:\d{2})(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?$/,
  );

  if (isoTimestamp) {
    return `${isoTimestamp[1]} ${isoTimestamp[2]}`;
  }

  return trimmed.replace("T", " ").replace(/\.\d+$/, "");
}

function formatLocalDate(date: Date) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");

  return `${year}-${month}-${day}`;
}

function parseLocalDate(value: string) {
  const [year, month, day] = value.split("-").map(Number);

  if (!year || !month || !day) {
    return new Date();
  }

  return new Date(year, month - 1, day);
}

function addLocalDays(date: Date, days: number) {
  const nextDate = new Date(date);
  nextDate.setDate(nextDate.getDate() + days);

  return nextDate;
}

function daysBetweenLocalDates(fromDate: string, toDate: string) {
  const millisecondsPerDay = 24 * 60 * 60 * 1000;
  const from = parseLocalDate(fromDate).getTime();
  const to = parseLocalDate(toDate).getTime();

  return Math.round((to - from) / millisecondsPerDay);
}

function startOfIsoWeek(date: Date) {
  const startDate = new Date(date);
  const day = startDate.getDay();
  const mondayOffset = day === 0 ? -6 : 1 - day;
  startDate.setDate(startDate.getDate() + mondayOffset);

  return startDate;
}

function formatWeekRange(startDate: string, endDate: string) {
  return `${startDate} - ${endDate}`;
}

function companyEventDueLabel(eventDate: string) {
  const daysUntil = daysBetweenLocalDates(formatLocalDate(new Date()), eventDate);

  if (daysUntil < 0) {
    return "Past";
  }

  if (daysUntil === 0) {
    return "Today";
  }

  if (daysUntil === 1) {
    return "Tomorrow";
  }

  if (daysUntil <= 3) {
    return `In ${daysUntil} days`;
  }

  return null;
}

function companyEventDueClass(eventDate: string) {
  const daysUntil = daysBetweenLocalDates(formatLocalDate(new Date()), eventDate);

  if (daysUntil < 0) {
    return "event-due-past";
  }

  if (daysUntil === 0) {
    return "event-due-today";
  }

  if (daysUntil <= 3) {
    return "event-due-soon";
  }

  return "";
}

function formatEnumLabel(value: string) {
  return value
    .split("_")
    .filter(Boolean)
    .map((part) => `${part.charAt(0).toUpperCase()}${part.slice(1).toLowerCase()}`)
    .join(" ");
}

function formatCompanyEventType(value: string) {
  const labels: Record<string, string> = {
    periodic_report: "Periodic Report",
    corporate_action: "Corporate Action",
    dividend: "Dividend",
    shareholder_meeting: "Shareholder Meeting",
    conference_call: "Conference Call",
    investor_conference: "Investor Conference",
    market_making: "Market Making",
    listing_change: "Listing Change",
    other_market_event: "Market Event",
    custom: "Custom",
  };

  return labels[value] ?? formatEnumLabel(value);
}

function formatCompanyEventStatus(value: string) {
  return formatEnumLabel(value);
}

function formatCompanyEventSourceType(value: string) {
  const labels: Record<string, string> = {
    manual: "Manual",
    official_calendar: "Official Calendar",
    official_report: "Official Report",
    public_calendar: "Public Calendar",
    public_media: "Public Media",
    notebook_entry: "Notebook",
    feed_item: "Feed Item",
  };

  return labels[value] ?? formatEnumLabel(value);
}

function formatAiProvider(value: string | null | undefined) {
  const labels: Record<string, string> = {
    provider_gemini: "Gemini",
  };

  if (!value) {
    return "Not configured";
  }

  return labels[value] ?? value;
}

function formatGeminiModel(value: string | null | undefined) {
  const labels: Record<string, string> = {
    "gemini-2.5-flash-lite": "Gemini 2.5 Flash-Lite",
    "gemini-2.5-flash": "Gemini 2.5 Flash",
    "gemini-3.1-flash-lite": "Gemini 3.1 Flash-Lite",
    "gemini-3.5-flash": "Gemini 3.5 Flash",
  };

  if (!value) {
    return "Gemini 2.5 Flash";
  }

  return labels[value] ?? value;
}

function formatCredentialStorage(value: string | null | undefined) {
  const labels: Record<string, string> = {
    os_keychain: "OS keychain",
    development_environment: "Development environment",
    not_configured: "Not configured",
    os_keychain_unavailable: "OS keychain unavailable",
  };

  if (!value) {
    return "Not configured";
  }

  return labels[value] ?? formatEnumLabel(value);
}

function formatCredentialConfigured(status: CredentialStatus | null) {
  if (!status) {
    return "Unknown";
  }

  return status.configured ? "Configured" : "Not configured";
}

function formatCredentialKind(value: string | null | undefined) {
  const labels: Record<string, string> = {
    api_key: "API Key",
    username_password: "Username/password",
    session_token: "Session token",
    oauth_token: "OAuth token",
  };

  if (!value) {
    return "API Key";
  }

  return labels[value] ?? formatEnumLabel(value);
}

function formatTranscriptStatus(value: string) {
  return formatEnumLabel(value);
}

function isYouTubeUrl(value: string) {
  const trimmed = value.trim();

  if (!trimmed) {
    return false;
  }

  try {
    const url = new URL(trimmed);
    const hostname = url.hostname.toLowerCase();

    return hostname === "youtu.be" || hostname.endsWith(".youtube.com") || hostname === "youtube.com";
  } catch {
    return false;
  }
}

function transcriptUrlValidationMessage(value: string) {
  const trimmed = value.trim();

  if (!trimmed) {
    return "URL is required.";
  }

  if (!isYouTubeUrl(trimmed)) {
    return "Use a YouTube URL from youtube.com or youtu.be.";
  }

  return null;
}

function transcriptSegmentTimestamp(segment: TranscriptSegment) {
  if (segment.startSeconds === null && segment.endSeconds === null) {
    return "No timestamp";
  }

  if (segment.endSeconds === null) {
    return formatTranscriptSecond(segment.startSeconds ?? 0);
  }

  return `${formatTranscriptSecond(segment.startSeconds ?? 0)}-${formatTranscriptSecond(segment.endSeconds)}`;
}

function transcriptSegmentMatchesQuery(segment: TranscriptSegment, query: string) {
  const normalizedQuery = query.trim().toLowerCase();
  if (!normalizedQuery) {
    return true;
  }

  return [
    segment.text,
    segment.speaker ?? "",
    segment.language ?? "",
    transcriptSegmentTimestamp(segment),
  ]
    .join(" ")
    .toLowerCase()
    .includes(normalizedQuery);
}

function highlightSearchMatch(text: string, query: string): ReactNode {
  const normalizedQuery = query.trim();
  if (!normalizedQuery) {
    return text;
  }

  const lowerText = text.toLowerCase();
  const lowerQuery = normalizedQuery.toLowerCase();
  const parts: ReactNode[] = [];
  let cursor = 0;
  let matchIndex = lowerText.indexOf(lowerQuery);

  while (matchIndex !== -1) {
    if (matchIndex > cursor) {
      parts.push(text.slice(cursor, matchIndex));
    }
    const matchEnd = matchIndex + normalizedQuery.length;
    parts.push(
      <mark className="search-highlight" key={`${matchIndex}-${matchEnd}`}>
        {text.slice(matchIndex, matchEnd)}
      </mark>,
    );
    cursor = matchEnd;
    matchIndex = lowerText.indexOf(lowerQuery, cursor);
  }

  if (cursor < text.length) {
    parts.push(text.slice(cursor));
  }

  return parts;
}

function formatTranscriptSecond(value: number) {
  const safeValue = Math.max(0, Math.floor(value));
  const minutes = Math.floor(safeValue / 60);
  const seconds = safeValue % 60;

  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

function uniqueTranscriptJobs(jobs: TranscriptJob[]) {
  const seenJobIds = new Set<string>();

  return jobs.filter((job) => {
    if (seenJobIds.has(job.id)) {
      return false;
    }

    seenJobIds.add(job.id);
    return true;
  });
}

function schedulerStartJitterMs(intervalMs: number) {
  const jitterWindowMs = Math.min(Math.max(Math.floor(intervalMs * 0.1), 1000), 60000);

  if (jitterWindowMs < 2000) {
    return Math.floor(Math.random() * jitterWindowMs);
  }

  const minimumJitterMs = Math.min(15000, Math.floor(jitterWindowMs / 2));

  return minimumJitterMs + Math.floor(Math.random() * (jitterWindowMs - minimumJitterMs + 1));
}

function notebookTagFromFeedValue(value: string): string {
  return value.trim().toLowerCase().replace(/\s+/g, "-");
}

function renderMarkdownInline(text: string, keyPrefix: string): ReactNode[] {
  const tokens = text.split(/(`[^`]+`|\*\*[^*]+\*\*|\*[^*]+\*)/g);

  return tokens
    .filter((token) => token.length > 0)
    .map((token, index) => {
      const key = `${keyPrefix}-${index}`;

      if (token.startsWith("`") && token.endsWith("`")) {
        return <code key={key}>{token.slice(1, -1)}</code>;
      }

      if (token.startsWith("**") && token.endsWith("**")) {
        return <strong key={key}>{token.slice(2, -2)}</strong>;
      }

      if (token.startsWith("*") && token.endsWith("*")) {
        return <em key={key}>{token.slice(1, -1)}</em>;
      }

      return token;
    });
}

function renderMarkdownBlocks(markdown: string) {
  const lines = markdown.split(/\r?\n/);
  const blocks: ReactNode[] = [];
  let index = 0;

  while (index < lines.length) {
    const line = lines[index];
    const trimmed = line.trim();

    if (!trimmed) {
      index += 1;
      continue;
    }

    if (trimmed.startsWith("```")) {
      const codeLines: string[] = [];
      index += 1;

      while (index < lines.length && !lines[index].trim().startsWith("```")) {
        codeLines.push(lines[index]);
        index += 1;
      }

      blocks.push(
        <pre key={`code-${index}`}>
          <code>{codeLines.join("\n")}</code>
        </pre>,
      );
      index += 1;
      continue;
    }

    const heading = trimmed.match(/^(#{1,3})\s+(.+)$/);
    if (heading) {
      const level = heading[1].length;
      const content = renderMarkdownInline(heading[2], `heading-${index}`);

      if (level === 1) {
        blocks.push(<h2 key={`heading-${index}`}>{content}</h2>);
      } else if (level === 2) {
        blocks.push(<h3 key={`heading-${index}`}>{content}</h3>);
      } else {
        blocks.push(<h4 key={`heading-${index}`}>{content}</h4>);
      }

      index += 1;
      continue;
    }

    if (/^[-*]\s+/.test(trimmed)) {
      const items: ReactNode[] = [];

      while (index < lines.length && /^[-*]\s+/.test(lines[index].trim())) {
        const item = lines[index].trim().replace(/^[-*]\s+/, "");
        items.push(<li key={`li-${index}`}>{renderMarkdownInline(item, `li-${index}`)}</li>);
        index += 1;
      }

      blocks.push(<ul key={`ul-${index}`}>{items}</ul>);
      continue;
    }

    if (/^\d+\.\s+/.test(trimmed)) {
      const items: ReactNode[] = [];

      while (index < lines.length && /^\d+\.\s+/.test(lines[index].trim())) {
        const item = lines[index].trim().replace(/^\d+\.\s+/, "");
        items.push(<li key={`li-${index}`}>{renderMarkdownInline(item, `li-${index}`)}</li>);
        index += 1;
      }

      blocks.push(<ol key={`ol-${index}`}>{items}</ol>);
      continue;
    }

    if (trimmed.startsWith(">")) {
      const quoteLines: string[] = [];

      while (index < lines.length && lines[index].trim().startsWith(">")) {
        quoteLines.push(lines[index].trim().replace(/^>\s?/, ""));
        index += 1;
      }

      blocks.push(
        <blockquote key={`quote-${index}`}>
          {quoteLines.map((quoteLine, quoteIndex) => (
            <p key={`quote-${index}-${quoteIndex}`}>
              {renderMarkdownInline(quoteLine, `quote-${index}-${quoteIndex}`)}
            </p>
          ))}
        </blockquote>,
      );
      continue;
    }

    const paragraphLines = [trimmed];
    index += 1;

    while (
      index < lines.length &&
      lines[index].trim() &&
      !/^(#{1,3})\s+/.test(lines[index].trim()) &&
      !/^[-*]\s+/.test(lines[index].trim()) &&
      !/^\d+\.\s+/.test(lines[index].trim()) &&
      !lines[index].trim().startsWith(">") &&
      !lines[index].trim().startsWith("```")
    ) {
      paragraphLines.push(lines[index].trim());
      index += 1;
    }

    blocks.push(
      <p key={`p-${index}`}>
        {renderMarkdownInline(paragraphLines.join(" "), `p-${index}`)}
      </p>,
    );
  }

  return blocks.length > 0 ? blocks : [<p key="empty">No note body.</p>];
}

function MarkdownNoteBody({
  body,
  ariaLabel,
}: {
  body: string;
  ariaLabel?: string;
}) {
  return (
    <div className="notebook-read-body" aria-label={ariaLabel}>
      {renderMarkdownBlocks(body)}
    </div>
  );
}

function quarterFromDate(date: Date) {
  return `${date.getFullYear()}-Q${Math.floor(date.getMonth() / 3) + 1}`;
}

function shiftQuarter(quarter: string, offset: number) {
  const [yearPart, quarterPart] = quarter.split("-Q");
  const year = Number(yearPart);
  const quarterNumber = Number(quarterPart);
  const zeroBased = year * 4 + quarterNumber - 1 + offset;
  const nextYear = Math.floor(zeroBased / 4);
  const nextQuarter = (zeroBased % 4) + 1;

  return `${nextYear}-Q${nextQuarter}`;
}

function nearbyQuarters() {
  const currentQuarter = quarterFromDate(new Date());

  return Array.from({ length: 7 }, (_, index) => shiftQuarter(currentQuarter, index - 2));
}

function NotebookDateField({
  label,
  ariaLabel,
  value,
  onChange,
}: {
  label: string;
  ariaLabel: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="date-picker-field">
      <span>{label}</span>
      <input
        aria-label={ariaLabel}
        type="date"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}

function NotebookQuarterField({
  label,
  ariaLabel,
  value,
  onChange,
}: {
  label: string;
  ariaLabel: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const [isOpen, setOpen] = useState(false);
  const currentQuarter = quarterFromDate(new Date());
  const quarters = nearbyQuarters();

  return (
    <div className="date-picker-field">
      <span>{label}</span>
      <div className="date-picker-input-row">
        <input
          aria-label={ariaLabel}
          placeholder="2026-Q4"
          value={value}
          onChange={(event) => onChange(event.target.value)}
        />
        <button
          aria-label={`${label} picker`}
          className="icon-button date-picker-toggle"
          onClick={() => setOpen((current) => !current)}
          type="button"
        >
          <CalendarDays size={15} />
        </button>
      </div>
      {isOpen ? (
        <div className="date-picker-popover quarter-picker-popover">
          <button
            className="secondary-button compact-button"
            onClick={() => {
              onChange(currentQuarter);
              setOpen(false);
            }}
            type="button"
          >
            <CalendarDays size={15} />
            Today
          </button>
          <div className="quarter-picker-options">
            {quarters.map((quarter) => (
              <button
                className={quarter === value ? "quarter-option quarter-option-active" : "quarter-option"}
                key={quarter}
                onClick={() => {
                  onChange(quarter);
                  setOpen(false);
                }}
                type="button"
              >
                {quarter}
              </button>
            ))}
          </div>
          <button
            className="minimal-button"
            onClick={() => {
              onChange("");
              setOpen(false);
            }}
            type="button"
          >
            Clear
          </button>
        </div>
      ) : null}
    </div>
  );
}

export function App() {
  const contentGridRef = useRef<HTMLElement | null>(null);
  const sourceRefreshInFlightRef = useRef(false);
  const sourceAdaptersRef = useRef<SourceAdapter[]>([]);
  const eventWeekFetchAttemptedRef = useRef<Set<string>>(new Set());
  const companyLookupVersionRef = useRef(0);
  const skipNextCompanyLookupRef = useRef(false);
  const companyFieldRefs = useRef<Record<keyof CompanyForm, HTMLInputElement | null>>({
    exchange: null,
    ticker: null,
    displayName: null,
    isin: null,
  });
  const [activeSection, setActiveSection] = useState<Section>("Inbox");
  const [theme, setTheme] = useState<Theme>("dark");
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);
  const [databaseStatus, setDatabaseStatus] = useState<DatabaseStatus | null>(null);
  const [databaseError, setDatabaseError] = useState<string | null>(null);
  const [companies, setCompanies] = useState<Company[]>([]);
  const [companiesError, setCompaniesError] = useState<string | null>(null);
  const [watchlists, setWatchlists] = useState<Watchlist[]>([]);
  const [watchlistMemberships, setWatchlistMemberships] = useState<WatchlistMembership[]>([]);
  const [watchlistsError, setWatchlistsError] = useState<string | null>(null);
  const [watchlistName, setWatchlistName] = useState("");
  const [watchlistAssignments, setWatchlistAssignments] = useState<Record<string, string>>({});
  const [watchlistFeedback, setWatchlistFeedback] = useState<WatchlistFeedback | null>(null);
  const [notebookEntries, setNotebookEntries] = useState<NotebookEntry[]>([]);
  const [notebookError, setNotebookError] = useState<string | null>(null);
  const [inboxWatchlistFilter, setInboxWatchlistFilter] = useState("all");
  const [inboxCompanyFilter, setInboxCompanyFilter] = useState("all");
  const [inboxTypeFilter, setInboxTypeFilter] = useState("all");
  const [inboxSourceFilter, setInboxSourceFilter] = useState("all");
  const [inboxStatusFilter, setInboxStatusFilter] = useState<InboxStatusFilter>("all");
  const [feedState, setFeedState] = useState<FeedItem[]>([]);
  const [feedError, setFeedError] = useState<string | null>(null);
  const [companyEvents, setCompanyEvents] = useState<CompanyEvent[]>([]);
  const [companyEventsError, setCompanyEventsError] = useState<string | null>(null);
  const [transcriptJobs, setTranscriptJobs] = useState<TranscriptJob[]>([]);
  const [transcriptJobsError, setTranscriptJobsError] = useState<string | null>(null);
  const [transcriptJobForm, setTranscriptJobForm] = useState<TranscriptJobForm>(emptyTranscriptJobForm);
  const [transcriptJobCreateError, setTranscriptJobCreateError] = useState<string | null>(null);
  const [transcriptJobCreateState, setTranscriptJobCreateState] = useState<DbRefreshState>("idle");
  const [transcriptJobRunInFlight, setTranscriptJobRunInFlight] = useState<string | null>(null);
  const [selectedTranscriptJobId, setSelectedTranscriptJobId] = useState<string | null>(null);
  const [transcriptSegmentsByJobId, setTranscriptSegmentsByJobId] = useState<Record<string, TranscriptSegment[]>>({});
  const [transcriptSegmentsErrorByJobId, setTranscriptSegmentsErrorByJobId] = useState<Record<string, string | null>>({});
  const [transcriptSegmentSearchByJobId, setTranscriptSegmentSearchByJobId] = useState<Record<string, string>>({});
  const [selectedTranscriptSegmentIdsByJobId, setSelectedTranscriptSegmentIdsByJobId] = useState<Record<string, string[]>>({});
  const [transcriptNoteDraftJobId, setTranscriptNoteDraftJobId] = useState<string | null>(null);
  const [transcriptNoteForm, setTranscriptNoteForm] = useState<NotebookForm>(emptyNotebookForm);
  const [transcriptNoteErrorByJobId, setTranscriptNoteErrorByJobId] = useState<Record<string, string | null>>({});
  const [transcriptNoteSaveInFlight, setTranscriptNoteSaveInFlight] = useState<string | null>(null);
  const [transcriptLinkQueryByJobId, setTranscriptLinkQueryByJobId] = useState<Record<string, string>>({});
  const [transcriptLinkErrorByJobId, setTranscriptLinkErrorByJobId] = useState<Record<string, string | null>>({});
  const [transcriptLinkInFlight, setTranscriptLinkInFlight] = useState<string | null>(null);
  const [transcriptDeleteInFlight, setTranscriptDeleteInFlight] = useState<string | null>(null);
  const [transcriptDescriptionDraftByJobId, setTranscriptDescriptionDraftByJobId] = useState<Record<string, string>>({});
  const [transcriptDescriptionErrorByJobId, setTranscriptDescriptionErrorByJobId] = useState<Record<string, string | null>>({});
  const [transcriptDescriptionSaveInFlight, setTranscriptDescriptionSaveInFlight] = useState<string | null>(null);
  const [companyEventViewMode, setCompanyEventViewMode] = useState<CompanyEventViewMode>("week");
  const [companyEventMode, setCompanyEventMode] = useState<CompanyEventMode>("upcoming");
  const [companyEventWeekAnchorDate, setCompanyEventWeekAnchorDate] = useState(() =>
    formatLocalDate(new Date()),
  );
  const [companyEventWatchlistFilter, setCompanyEventWatchlistFilter] = useState("all");
  const [companyEventCompanyFilter, setCompanyEventCompanyFilter] = useState("all");
  const [companyEventTypeFilter, setCompanyEventTypeFilter] = useState("all");
  const [companyEventStatusFilter, setCompanyEventStatusFilter] = useState("all");
  const [companyEventDateFrom, setCompanyEventDateFrom] = useState("");
  const [companyEventDateTo, setCompanyEventDateTo] = useState("");
  const [selectedCompanyEventId, setSelectedCompanyEventId] = useState<string | null>(null);
  const [isCompanyEventComposerOpen, setCompanyEventComposerOpen] = useState(false);
  const [companyEventForm, setCompanyEventForm] = useState<CompanyEventForm>(emptyCompanyEventForm);
  const [companyEventCreateError, setCompanyEventCreateError] = useState<string | null>(null);
  const [sourceAdapters, setSourceAdapters] = useState<SourceAdapter[]>([]);
  const [sourceAdaptersError, setSourceAdaptersError] = useState<string | null>(null);
  const [selectedSourceAdapterId, setSelectedSourceAdapterId] = useState<string | null>(null);
  const [settings, setSettings] = useState<UserSettings | null>(null);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [geminiCredentialStatus, setGeminiCredentialStatus] = useState<CredentialStatus | null>(null);
  const [geminiCredentialError, setGeminiCredentialError] = useState<string | null>(null);
  const [geminiApiKeyDraft, setGeminiApiKeyDraft] = useState("");
  const [geminiCredentialInFlight, setGeminiCredentialInFlight] = useState(false);
  const [selectedFeedItemId, setSelectedFeedItemId] = useState<string | null>(null);
  const [selectedCompanyId, setSelectedCompanyId] = useState<string | null>(null);
  const [selectedCompanyFeedItemId, setSelectedCompanyFeedItemId] = useState<string | null>(null);
  const [selectedNotebookEntryId, setSelectedNotebookEntryId] = useState<string | null>(null);
  const [isNotebookComposerOpen, setNotebookComposerOpen] = useState(false);
  const [isNotebookEditMode, setNotebookEditMode] = useState(false);
  const [selectedClaimEntryId, setSelectedClaimEntryId] = useState<string | null>(null);
  const [claimStatusDraft, setClaimStatusDraft] = useState("");
  const [selectedNotebookCompanyId, setSelectedNotebookCompanyId] = useState<string | null>(null);
  const [selectedNotebookScreenEntryId, setSelectedNotebookScreenEntryId] = useState<string | null>(null);
  const [isNotebookScreenComposerOpen, setNotebookScreenComposerOpen] = useState(false);
  const [isNotebookScreenEditMode, setNotebookScreenEditMode] = useState(false);
  const [notebookScreenKindFilter, setNotebookScreenKindFilter] = useState("all");
  const [notebookScreenClaimStatusFilter, setNotebookScreenClaimStatusFilter] = useState("all");
  const [notebookScreenFollowUpFilter, setNotebookScreenFollowUpFilter] = useState("all");
  const [notebookScreenTagFilter, setNotebookScreenTagFilter] = useState("");
  const [notebookScreenForm, setNotebookScreenForm] = useState<NotebookForm>(emptyNotebookForm);
  const [notebookScreenDraftOrigins, setNotebookScreenDraftOrigins] =
    useState<NotebookDraftOrigin[]>(manualNotebookOrigins);
  const [notebookScreenEditForm, setNotebookScreenEditForm] = useState<NotebookForm>(emptyNotebookForm);
  const [companyWorkspaceTab, setCompanyWorkspaceTab] = useState<CompanyWorkspaceTab>("Feed");
  const [detailPaneWidth, setDetailPaneWidth] = useState(detailPaneDefaultWidth);
  const [dbRefreshState, setDbRefreshState] = useState<DbRefreshState>("idle");
  const [deleteUnsavedFeedState, setDeleteUnsavedFeedState] = useState<DbRefreshState>("idle");
  const [deleteUnsavedFeedError, setDeleteUnsavedFeedError] = useState<string | null>(null);
  const [feedPruneResult, setFeedPruneResult] = useState<FeedPruneResult | null>(null);
  const [sourceRefreshState, setSourceRefreshState] = useState<SourceRefreshState>("idle");
  const [sourceRefreshResult, setSourceRefreshResult] = useState<SourceIngestionResult | null>(null);
  const [sourceRefreshError, setSourceRefreshError] = useState<string | null>(null);
  const [sourceRefreshFailureCount, setSourceRefreshFailureCount] = useState(0);
  const [sourceAdapterRefreshInFlight, setSourceAdapterRefreshInFlight] = useState<string | null>(null);
  const [registryRefreshState, setRegistryRefreshState] = useState<SourceRefreshState>("idle");
  const [registryRefreshResult, setRegistryRefreshResult] = useState<CompanyRegistryRefreshResult | null>(null);
  const [registryRefreshError, setRegistryRefreshError] = useState<string | null>(null);
  const [nextSourceRefreshAtByAdapterId, setNextSourceRefreshAtByAdapterId] = useState<Record<string, number>>({});
  const [nextRegistryRefreshAt, setNextRegistryRefreshAt] = useState<number | null>(null);
  const [unmatchedSourceItems, setUnmatchedSourceItems] = useState<Record<string, UnmatchedSourceItem[]>>({});
  const [unmatchedSourceItemsError, setUnmatchedSourceItemsError] = useState<string | null>(null);
  const [expandedUnmatchedAdapters, setExpandedUnmatchedAdapters] = useState<Record<string, boolean>>({});
  const [companyRegistryEntries, setCompanyRegistryEntries] = useState<CompanyRegistryEntry[]>([]);
  const [companyRegistryEntriesError, setCompanyRegistryEntriesError] = useState<string | null>(null);
  const [isCompanyRegistryListExpanded, setCompanyRegistryListExpanded] = useState(false);
  const [companyRegistrySearch, setCompanyRegistrySearch] = useState("");
  const [selectedCompanyRegistryTicker, setSelectedCompanyRegistryTicker] = useState<string | null>(null);
  const [addingRegistryTicker, setAddingRegistryTicker] = useState<string | null>(null);
  const [companyListSearch, setCompanyListSearch] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  const [lookupStatus, setLookupStatus] = useState<string | null>(null);
  const [companyForm, setCompanyForm] = useState<CompanyForm>({
    exchange: "GPW",
    ticker: "",
    displayName: "",
    isin: "",
  });
  const [notebookForm, setNotebookForm] = useState<NotebookForm>(emptyNotebookForm);
  const [notebookEditForm, setNotebookEditForm] = useState<NotebookForm>(emptyNotebookForm);

  const effectiveTheme = useMemo(() => resolveTheme(theme), [theme]);
  const membershipsByCompany = useMemo(() => {
    return watchlistMemberships.reduce<Record<string, WatchlistMembership[]>>(
      (grouped, membership) => {
        grouped[membership.companyId] = grouped[membership.companyId] ?? [];
        grouped[membership.companyId].push(membership);
        return grouped;
      },
      {},
    );
  }, [watchlistMemberships]);
  const companiesById = useMemo(() => {
    return companies.reduce<Record<string, Company>>((indexed, company) => {
      indexed[company.id] = company;
      return indexed;
    }, {});
  }, [companies]);
  const transcriptCompanySuggestions = useMemo(() => {
    const query = transcriptJobForm.companyQuery.trim().toLowerCase();

    if (!query) {
      return companies.slice(0, 5);
    }

    return companies
      .filter((company) => {
        const values = [
          company.ticker,
          company.qualifiedTicker,
          company.displayName,
          company.isin ?? "",
        ].map((value) => value.toLowerCase());

        return values.some((value) => value.includes(query));
      })
      .slice(0, 5);
  }, [companies, transcriptJobForm.companyQuery]);
  const totalUnreadFeedItems = useMemo(
    () => feedState.filter((item) => item.unread).length,
    [feedState],
  );
  const feedTypes = useMemo(() => {
    return Array.from(new Set(feedState.map((item) => item.type))).sort();
  }, [feedState]);
  const feedSources = useMemo(() => {
    return Array.from(new Set(feedState.map((item) => item.source))).sort();
  }, [feedState]);
  const selectedCompanyEvent =
    companyEvents.find((event) => event.id === selectedCompanyEventId) ?? null;
  const companyEventTypes = useMemo(() => {
    return Array.from(new Set(companyEvents.map((event) => event.eventType))).sort();
  }, [companyEvents]);
  const companyEventStatuses = useMemo(() => {
    return Array.from(new Set(companyEvents.map((event) => event.status))).sort();
  }, [companyEvents]);
  const companyEventWeekRange = useMemo(() => {
    const weekStart = startOfIsoWeek(parseLocalDate(companyEventWeekAnchorDate));
    const weekEnd = addLocalDays(weekStart, 6);

    return {
      start: formatLocalDate(weekStart),
      end: formatLocalDate(weekEnd),
    };
  }, [companyEventWeekAnchorDate]);
  const companyEventWeekDays = useMemo(() => {
    const weekStart = parseLocalDate(companyEventWeekRange.start);

    return Array.from({ length: 7 }, (_, dayOffset) => {
      const date = addLocalDays(weekStart, dayOffset);

      return {
        date: formatLocalDate(date),
        label: date.toLocaleDateString(undefined, { weekday: "long" }),
      };
    });
  }, [companyEventWeekRange.start]);
  const companyEventsByDate = useMemo(() => {
    return companyEvents.reduce<Record<string, CompanyEvent[]>>((grouped, event) => {
      grouped[event.eventDate] = grouped[event.eventDate] ?? [];
      grouped[event.eventDate].push(event);

      return grouped;
    }, {});
  }, [companyEvents]);
  const companyEventWorkingWeekDays = useMemo(
    () => companyEventWeekDays.slice(0, 5),
    [companyEventWeekDays],
  );
  const companyEventWeekendDays = useMemo(
    () => companyEventWeekDays.slice(5),
    [companyEventWeekDays],
  );
  const companyEventWeekendEvents = useMemo(() => {
    return companyEventWeekendDays.flatMap((day) => companyEventsByDate[day.date] ?? []);
  }, [companyEventWeekendDays, companyEventsByDate]);
  const filteredFeedItems = useMemo(() => {
    const normalizedSearch = searchQuery.trim().toLowerCase();
    const allowedTickers =
      inboxWatchlistFilter === "all"
        ? null
        : new Set(
            watchlistMemberships
              .filter((membership) => membership.watchlistId === inboxWatchlistFilter)
              .map((membership) => companiesById[membership.companyId]?.qualifiedTicker)
              .filter((qualifiedTicker): qualifiedTicker is string => Boolean(qualifiedTicker)),
          );

    return feedState.filter((item) => {
      const watchlistMatches = allowedTickers ? allowedTickers.has(item.company) : true;
      const companyMatches = inboxCompanyFilter === "all" || item.company === inboxCompanyFilter;
      const typeMatches = inboxTypeFilter === "all" || item.type === inboxTypeFilter;
      const sourceMatches = inboxSourceFilter === "all" || item.source === inboxSourceFilter;
      const statusMatches =
        inboxStatusFilter === "all" ||
        (inboxStatusFilter === "unread" && item.unread) ||
        (inboxStatusFilter === "saved" && item.saved);
      const searchMatches =
        normalizedSearch.length === 0 ||
        [item.company, item.title, item.source, item.type, item.summary]
          .join(" ")
          .toLowerCase()
          .includes(normalizedSearch);

      return (
        watchlistMatches &&
        companyMatches &&
        typeMatches &&
        sourceMatches &&
        statusMatches &&
        searchMatches
      );
    });
  }, [
    companiesById,
    feedState,
    inboxCompanyFilter,
    inboxSourceFilter,
    inboxStatusFilter,
    inboxTypeFilter,
    inboxWatchlistFilter,
    searchQuery,
    watchlistMemberships,
  ]);
  const selectedFeedItem =
    filteredFeedItems.find((item) => item.id === selectedFeedItemId) ?? filteredFeedItems[0] ?? null;
  const inboxReviewStats = useMemo(
    () => ({
      visible: filteredFeedItems.length,
      unread: filteredFeedItems.filter((item) => item.unread).length,
      saved: filteredFeedItems.filter((item) => item.saved).length,
    }),
    [filteredFeedItems],
  );
  const selectedFeedCompany =
    selectedFeedItem
      ? companies.find((company) => company.qualifiedTicker === selectedFeedItem.company) ?? null
      : null;
  const selectedCompany =
    companies.find((company) => company.id === selectedCompanyId) ?? null;
  const selectedCompanyNotebookEntries = useMemo(() => {
    if (!selectedCompany) {
      return [];
    }

    return notebookEntries.filter((entry) => entry.companyId === selectedCompany.id);
  }, [notebookEntries, selectedCompany]);
  const selectedNotebookEntry =
    selectedCompanyNotebookEntries.find((entry) => entry.id === selectedNotebookEntryId) ??
    selectedCompanyNotebookEntries[0] ??
    null;
  const selectedCompanyClaimEntries = useMemo(
    () =>
      selectedCompanyNotebookEntries.filter(
        (entry) => entry.kind === "claim" || Boolean(entry.claimStatus),
      ),
    [selectedCompanyNotebookEntries],
  );
  const selectedClaimEntry =
    selectedCompanyClaimEntries.find((entry) => entry.id === selectedClaimEntryId) ?? null;
  const isNotebookEditDirty = selectedNotebookEntry
    ? JSON.stringify(notebookEditForm) !== JSON.stringify(notebookFormFromEntry(selectedNotebookEntry))
    : false;
  const selectedNotebookScreenCompany =
    companies.find((company) => company.id === selectedNotebookCompanyId) ?? companies[0] ?? null;
  const selectedNotebookScreenEntries = useMemo(() => {
    if (!selectedNotebookScreenCompany) {
      return [];
    }

    const normalizedSearch = searchQuery.trim().toLowerCase();
    const normalizedTagFilter = notebookScreenTagFilter.trim().toLowerCase();
    const entries = notebookEntries.filter(
      (entry) => entry.companyId === selectedNotebookScreenCompany.id,
    );

    return entries.filter((entry) => {
      const kindMatches =
        notebookScreenKindFilter === "all" || entry.kind === notebookScreenKindFilter;
      const statusMatches =
        notebookScreenClaimStatusFilter === "all" ||
        entry.claimStatus === notebookScreenClaimStatusFilter;
      const hasFollowUp = Boolean(entry.followUpAfter || entry.followUpDate);
      const followUpMatches =
        notebookScreenFollowUpFilter === "all" ||
        (notebookScreenFollowUpFilter === "has_follow_up" && hasFollowUp) ||
        (notebookScreenFollowUpFilter === "no_follow_up" && !hasFollowUp);
      const tagMatches =
        normalizedTagFilter.length === 0 ||
        entry.tags.some((tag) => tag.toLowerCase().includes(normalizedTagFilter));
      const searchMatches =
        normalizedSearch.length === 0 ||
        [
        entry.title,
        entry.body,
        entry.kind,
        entry.claimStatus ?? "",
        entry.followUpAfter ?? "",
        entry.followUpDate ?? "",
        entry.tags.join(" "),
      ]
        .join(" ")
        .toLowerCase()
          .includes(normalizedSearch);

      return kindMatches && statusMatches && followUpMatches && tagMatches && searchMatches;
    });
  }, [
    notebookEntries,
    notebookScreenClaimStatusFilter,
    notebookScreenKindFilter,
    notebookScreenFollowUpFilter,
    notebookScreenTagFilter,
    searchQuery,
    selectedNotebookScreenCompany,
  ]);
  const selectedNotebookScreenEntry =
    selectedNotebookScreenEntries.find((entry) => entry.id === selectedNotebookScreenEntryId) ?? null;
  const isNotebookScreenEditDirty = selectedNotebookScreenEntry
    ? JSON.stringify(notebookScreenEditForm) !==
      JSON.stringify(notebookFormFromEntry(selectedNotebookScreenEntry))
    : false;
  const selectedCompanyFeedItems = useMemo(() => {
    if (!selectedCompany) {
      return [];
    }

    return feedState.filter((item) => item.company === selectedCompany.qualifiedTicker);
  }, [feedState, selectedCompany]);
  const selectedCompanyFeedItem =
    selectedCompanyFeedItems.find((item) => item.id === selectedCompanyFeedItemId) ?? null;
  const registryAdapter = useMemo(
    () => sourceAdapters.find((adapter) => adapter.id === gpwRegistryAdapterId) ?? null,
    [sourceAdapters],
  );
  const scheduledSourceAdapters = useMemo(
    () => sourceAdapters.filter((adapter) => adapter.enabled && adapter.id !== gpwRegistryAdapterId),
    [sourceAdapters],
  );
  const scheduledSourceAdapterKey = useMemo(
    () => scheduledSourceAdapters.map((adapter) => adapter.id).join("|"),
    [scheduledSourceAdapters],
  );
  const filteredCompanyRegistryEntries = useMemo(() => {
    const normalizedSearch = companyRegistrySearch.trim().toLowerCase();

    if (normalizedSearch.length === 0) {
      return companyRegistryEntries;
    }

    return companyRegistryEntries.filter((entry) =>
      [
        entry.exchange,
        entry.ticker,
        entry.qualifiedTicker,
        entry.displayName,
        entry.isin ?? "",
      ]
        .join(" ")
        .toLowerCase()
      .includes(normalizedSearch),
    );
  }, [companyRegistryEntries, companyRegistrySearch]);
  const companyFormRegistryMatches = useMemo(() => {
    if (selectedCompanyRegistryTicker) {
      return [];
    }

    const searchTerms = [
      companyForm.ticker,
      companyForm.displayName,
      companyForm.isin,
    ]
      .map((value) => value.trim().toLowerCase())
      .filter((value) => value.length >= 2);

    if (companyForm.exchange.trim().toUpperCase() !== "GPW" || searchTerms.length === 0) {
      return [];
    }

    return companyRegistryEntries
      .filter((entry) =>
        searchTerms.some((term) =>
          [
            entry.ticker,
            entry.qualifiedTicker,
            entry.displayName,
            entry.isin ?? "",
          ]
            .join(" ")
            .toLowerCase()
            .includes(term),
        ),
      )
      .slice(0, 5);
  }, [
    companyForm.displayName,
    companyForm.exchange,
    companyForm.isin,
    companyForm.ticker,
    companyRegistryEntries,
    selectedCompanyRegistryTicker,
  ]);
  const filteredCompanies = useMemo(() => {
    const normalizedSearch = companyListSearch.trim().toLowerCase();

    if (normalizedSearch.length === 0) {
      return companies;
    }

    return companies.filter((company) =>
      [
        company.exchange,
        company.ticker,
        company.qualifiedTicker,
        company.displayName,
        company.isin ?? "",
      ]
        .join(" ")
        .toLowerCase()
        .includes(normalizedSearch),
    );
  }, [companies, companyListSearch]);
  const selectedCompanyFeedStats = useMemo(
    () => ({
      total: selectedCompanyFeedItems.length,
      unread: selectedCompanyFeedItems.filter((item) => item.unread).length,
      saved: selectedCompanyFeedItems.filter((item) => item.saved).length,
    }),
    [selectedCompanyFeedItems],
  );
  const sourceStatusSummary = useMemo(() => {
    if (sourceAdaptersError) {
      return {
        label: "error",
        title: `Source adapter command failed: ${sourceAdaptersError}`,
        tone: "danger",
      };
    }

    if (sourceAdapters.length === 0) {
      return {
        label: "0 sources",
        title: "No source adapters configured",
        tone: "warn",
      };
    }

    const enabledAdapters = sourceAdapters.filter((adapter) => adapter.enabled);
    const adaptersWithErrors = sourceAdapters.filter((adapter) => adapter.lastError);

    if (adaptersWithErrors.length > 0) {
      return {
        label: `${adaptersWithErrors.length} issue${adaptersWithErrors.length === 1 ? "" : "s"}`,
        title: `${adaptersWithErrors.length} source adapter issue${adaptersWithErrors.length === 1 ? "" : "s"}`,
        tone: "danger",
      };
    }

    return {
      label: `${enabledAdapters.length}/${sourceAdapters.length}`,
      title: `${enabledAdapters.length} enabled source adapter${enabledAdapters.length === 1 ? "" : "s"} ready`,
      tone: "ok",
    };
  }, [sourceAdapters, sourceAdaptersError]);
  const hasActiveInboxFilters =
    searchQuery.trim().length > 0 ||
    inboxWatchlistFilter !== "all" ||
    inboxCompanyFilter !== "all" ||
    inboxTypeFilter !== "all" ||
    inboxSourceFilter !== "all" ||
    inboxStatusFilter !== "all";
  const inboxEmptyState =
    companies.length === 0
      ? "no-companies"
      : feedState.length === 0
        ? "no-feed"
        : filteredFeedItems.length === 0
          ? "no-matches"
          : null;

  useEffect(() => {
    document.documentElement.dataset.theme = effectiveTheme;
  }, [effectiveTheme]);

  useEffect(() => {
    if (selectedFeedItemId && filteredFeedItems.some((item) => item.id === selectedFeedItemId)) {
      return;
    }

    const nextSelectedFeedItemId = filteredFeedItems[0]?.id ?? null;

    if (selectedFeedItemId !== nextSelectedFeedItemId) {
      setSelectedFeedItemId(nextSelectedFeedItemId);
    }
  }, [filteredFeedItems, selectedFeedItemId]);

  useEffect(() => {
    invoke<HealthResponse>("health")
      .then((response) => {
        setHealth(response);
        setHealthError(null);
      })
      .catch((error) => {
        setHealth(null);
        setHealthError(String(error));
      });
  }, []);

  useEffect(() => {
    invoke<DatabaseStatus>("database_status")
      .then((response) => {
        setDatabaseStatus(response);
        setDatabaseError(null);
      })
      .catch((error) => {
        setDatabaseStatus(null);
        setDatabaseError(String(error));
      });
  }, []);

  function refreshDatabaseStatus() {
    return invoke<DatabaseStatus>("database_status")
      .then((response) => {
        setDatabaseStatus(response);
        setDatabaseError(null);
      })
      .catch((error) => {
        setDatabaseStatus(null);
        setDatabaseError(String(error));
      });
  }

  function refreshCompanies() {
    return invoke<Company[]>("list_companies")
      .then((response) => {
        setCompanies(response);
        setCompaniesError(null);
      })
      .catch((error) => {
        setCompanies([]);
        setCompaniesError(String(error));
      });
  }

  function refreshWatchlists() {
    return invoke<Watchlist[]>("list_watchlists")
      .then((response) => {
        setWatchlists(response);
        setWatchlistsError(null);
        setWatchlistAssignments((current) => {
          const fallback = response[0]?.id ?? "";
          const next = { ...current };

          for (const company of companies) {
            if (!next[company.id]) {
              next[company.id] = fallback;
            }
          }

          return next;
        });
      })
      .catch((error) => {
        setWatchlists([]);
        setWatchlistsError(String(error));
      });
  }

  function refreshWatchlistMemberships() {
    return invoke<WatchlistMembership[]>("list_watchlist_memberships")
      .then((response) => {
        setWatchlistMemberships(response);
        setWatchlistsError(null);
      })
      .catch((error) => {
        setWatchlistMemberships([]);
        setWatchlistsError(String(error));
      });
  }

  function refreshFeedItems() {
    return invoke<FeedItem[]>("list_feed_items")
      .then((response) => {
        setFeedState(response);
        setFeedError(null);
        setSelectedFeedItemId((current) => {
          if (current && response.some((item) => item.id === current)) {
            return current;
          }

          return response[0]?.id ?? null;
        });
      })
      .catch((error) => {
        setFeedState([]);
        setFeedError(String(error));
      });
  }

  function refreshCompanyEvents(mode: CompanyEventMode = companyEventMode) {
    const isWeekView = companyEventViewMode === "week";

    return invoke<CompanyEvent[]>("list_company_events", {
      input: {
        mode: isWeekView ? "all" : mode,
        companyId: companyEventCompanyFilter === "all" ? null : companyEventCompanyFilter,
        watchlistId: companyEventWatchlistFilter === "all" ? null : companyEventWatchlistFilter,
        eventType: companyEventTypeFilter === "all" ? null : companyEventTypeFilter,
        status: companyEventStatusFilter === "all" ? null : companyEventStatusFilter,
        dateFrom: isWeekView ? companyEventWeekRange.start : companyEventDateFrom.trim() || null,
        dateTo: isWeekView ? companyEventWeekRange.end : companyEventDateTo.trim() || null,
      },
    })
      .then((response) => {
        setCompanyEvents(response);
        setCompanyEventsError(null);
        setSelectedCompanyEventId((current) => {
          if (current && response.some((event) => event.id === current)) {
            return current;
          }

          return null;
        });
      })
      .catch((error) => {
        setCompanyEvents([]);
        setCompanyEventsError(String(error));
      });
  }

  function clearCompanyEventFilters() {
    setCompanyEventWatchlistFilter("all");
    setCompanyEventCompanyFilter("all");
    setCompanyEventTypeFilter("all");
    setCompanyEventStatusFilter("all");
    setCompanyEventDateFrom("");
    setCompanyEventDateTo("");
    setSelectedCompanyEventId(null);
  }

  function openCompanyEventComposer() {
    setCompanyEventForm({
      ...emptyCompanyEventForm(),
      companyId:
        companyEventCompanyFilter !== "all"
          ? companyEventCompanyFilter
          : companies[0]?.id ?? "",
      eventDate:
        companyEventViewMode === "week" ? companyEventWeekAnchorDate : formatLocalDate(new Date()),
    });
    setCompanyEventCreateError(null);
    setCompanyEventComposerOpen(true);
  }

  function createCompanyEvent() {
    const trimmedTitle = companyEventForm.title.trim();

    if (!companyEventForm.companyId || !trimmedTitle || !companyEventForm.eventDate) {
      setCompanyEventCreateError("Company, title, and date are required.");
      return;
    }

    void invoke<CompanyEvent>("create_company_event", {
      input: {
        companyId: companyEventForm.companyId,
        eventType: companyEventForm.eventType,
        title: trimmedTitle,
        eventDate: companyEventForm.eventDate,
        eventTime: companyEventForm.eventTime.trim() || null,
        status: companyEventForm.status,
        sourceType: "manual",
        sourceAdapterId: null,
        sourceEventKey: null,
        sourceUrl: null,
        attribution: "Manual",
        fetchedAt: null,
      },
    })
      .then((createdEvent) => {
        setCompanyEventCreateError(null);
        setCompanyEventComposerOpen(false);
        setCompanyEventForm(emptyCompanyEventForm());
        setSelectedCompanyEventId(createdEvent.id);
        return refreshCompanyEvents();
      })
      .catch((error) => {
        setCompanyEventCreateError(String(error));
      });
  }

  function refreshTranscriptJobs(companyId: string | null = null) {
    return invoke<TranscriptJob[]>("list_video_transcript_jobs", {
      input: {
        companyId,
      },
    })
      .then((response) => {
        setTranscriptJobs(uniqueTranscriptJobs(response));
        setTranscriptJobsError(null);
        const jobIds = new Set(response.map((job) => job.id));
        setTranscriptSegmentsByJobId((current) =>
          Object.fromEntries(Object.entries(current).filter(([jobId]) => jobIds.has(jobId))),
        );
        setSelectedTranscriptSegmentIdsByJobId((current) =>
          Object.fromEntries(Object.entries(current).filter(([jobId]) => jobIds.has(jobId))),
        );
        setTranscriptSegmentsErrorByJobId((current) =>
          Object.fromEntries(Object.entries(current).filter(([jobId]) => jobIds.has(jobId))),
        );
        setSelectedTranscriptJobId((current) => (current && jobIds.has(current) ? current : null));
      })
      .catch((error) => {
        setTranscriptJobs([]);
        setTranscriptJobsError(String(error));
      });
  }

  function refreshTranscriptSegments(jobId: string) {
    return invoke<TranscriptSegment[]>("list_transcript_segments", {
      transcriptJobId: jobId,
    })
      .then((response) => {
        setTranscriptSegmentsByJobId((current) => ({
          ...current,
          [jobId]: response,
        }));
        setTranscriptSegmentsErrorByJobId((current) => ({
          ...current,
          [jobId]: null,
        }));
        setSelectedTranscriptSegmentIdsByJobId((current) => {
          const availableIds = new Set(response.map((segment) => segment.id));
          const selectedIds = current[jobId]?.filter((segmentId) => availableIds.has(segmentId)) ?? [];

          return {
            ...current,
            [jobId]: selectedIds,
          };
        });
      })
      .catch((error) => {
        setTranscriptSegmentsByJobId((current) => ({
          ...current,
          [jobId]: [],
        }));
        setTranscriptSegmentsErrorByJobId((current) => ({
          ...current,
          [jobId]: String(error),
        }));
      });
  }

  function toggleTranscriptJob(job: TranscriptJob) {
    setSelectedTranscriptJobId((current) => (current === job.id ? null : job.id));

    if (job.status === "completed") {
      void refreshTranscriptSegments(job.id);
    }
  }

  function toggleTranscriptJobFromKeyboard(
    event: KeyboardEvent<HTMLElement>,
    job: TranscriptJob,
  ) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      toggleTranscriptJob(job);
    }
  }

  function toggleTranscriptSegment(jobId: string, segmentId: string) {
    setSelectedTranscriptSegmentIdsByJobId((current) => {
      const selectedIds = current[jobId] ?? [];
      const nextSelectedIds = selectedIds.includes(segmentId)
        ? selectedIds.filter((selectedId) => selectedId !== segmentId)
        : [...selectedIds, segmentId];

      return {
        ...current,
        [jobId]: nextSelectedIds,
      };
    });
  }

  function openTranscriptNoteDraft(job: TranscriptJob, selectedSegments: TranscriptSegment[]) {
    if (!job.companyId) {
      setTranscriptNoteErrorByJobId((current) => ({
        ...current,
        [job.id]: "Choose a company to save this as a company notebook note. The transcript can remain unlinked.",
      }));
      return;
    }

    if (selectedSegments.length === 0) {
      setTranscriptNoteErrorByJobId((current) => ({
        ...current,
        [job.id]: "Select at least one transcript segment.",
      }));
      return;
    }

    setTranscriptNoteDraftJobId(job.id);
    setTranscriptNoteForm({
      title: job.sourceLabel ?? "Transcript note",
      body: selectedSegments.map((segment) => `> ${segment.text}`).join("\n\n"),
      tags: "transcript",
      kind: "observation",
      claimStatus: "",
      eventDate: "",
      followUpAfter: "",
      followUpDate: "",
    });
    setTranscriptNoteErrorByJobId((current) => ({
      ...current,
      [job.id]: null,
    }));
  }

  function discardTranscriptNoteDraft() {
    setTranscriptNoteDraftJobId(null);
    setTranscriptNoteForm(emptyNotebookForm());
  }

  function updateTranscriptLinkQuery(jobId: string, value: string) {
    setTranscriptLinkQueryByJobId((current) => ({
      ...current,
      [jobId]: value,
    }));
  }

  function linkTranscriptJobCompany(jobId: string, company: Company) {
    setTranscriptLinkInFlight(jobId);
    void invoke<TranscriptJob>("resolve_transcript_job_company", {
      input: {
        jobId,
        companyId: company.id,
      },
    })
      .then((updated) => {
        setTranscriptJobs((current) => current.map((job) => (job.id === updated.id ? updated : job)));
        setTranscriptLinkQueryByJobId((current) => ({
          ...current,
          [jobId]: "",
        }));
        setTranscriptLinkErrorByJobId((current) => ({
          ...current,
          [jobId]: null,
        }));
        setTranscriptNoteErrorByJobId((current) => ({
          ...current,
          [jobId]: null,
        }));
        void refreshTranscriptSegments(jobId);
      })
      .catch((error) => {
        setTranscriptLinkErrorByJobId((current) => ({
          ...current,
          [jobId]: String(error),
        }));
      })
      .finally(() => {
        setTranscriptLinkInFlight(null);
      });
  }

  function deleteTranscriptJob(job: TranscriptJob) {
    const label = job.sourceLabel ?? job.sourceUrl;
    const confirmed = window.confirm(`Delete transcript job "${label}"? Stored transcript segments for this job will also be removed.`);

    if (!confirmed) {
      return;
    }

    setTranscriptDeleteInFlight(job.id);
    void invoke<void>("delete_video_transcript_job", {
      jobId: job.id,
    })
      .then(() => {
        setTranscriptJobs((current) => current.filter((entry) => entry.id !== job.id));
        setTranscriptSegmentsByJobId((current) => {
          const next = { ...current };
          delete next[job.id];
          return next;
        });
        setSelectedTranscriptSegmentIdsByJobId((current) => {
          const next = { ...current };
          delete next[job.id];
          return next;
        });
        setTranscriptSegmentsErrorByJobId((current) => {
          const next = { ...current };
          delete next[job.id];
          return next;
        });
        setTranscriptNoteErrorByJobId((current) => {
          const next = { ...current };
          delete next[job.id];
          return next;
        });
        setTranscriptLinkErrorByJobId((current) => {
          const next = { ...current };
          delete next[job.id];
          return next;
        });
        setTranscriptLinkQueryByJobId((current) => {
          const next = { ...current };
          delete next[job.id];
          return next;
        });
        setTranscriptDescriptionDraftByJobId((current) => {
          const next = { ...current };
          delete next[job.id];
          return next;
        });
        setTranscriptDescriptionErrorByJobId((current) => {
          const next = { ...current };
          delete next[job.id];
          return next;
        });
        setSelectedTranscriptJobId((current) => (current === job.id ? null : current));
        setTranscriptNoteDraftJobId((current) => (current === job.id ? null : current));
        setTranscriptJobsError(null);
      })
      .catch((error) => {
        setTranscriptJobsError(String(error));
        void refreshTranscriptJobs();
      })
      .finally(() => {
        setTranscriptDeleteInFlight(null);
      });
  }

  function updateTranscriptJobDescription(job: TranscriptJob) {
    const draft = transcriptDescriptionDraftByJobId[job.id] ?? job.sourceLabel ?? "";

    setTranscriptDescriptionSaveInFlight(job.id);
    setTranscriptDescriptionErrorByJobId((current) => ({
      ...current,
      [job.id]: null,
    }));

    void invoke<TranscriptJob>("update_video_transcript_job", {
      input: {
        jobId: job.id,
        sourceLabel: draft.trim() || null,
      },
    })
      .then((updated) => {
        setTranscriptJobs((current) => current.map((entry) => (entry.id === updated.id ? updated : entry)));
        setTranscriptDescriptionDraftByJobId((current) => ({
          ...current,
          [updated.id]: updated.sourceLabel ?? "",
        }));
      })
      .catch((error) => {
        setTranscriptDescriptionErrorByJobId((current) => ({
          ...current,
          [job.id]: String(error),
        }));
      })
      .finally(() => {
        setTranscriptDescriptionSaveInFlight(null);
      });
  }

  function createTranscriptNotebookEntry(job: TranscriptJob, event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const selectedSegmentIds = selectedTranscriptSegmentIdsByJobId[job.id] ?? [];

    if (!job.companyId) {
      setTranscriptNoteErrorByJobId((current) => ({
        ...current,
        [job.id]: "Choose a company to save this as a company notebook note. The transcript can remain unlinked.",
      }));
      return;
    }

    if (selectedSegmentIds.length === 0) {
      setTranscriptNoteErrorByJobId((current) => ({
        ...current,
        [job.id]: "Select at least one transcript segment.",
      }));
      return;
    }

    const companyId = job.companyId;
    setTranscriptNoteSaveInFlight(job.id);
    void invoke<NotebookEntry>("create_note_from_transcript_selection", {
      input: {
        transcriptJobId: job.id,
        transcriptSegmentIds: selectedSegmentIds,
        noteDraft: {
          title: transcriptNoteForm.title,
          body: transcriptNoteForm.body,
          tags: transcriptNoteForm.tags
            .split(",")
            .map((tag) => tag.trim())
            .filter(Boolean),
          kind: transcriptNoteForm.kind,
          claimStatus: transcriptNoteForm.claimStatus || null,
          eventDate: transcriptNoteForm.eventDate || null,
          followUpAfter: transcriptNoteForm.followUpAfter || null,
          followUpDate: transcriptNoteForm.followUpDate || null,
        },
      },
    })
      .then((created) => {
        setNotebookEntries((current) => [
          created,
          ...current.filter((entry) => entry.id !== created.id),
        ]);
        setSelectedNotebookCompanyId(companyId);
        setSelectedNotebookScreenEntryId(created.id);
        setTranscriptNoteDraftJobId(null);
        setTranscriptNoteForm(emptyNotebookForm());
        setTranscriptNoteErrorByJobId((current) => ({
          ...current,
          [job.id]: null,
        }));
        setSelectedTranscriptSegmentIdsByJobId((current) => ({
          ...current,
          [job.id]: [],
        }));
        void refreshNotebookEntries(companyId);
      })
      .catch((error) => {
        setTranscriptNoteErrorByJobId((current) => ({
          ...current,
          [job.id]: String(error),
        }));
      })
      .finally(() => {
        setTranscriptNoteSaveInFlight(null);
      });
  }

  function selectTranscriptCompany(company: Company) {
    setTranscriptJobForm((current) => ({
      ...current,
      companyId: company.id,
      companyQuery: company.qualifiedTicker,
    }));
  }

  function createTranscriptJob(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const url = transcriptJobForm.url.trim();
    const validationMessage = transcriptUrlValidationMessage(url);

    if (validationMessage) {
      setTranscriptJobCreateError(validationMessage);
      return;
    }

    setTranscriptJobCreateError(null);
    setTranscriptJobCreateState("refreshing");

    void invoke<TranscriptJob>("create_video_transcript_job", {
      input: {
        sourceUrl: url,
        companyId: transcriptJobForm.companyId || null,
        providerId: settings?.aiProviders.youtubeTranscriptionProvider ?? "provider_gemini",
        sourceLabel: transcriptJobForm.label.trim() || null,
        recognizedCompanyCandidates: null,
      },
    })
      .then((created) => {
        setTranscriptJobForm(emptyTranscriptJobForm());
        setTranscriptJobs((current) => [
          created,
          ...current.filter((job) => job.id !== created.id),
        ]);
        setTranscriptJobCreateState("done");
        if (geminiCredentialStatus?.configured) {
          runTranscriptJob(created.id);
          return Promise.resolve();
        } else {
          setTranscriptJobsError("Gemini transcription credentials are not configured in this app runtime. Save the Gemini API key in Settings before running a transcript job.");
          return refreshTranscriptJobs();
        }
      })
      .catch((error) => {
        setTranscriptJobCreateError(String(error));
        setTranscriptJobCreateState("idle");
      });
  }

  function runTranscriptJob(jobId: string) {
    if (!geminiCredentialStatus?.configured) {
      setTranscriptJobsError("Gemini transcription credentials are not configured in this app runtime. Save the Gemini API key in Settings before running a transcript job.");
      return;
    }

    setTranscriptJobRunInFlight(jobId);
    setTranscriptJobs((current) =>
      current.map((job) =>
        job.id === jobId
          ? {
              ...job,
              status: "running",
              error: null,
              errorCode: null,
            }
          : job,
      ),
    );

    void invoke<TranscriptJob>("run_video_transcript_job", {
      input: {
        jobId,
        providerMode: settings?.aiProviders.youtubeTranscriptionProvider ?? "provider_gemini",
      },
    })
      .then((updated) => {
        setTranscriptJobs((current) => current.map((job) => (job.id === updated.id ? updated : job)));
        setTranscriptJobsError(null);
        if (selectedTranscriptJobId === updated.id && updated.status === "completed") {
          void refreshTranscriptSegments(updated.id);
        }
        return refreshTranscriptJobs();
      })
      .catch((error) => {
        setTranscriptJobsError(String(error));
      })
      .finally(() => {
        setTranscriptJobRunInFlight(null);
      });
  }

  function refreshSourceAdapters() {
    return invoke<SourceAdapter[]>("list_source_adapters")
      .then((response) => {
        setSourceAdapters(response);
        setSourceAdaptersError(null);
      })
      .catch((error) => {
        setSourceAdapters([]);
        setSourceAdaptersError(String(error));
      });
  }

  function refreshUnmatchedSourceItems(adapterId: string) {
    return invoke<UnmatchedSourceItem[]>("list_unmatched_source_items", { adapterId })
      .then((response) => {
        setUnmatchedSourceItems((current) => ({
          ...current,
          [adapterId]: response,
        }));
        setUnmatchedSourceItemsError(null);
      })
      .catch((error) => {
        setUnmatchedSourceItems((current) => ({
          ...current,
          [adapterId]: [],
        }));
        setUnmatchedSourceItemsError(String(error));
      });
  }

  function refreshCompanyRegistryEntries() {
    return invoke<CompanyRegistryEntry[]>("list_company_registry_entries")
      .then((response) => {
        setCompanyRegistryEntries(response);
        setCompanyRegistryEntriesError(null);
      })
      .catch((error) => {
        setCompanyRegistryEntries([]);
        setCompanyRegistryEntriesError(String(error));
      });
  }

  function refreshSettings() {
    return invoke<UserSettings>("get_settings")
      .then((response) => {
        setSettings(response);
        setTheme(response.theme);
        setSettingsError(null);
      })
      .catch((error) => {
        setSettings(null);
        setSettingsError(String(error));
      });
  }

  function refreshGeminiCredentialStatus() {
    return invoke<CredentialStatus>("get_gemini_transcription_credential_status")
      .then((response) => {
        setGeminiCredentialStatus(response);
        setGeminiCredentialError(null);
      })
      .catch((error) => {
        setGeminiCredentialStatus(null);
        setGeminiCredentialError(String(error));
      });
  }

  function refreshDatabaseBackedViews() {
    setDbRefreshState("refreshing");

    Promise.all([
      refreshDatabaseStatus(),
      refreshCompanies(),
      refreshWatchlists(),
      refreshWatchlistMemberships(),
      refreshFeedItems(),
      refreshCompanyEvents(),
      refreshSourceAdapters(),
      refreshSettings(),
      refreshGeminiCredentialStatus(),
    ]).then(() => {
      setDbRefreshState("done");
      window.setTimeout(() => {
        setDbRefreshState("idle");
      }, 900);
    });
  }

  function deleteUnsavedFeedItems() {
    const confirmed = window.confirm("Delete all unsaved feed items? Saved items will stay.");

    if (!confirmed) {
      return;
    }

    setDeleteUnsavedFeedState("refreshing");
    setDeleteUnsavedFeedError(null);

    invoke<FeedDeleteResult>("delete_unsaved_feed_items")
      .then(() => Promise.all([refreshFeedItems(), refreshDatabaseStatus()]))
      .then(() => {
        setDeleteUnsavedFeedState("done");
        window.setTimeout(() => {
          setDeleteUnsavedFeedState("idle");
        }, 900);
      })
      .catch((error) => {
        setDeleteUnsavedFeedError(String(error));
        setDeleteUnsavedFeedState("idle");
      });
  }

  function refreshSources(trigger: SourceRefreshTrigger = "manual") {
    if (sourceRefreshInFlightRef.current) {
      return Promise.resolve();
    }

    if (trigger === "scheduler") {
      if (!settings || settings.pollIntervalSeconds <= 0) {
        return Promise.resolve();
      }

      if (!anySourceAdapterRefreshDue(settings.pollIntervalSeconds * 1000)) {
        return Promise.resolve();
      }
    }

    sourceRefreshInFlightRef.current = true;
    setSourceRefreshState("refreshing");
    setSourceRefreshError(null);

    return invoke<SourceIngestionResult>("refresh_sources", { input: { trigger } })
      .then((response) => {
        setSourceRefreshResult(response);
        setSourceRefreshFailureCount(0);
        setSelectedSourceAdapterId(response.adapterId);
        return Promise.all([
          refreshFeedItems(),
          refreshCompanyEvents(),
          refreshSourceAdapters(),
          refreshDatabaseStatus(),
          refreshUnmatchedSourceItems(response.adapterId),
        ]);
      })
      .then(() => {
        setSourceRefreshState("done");
        window.setTimeout(() => {
          setSourceRefreshState("idle");
        }, 900);
      })
      .catch((error) => {
        setSourceRefreshError(String(error));
        setSourceRefreshFailureCount((current) => current + 1);
        setSourceRefreshState("idle");
        refreshSourceAdapters();
      })
      .finally(() => {
        sourceRefreshInFlightRef.current = false;
      });
  }

  function sourceAdapterRefreshDue(adapter: SourceAdapter, intervalMs: number) {
    const now = Date.now();

    if (!adapter.lastSuccessAt) {
      return true;
    }

    const lastSuccessAt = Date.parse(adapter.lastSuccessAt);
    if (Number.isNaN(lastSuccessAt)) {
      return true;
    }

    return now - lastSuccessAt >= intervalMs;
  }

  function anySourceAdapterRefreshDue(intervalMs: number) {
    return scheduledSourceAdapters.some((adapter) => sourceAdapterRefreshDue(adapter, intervalMs));
  }

  function refreshSingleSource(adapter: SourceAdapter, trigger: SourceRefreshTrigger = "manual") {
    if (sourceRefreshInFlightRef.current || sourceAdapterRefreshInFlight) {
      return Promise.resolve();
    }

    if (!adapter.enabled) {
      return Promise.resolve();
    }

    if (adapter.id === gpwRegistryAdapterId) {
      return refreshCompanyRegistry(trigger);
    }

    sourceRefreshInFlightRef.current = true;
    setSourceAdapterRefreshInFlight(adapter.id);
    setSourceRefreshState("refreshing");
    setSourceRefreshError(null);

    return invoke<SourceIngestionResult>("refresh_source", {
      input: {
        adapterId: adapter.id,
        trigger,
      },
    })
      .then((response) => {
        setSourceRefreshResult(response);
        setSourceRefreshFailureCount(0);
        setSelectedSourceAdapterId(response.adapterId);
        return Promise.all([
          refreshFeedItems(),
          refreshCompanyEvents(),
          refreshSourceAdapters(),
          refreshDatabaseStatus(),
          refreshUnmatchedSourceItems(response.adapterId),
        ]);
      })
      .then(() => {
        setSourceRefreshState("done");
        window.setTimeout(() => {
          setSourceRefreshState("idle");
        }, 900);
      })
      .catch((error) => {
        setSourceRefreshError(String(error));
        setSourceRefreshFailureCount((current) => current + 1);
        setSourceRefreshState("idle");
        refreshSourceAdapters();
      })
      .finally(() => {
        sourceRefreshInFlightRef.current = false;
        setSourceAdapterRefreshInFlight(null);
      });
  }

  function combineSourceResults(results: SourceIngestionResult[], adapterId: string): SourceIngestionResult {
    return results.reduce<SourceIngestionResult>(
      (combined, result) => ({
        adapterId,
        itemsFetched: combined.itemsFetched + result.itemsFetched,
        itemsCreated: combined.itemsCreated + result.itemsCreated,
        itemsMatched: combined.itemsMatched + result.itemsMatched,
        itemsUnmatched: combined.itemsUnmatched + result.itemsUnmatched,
        detailItemsAttempted: combined.detailItemsAttempted + result.detailItemsAttempted,
        detailItemsStored: combined.detailItemsStored + result.detailItemsStored,
        detailItemsFailed: combined.detailItemsFailed + result.detailItemsFailed,
        fetchedAt: result.fetchedAt ?? combined.fetchedAt,
      }),
      {
        adapterId,
        itemsFetched: 0,
        itemsCreated: 0,
        itemsMatched: 0,
        itemsUnmatched: 0,
        detailItemsAttempted: 0,
        detailItemsStored: 0,
        detailItemsFailed: 0,
        fetchedAt: null,
      },
    );
  }

  function refreshEventSources(trigger: SourceRefreshTrigger = "manual", date: string | null = null) {
    if (sourceRefreshInFlightRef.current || sourceAdapterRefreshInFlight) {
      return Promise.resolve();
    }

    sourceRefreshInFlightRef.current = true;
    setSourceAdapterRefreshInFlight("events");
    setSourceRefreshState("refreshing");
    setSourceRefreshError(null);

    return eventSourceAdapterIds
      .reduce<Promise<SourceIngestionResult[]>>(
        (chain, adapterId) =>
          chain.then((results) =>
            invoke<SourceIngestionResult>("refresh_source", {
              input: {
                adapterId,
                trigger,
                date: adapterId === "bankier-kalendarium-html" ? date : null,
              },
            }).then((result) => [...results, result]),
          ),
        Promise.resolve([]),
      )
      .then((results) => {
        const summary = combineSourceResults(results, "bankier-kalendarium-html");
        setSourceRefreshResult(summary);
        setSourceRefreshFailureCount(0);
        setSelectedSourceAdapterId("bankier-kalendarium-html");
        return Promise.all([
          refreshCompanyEvents(),
          refreshSourceAdapters(),
          refreshDatabaseStatus(),
          refreshUnmatchedSourceItems("bankier-kalendarium-html"),
        ]);
      })
      .then(() => {
        setSourceRefreshState("done");
        window.setTimeout(() => {
          setSourceRefreshState("idle");
        }, 900);
      })
      .catch((error) => {
        setSourceRefreshError(String(error));
        setSourceRefreshFailureCount((current) => current + 1);
        setSourceRefreshState("idle");
        refreshSourceAdapters();
      })
      .finally(() => {
        sourceRefreshInFlightRef.current = false;
        setSourceAdapterRefreshInFlight(null);
      });
  }

  function refreshBankierCalendarWeek(date: string, trigger: SourceRefreshTrigger = "manual") {
    if (sourceRefreshInFlightRef.current || sourceAdapterRefreshInFlight) {
      return Promise.resolve();
    }

    sourceRefreshInFlightRef.current = true;
    setSourceAdapterRefreshInFlight("bankier-kalendarium-html");
    setSourceRefreshState("refreshing");
    setSourceRefreshError(null);

    return invoke<SourceIngestionResult>("refresh_source", {
      input: {
        adapterId: "bankier-kalendarium-html",
        trigger,
        date,
      },
    })
      .then((response) => {
        setSourceRefreshResult(response);
        setSourceRefreshFailureCount(0);
        setSelectedSourceAdapterId(response.adapterId);
        return Promise.all([
          refreshCompanyEvents(),
          refreshSourceAdapters(),
          refreshDatabaseStatus(),
          refreshUnmatchedSourceItems(response.adapterId),
        ]);
      })
      .then(() => {
        setSourceRefreshState("done");
        window.setTimeout(() => {
          setSourceRefreshState("idle");
        }, 900);
      })
      .catch((error) => {
        setSourceRefreshError(String(error));
        setSourceRefreshFailureCount((current) => current + 1);
        setSourceRefreshState("idle");
        refreshSourceAdapters();
      })
      .finally(() => {
        sourceRefreshInFlightRef.current = false;
        setSourceAdapterRefreshInFlight(null);
      });
  }

  function refreshScheduledSource(adapterId: string, intervalMs: number) {
    const adapter = sourceAdaptersRef.current.find((sourceAdapter) => sourceAdapter.id === adapterId);

    if (!adapter || !adapter.enabled) {
      return Promise.resolve();
    }

    if (sourceRefreshInFlightRef.current || !sourceAdapterRefreshDue(adapter, intervalMs)) {
      return Promise.resolve();
    }

    sourceRefreshInFlightRef.current = true;
    setSourceRefreshState("refreshing");
    setSourceRefreshError(null);

    return invoke<SourceIngestionResult>("refresh_source", {
      input: {
        adapterId: adapter.id,
        trigger: "scheduler",
      },
    })
      .then((response) => {
        setSourceRefreshResult(response);
        setSourceRefreshFailureCount(0);
        return Promise.all([
          refreshFeedItems(),
          refreshCompanyEvents(),
          refreshSourceAdapters(),
          refreshDatabaseStatus(),
          refreshUnmatchedSourceItems(response.adapterId),
        ]);
      })
      .then(() => {
        setSourceRefreshState("done");
        window.setTimeout(() => {
          setSourceRefreshState("idle");
        }, 900);
      })
      .catch((error) => {
        setSourceRefreshError(String(error));
        setSourceRefreshFailureCount((current) => current + 1);
        setSourceRefreshState("idle");
        refreshSourceAdapters();
      })
      .finally(() => {
        sourceRefreshInFlightRef.current = false;
      });
  }

  function refreshCompanyRegistry(trigger: SourceRefreshTrigger = "manual") {
    setRegistryRefreshState("refreshing");
    setRegistryRefreshError(null);

    return invoke<CompanyRegistryRefreshResult>("refresh_gpw_company_registry", { input: { trigger } })
      .then((response) => {
        setRegistryRefreshResult(response);
        setSelectedSourceAdapterId(response.adapterId);
        return Promise.all([refreshSourceAdapters(), refreshDatabaseStatus(), refreshCompanyRegistryEntries()]);
      })
      .then(() => {
        setRegistryRefreshState("done");
        window.setTimeout(() => {
          setRegistryRefreshState("idle");
        }, 900);
      })
      .catch((error) => {
        setRegistryRefreshError(String(error));
        setRegistryRefreshState("idle");
        refreshSourceAdapters();
      });
  }

  function refreshCompanyRegistryIfStale(staleAfterSeconds: number) {
    return invoke<CompanyRegistryRefreshResult | null>("refresh_gpw_company_registry_if_stale", {
      input: {
        trigger: "scheduler",
        staleAfterSeconds,
      },
    })
      .then((response) => {
        if (!response) {
          return Promise.resolve();
        }

        setRegistryRefreshResult(response);
        return Promise.all([refreshSourceAdapters(), refreshDatabaseStatus(), refreshCompanyRegistryEntries()]).then(
          () => undefined,
        );
      })
      .catch((error) => {
        setRegistryRefreshError(String(error));
        refreshSourceAdapters();
      });
  }

  function pruneOldFeedItems() {
    return invoke<FeedPruneResult>("prune_old_feed_items", {
      input: {
        retentionDays: feedPruneRetentionDays,
      },
    })
      .then((response) => {
        setFeedPruneResult(response);
        return refreshFeedItems();
      })
      .catch(() => undefined);
  }

  useEffect(() => {
    sourceAdaptersRef.current = sourceAdapters;
  }, [sourceAdapters]);

  useEffect(() => {
    refreshCompanies();
    refreshWatchlists();
    refreshWatchlistMemberships();
    refreshFeedItems();
    refreshCompanyEvents();
    refreshTranscriptJobs();
    refreshSourceAdapters();
    refreshSettings();
    refreshGeminiCredentialStatus();
  }, []);

  useEffect(() => {
    void refreshCompanyEvents(companyEventMode);
  }, [
    companyEventCompanyFilter,
    companyEventDateFrom,
    companyEventDateTo,
    companyEventMode,
    companyEventStatusFilter,
    companyEventTypeFilter,
    companyEventViewMode,
    companyEventWeekRange.end,
    companyEventWeekRange.start,
    companyEventWatchlistFilter,
  ]);

  useEffect(() => {
    if (activeSection !== "Events" || companyEventViewMode !== "week") {
      return;
    }

    const weekStart = companyEventWeekRange.start;
    if (eventWeekFetchAttemptedRef.current.has(weekStart)) {
      return;
    }

    eventWeekFetchAttemptedRef.current.add(weekStart);
    void refreshBankierCalendarWeek(weekStart, "manual");
  }, [activeSection, companyEventViewMode, companyEventWeekRange.start]);

  useEffect(() => {
    const firstRunDelayMs = feedPruneInitialDelayMs + schedulerStartJitterMs(feedPruneIntervalMs);
    let intervalId: number | null = null;
    const timeoutId = window.setTimeout(() => {
      void pruneOldFeedItems();
      intervalId = window.setInterval(() => {
        void pruneOldFeedItems();
      }, feedPruneIntervalMs);
    }, firstRunDelayMs);

    return () => {
      window.clearTimeout(timeoutId);
      if (intervalId !== null) {
        window.clearInterval(intervalId);
      }
    };
  }, []);

  useEffect(() => {
    if (!settings || settings.pollIntervalSeconds <= 0 || scheduledSourceAdapters.length === 0) {
      setNextSourceRefreshAtByAdapterId({});
      return undefined;
    }

    const intervalSeconds = sourceRefreshFailureCount >= 2
      ? Math.min(settings.pollIntervalSeconds * 2, 3600)
      : settings.pollIntervalSeconds;
    const intervalMs = intervalSeconds * 1000;
    const timers = scheduledSourceAdapters.map((adapter) => {
      const firstRunDelayMs = intervalMs + schedulerStartJitterMs(intervalMs);
      setNextSourceRefreshAtByAdapterId((current) => ({
        ...current,
        [adapter.id]: Date.now() + firstRunDelayMs,
      }));

      let intervalId: number | null = null;
      const timeoutId = window.setTimeout(() => {
        setNextSourceRefreshAtByAdapterId((current) => ({
          ...current,
          [adapter.id]: Date.now() + intervalMs,
        }));
        void refreshScheduledSource(adapter.id, intervalMs);
        intervalId = window.setInterval(() => {
          setNextSourceRefreshAtByAdapterId((current) => ({
            ...current,
            [adapter.id]: Date.now() + intervalMs,
          }));
          void refreshScheduledSource(adapter.id, intervalMs);
        }, intervalMs);
      }, firstRunDelayMs);

      return { intervalId: () => intervalId, timeoutId };
    });

    return () => {
      for (const timer of timers) {
        window.clearTimeout(timer.timeoutId);
        const intervalId = timer.intervalId();
        if (intervalId !== null) {
          window.clearInterval(intervalId);
        }
      }
      setNextSourceRefreshAtByAdapterId({});
    };
  }, [settings?.pollIntervalSeconds, sourceRefreshFailureCount, scheduledSourceAdapterKey]);

  useEffect(() => {
    if (!registryAdapter?.enabled || registryAdapter.defaultPollIntervalSeconds <= 0) {
      setNextRegistryRefreshAt(null);
      return undefined;
    }

    const intervalMs = registryAdapter.defaultPollIntervalSeconds * 1000;
    const firstRunDelayMs = intervalMs + schedulerStartJitterMs(intervalMs);
    setNextRegistryRefreshAt(Date.now() + firstRunDelayMs);

    let intervalId: number | null = null;
    const timeoutId = window.setTimeout(() => {
      setNextRegistryRefreshAt(Date.now() + intervalMs);
      void refreshCompanyRegistryIfStale(registryAdapter.defaultPollIntervalSeconds);
      intervalId = window.setInterval(() => {
        setNextRegistryRefreshAt(Date.now() + intervalMs);
        void refreshCompanyRegistryIfStale(registryAdapter.defaultPollIntervalSeconds);
      }, intervalMs);
    }, firstRunDelayMs);

    return () => {
      window.clearTimeout(timeoutId);
      if (intervalId !== null) {
        window.clearInterval(intervalId);
      }
      setNextRegistryRefreshAt(null);
    };
  }, [registryAdapter?.enabled, registryAdapter?.defaultPollIntervalSeconds]);

  useEffect(() => {
    if (!selectedCompanyId) {
      setSelectedNotebookEntryId(null);
      setSelectedClaimEntryId(null);
      setNotebookEditMode(false);
      return;
    }

    refreshNotebookEntries(selectedCompanyId);
  }, [selectedCompanyId]);

  useEffect(() => {
    if (activeSection !== "Notebooks" || companies.length === 0) {
      return;
    }

    const nextCompanyId =
      selectedNotebookCompanyId && companies.some((company) => company.id === selectedNotebookCompanyId)
        ? selectedNotebookCompanyId
        : companies[0].id;

    if (selectedNotebookCompanyId !== nextCompanyId) {
      setSelectedNotebookCompanyId(nextCompanyId);
    }

    refreshNotebookEntries(nextCompanyId);
  }, [activeSection, companies, selectedNotebookCompanyId]);

  useEffect(() => {
    if (activeSection === "Companies") {
      refreshCompanyRegistryEntries();
    }
  }, [activeSection, companies.length]);

  useEffect(() => {
    if (!selectedNotebookEntry) {
      setNotebookEditMode(false);
      setNotebookEditForm(emptyNotebookForm());
      return;
    }

    setNotebookEditForm(notebookFormFromEntry(selectedNotebookEntry));
    setNotebookEditMode(false);
  }, [selectedNotebookEntry]);

  useEffect(() => {
    if (!selectedNotebookScreenEntry) {
      setNotebookScreenEditMode(false);
      setNotebookScreenEditForm(emptyNotebookForm());
      return;
    }

    setNotebookScreenEditForm(notebookFormFromEntry(selectedNotebookScreenEntry));
    setNotebookScreenEditMode(false);
  }, [selectedNotebookScreenEntry]);

  useEffect(() => {
    if (!selectedClaimEntry) {
      setClaimStatusDraft("");
      return;
    }

    setClaimStatusDraft(selectedClaimEntry.claimStatus ?? "open");
  }, [selectedClaimEntry]);

  function updateTheme(nextTheme: Theme) {
    setTheme(nextTheme);

    invoke<UserSettings>("update_settings", {
      input: {
        theme: nextTheme,
      },
    })
      .then((response) => {
        setSettings(response);
        setTheme(response.theme);
        setSettingsError(null);
      })
      .catch((error) => {
        setSettingsError(String(error));
      });
  }

  function updatePollInterval(nextPollIntervalSeconds: number) {
    invoke<UserSettings>("update_settings", {
      input: {
        pollIntervalSeconds: nextPollIntervalSeconds,
      },
    })
      .then((response) => {
        setSettings(response);
        setSettingsError(null);
      })
      .catch((error) => {
        setSettingsError(String(error));
      });
  }

  function updateYoutubeTranscriptionModel(nextModel: string) {
    invoke<UserSettings>("update_settings", {
      input: {
        youtubeTranscriptionModel: nextModel,
      },
    })
      .then((response) => {
        setSettings(response);
        setSettingsError(null);
      })
      .catch((error) => {
        setSettingsError(String(error));
      });
  }

  function updateYoutubeTranscriptionTimeout(nextTimeoutSeconds: number) {
    invoke<UserSettings>("update_settings", {
      input: {
        youtubeTranscriptionTimeoutSeconds: nextTimeoutSeconds,
      },
    })
      .then((response) => {
        setSettings(response);
        setSettingsError(null);
      })
      .catch((error) => {
        setSettingsError(String(error));
      });
  }

  function saveGeminiApiKey(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const apiKey = geminiApiKeyDraft.trim();
    if (!apiKey) {
      setGeminiCredentialError("Gemini API key is required.");
      return;
    }

    setGeminiCredentialInFlight(true);
    invoke<CredentialStatus>("set_gemini_transcription_api_key", {
      input: {
        apiKey,
      },
    })
      .then((response) => {
        setGeminiCredentialStatus(response);
        setGeminiCredentialError(null);
        setGeminiApiKeyDraft("");
      })
      .catch((error) => {
        setGeminiCredentialError(String(error));
      })
      .finally(() => {
        setGeminiCredentialInFlight(false);
      });
  }

  function clearGeminiApiKey() {
    setGeminiCredentialInFlight(true);
    invoke<CredentialStatus>("clear_gemini_transcription_api_key")
      .then((response) => {
        setGeminiCredentialStatus(response);
        setGeminiCredentialError(null);
        setGeminiApiKeyDraft("");
      })
      .catch((error) => {
        setGeminiCredentialError(String(error));
      })
      .finally(() => {
        setGeminiCredentialInFlight(false);
      });
  }

  function updateCompanyForm(field: keyof CompanyForm, value: string) {
    companyLookupVersionRef.current += 1;
    setSelectedCompanyRegistryTicker(null);
    setCompanyForm((current) => ({
      ...current,
      [field]: value,
    }));
  }

  function clearCompanyFormField(field: keyof CompanyForm) {
    companyLookupVersionRef.current += 1;
    skipNextCompanyLookupRef.current = true;
    setSelectedCompanyRegistryTicker(null);
    setLookupStatus(null);
    setCompanyForm((current) => ({
      ...current,
      [field]: field === "exchange" ? "GPW" : "",
    }));
    window.setTimeout(() => {
      companyFieldRefs.current[field]?.focus();
    }, 0);
  }

  function updateNotebookForm(field: keyof NotebookForm, value: string) {
    setNotebookForm((current) => ({
      ...current,
      [field]: value,
    }));
  }

  function updateNotebookEditForm(field: keyof NotebookForm, value: string) {
    setNotebookEditForm((current) => ({
      ...current,
      [field]: value,
    }));
  }

  function updateNotebookScreenForm(field: keyof NotebookForm, value: string) {
    setNotebookScreenForm((current) => ({
      ...current,
      [field]: value,
    }));
  }

  function updateNotebookScreenEditForm(field: keyof NotebookForm, value: string) {
    setNotebookScreenEditForm((current) => ({
      ...current,
      [field]: value,
    }));
  }

  function updateTranscriptNoteForm(field: keyof NotebookForm, value: string) {
    setTranscriptNoteForm((current) => ({
      ...current,
      [field]: value,
    }));
  }

  function findCompanyForFeedItem(item: FeedItem) {
    return companies.find((company) => company.qualifiedTicker === item.company) ?? null;
  }

  function feedItemSummary(item: FeedItem) {
    return item.summary.trim() || item.title;
  }

  function openFeedItemNoteDraft(item: FeedItem) {
    const company = findCompanyForFeedItem(item);

    if (!company) {
      return;
    }

    setSelectedNotebookCompanyId(company.id);
    setSelectedNotebookScreenEntryId(null);
    setNotebookScreenEditMode(false);
    setNotebookScreenComposerOpen(true);
    setNotebookScreenForm({
      title: item.title,
      body: item.bodyText || feedItemSummary(item),
      tags: ["feed", notebookTagFromFeedValue(item.type), notebookTagFromFeedValue(item.source)]
        .filter(Boolean)
        .join(", "),
      kind: "observation",
      claimStatus: "",
      eventDate: "",
      followUpAfter: "",
      followUpDate: "",
    });
    setNotebookScreenDraftOrigins([
      {
        sourceType: "feed_item",
        sourceId: item.id,
        sourceUrl: item.sourceUrl,
        label: `${item.source}: ${item.title}`,
      },
    ]);
    setNotebookError(null);
    setActiveSection("Notebooks");
    refreshNotebookEntries(company.id);
  }

  function refreshNotebookEntries(companyId: string) {
    return invoke<NotebookEntry[]>("list_notebook_entries", { companyId })
      .then((response) => {
        setNotebookEntries((current) => [
          ...response,
          ...current.filter((entry) => entry.companyId !== companyId),
        ]);
        setSelectedNotebookEntryId((current) => {
          if (current && response.some((entry) => entry.id === current)) {
            return current;
          }

          return response[0]?.id ?? null;
        });
        setSelectedNotebookScreenEntryId((current) => {
          if (selectedNotebookCompanyId !== companyId) {
            return current;
          }

          if (current && response.some((entry) => entry.id === current)) {
            return current;
          }

          return null;
        });
        setNotebookError(null);
      })
      .catch((error) => {
        setNotebookEntries((current) => current.filter((entry) => entry.companyId !== companyId));
        setNotebookError(String(error));
      });
  }

  function createNotebookEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!selectedCompany) {
      return;
    }

    invoke<NotebookEntry>("create_notebook_entry", {
      input: {
        companyId: selectedCompany.id,
        title: notebookForm.title,
        body: notebookForm.body,
        bodyFormat: "markdown",
        tags: notebookForm.tags
          .split(",")
          .map((tag) => tag.trim())
          .filter(Boolean),
        kind: notebookForm.kind,
        claimStatus: notebookForm.claimStatus || null,
        eventDate: notebookForm.eventDate || null,
        followUpAfter: notebookForm.followUpAfter || null,
        followUpDate: notebookForm.followUpDate || null,
        origins: [
          {
            sourceType: "manual",
            sourceId: null,
            sourceUrl: null,
            label: "Manual note",
          },
        ],
      },
    })
      .then((created) => {
        setNotebookForm(emptyNotebookForm());
        setNotebookComposerOpen(false);
        setSelectedNotebookEntryId(created.id);
        setNotebookError(null);
        refreshNotebookEntries(selectedCompany.id);
      })
      .catch((error) => {
        setNotebookError(String(error));
      });
  }

  function saveNotebookEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!selectedNotebookEntry || !selectedCompany) {
      return;
    }

    invoke<NotebookEntry>("update_notebook_entry", {
      input: {
        id: selectedNotebookEntry.id,
        title: notebookEditForm.title,
        body: notebookEditForm.body,
        tags: notebookEditForm.tags
          .split(",")
          .map((tag) => tag.trim())
          .filter(Boolean),
        kind: notebookEditForm.kind,
        claimStatus: notebookEditForm.claimStatus || null,
        eventDate: notebookEditForm.eventDate || null,
        followUpAfter: notebookEditForm.followUpAfter || null,
        followUpDate: notebookEditForm.followUpDate || null,
      },
    })
      .then((updated) => {
        setNotebookEntries((current) =>
          current.map((entry) => (entry.id === updated.id ? updated : entry)),
        );
        setSelectedNotebookEntryId(updated.id);
        setNotebookEditMode(false);
        setNotebookError(null);
        refreshNotebookEntries(selectedCompany.id);
      })
      .catch((error) => {
        setNotebookError(String(error));
      });
  }

  function cancelNotebookEdit() {
    if (selectedNotebookEntry) {
      setNotebookEditForm(notebookFormFromEntry(selectedNotebookEntry));
    }

    setNotebookEditMode(false);
  }

  function toggleClaimEntry(entry: NotebookEntry) {
    setSelectedClaimEntryId((current) => (current === entry.id ? null : entry.id));
  }

  function saveClaimStatus(entry: NotebookEntry) {
    if (!selectedCompany) {
      return;
    }

    invoke<NotebookEntry>("update_notebook_entry", {
      input: {
        id: entry.id,
        title: entry.title,
        body: entry.body,
        tags: entry.tags,
        kind: entry.kind,
        claimStatus: claimStatusDraft || null,
        eventDate: entry.eventDate,
        followUpAfter: entry.followUpAfter,
        followUpDate: entry.followUpDate,
      },
    })
      .then((updated) => {
        setNotebookEntries((current) =>
          current.map((notebookEntry) => (notebookEntry.id === updated.id ? updated : notebookEntry)),
        );
        setSelectedClaimEntryId(updated.id);
        setNotebookError(null);
        refreshNotebookEntries(selectedCompany.id);
      })
      .catch((error) => {
        setNotebookError(String(error));
      });
  }

  function selectNotebookScreenCompany(company: Company) {
    setSelectedNotebookCompanyId(company.id);
    setSelectedNotebookScreenEntryId(null);
    setNotebookScreenEditMode(false);
    setNotebookScreenComposerOpen(false);
    setNotebookScreenForm(emptyNotebookForm());
    setNotebookScreenDraftOrigins(manualNotebookOrigins());
    refreshNotebookEntries(company.id);
  }

  function showNotebookCompanyFollowUps(company: Company) {
    selectNotebookScreenCompany(company);
    setNotebookScreenKindFilter("all");
    setNotebookScreenClaimStatusFilter("all");
    setNotebookScreenFollowUpFilter("has_follow_up");
    setNotebookScreenTagFilter("");
  }

  function showNotebookCompanyOpenClaims(company: Company) {
    selectNotebookScreenCompany(company);
    setNotebookScreenKindFilter("all");
    setNotebookScreenClaimStatusFilter("open");
    setNotebookScreenFollowUpFilter("all");
    setNotebookScreenTagFilter("");
  }

  function toggleNotebookScreenComposer() {
    setNotebookScreenComposerOpen((current) => {
      const next = !current;

      if (next) {
        setNotebookScreenForm(emptyNotebookForm());
        setNotebookScreenDraftOrigins(manualNotebookOrigins());
      }

      return next;
    });
  }

  function discardNotebookScreenDraft() {
    setNotebookScreenComposerOpen(false);
    setNotebookScreenForm(emptyNotebookForm());
    setNotebookScreenDraftOrigins(manualNotebookOrigins());
  }

  function toggleNotebookScreenEntry(entry: NotebookEntry) {
    setSelectedNotebookScreenEntryId((current) => {
      const next = current === entry.id ? null : entry.id;

      if (next === null) {
        setNotebookScreenEditMode(false);
      }

      return next;
    });
  }

  function createNotebookScreenEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!selectedNotebookScreenCompany) {
      return;
    }

    invoke<NotebookEntry>("create_notebook_entry", {
      input: {
        companyId: selectedNotebookScreenCompany.id,
        title: notebookScreenForm.title,
        body: notebookScreenForm.body,
        bodyFormat: "markdown",
        tags: notebookScreenForm.tags
          .split(",")
          .map((tag) => tag.trim())
          .filter(Boolean),
        kind: notebookScreenForm.kind,
        claimStatus: notebookScreenForm.claimStatus || null,
        eventDate: notebookScreenForm.eventDate || null,
        followUpAfter: notebookScreenForm.followUpAfter || null,
        followUpDate: notebookScreenForm.followUpDate || null,
        origins: notebookScreenDraftOrigins,
      },
    })
      .then((created) => {
        setNotebookScreenForm(emptyNotebookForm());
        setNotebookScreenDraftOrigins(manualNotebookOrigins());
        setNotebookScreenComposerOpen(false);
        setSelectedNotebookScreenEntryId(created.id);
        setNotebookScreenEditMode(false);
        setNotebookError(null);
        refreshNotebookEntries(selectedNotebookScreenCompany.id);
      })
      .catch((error) => {
        setNotebookError(String(error));
      });
  }

  function saveNotebookScreenEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!selectedNotebookScreenEntry || !selectedNotebookScreenCompany) {
      return;
    }

    invoke<NotebookEntry>("update_notebook_entry", {
      input: {
        id: selectedNotebookScreenEntry.id,
        title: notebookScreenEditForm.title,
        body: notebookScreenEditForm.body,
        tags: notebookScreenEditForm.tags
          .split(",")
          .map((tag) => tag.trim())
          .filter(Boolean),
        kind: notebookScreenEditForm.kind,
        claimStatus: notebookScreenEditForm.claimStatus || null,
        eventDate: notebookScreenEditForm.eventDate || null,
        followUpAfter: notebookScreenEditForm.followUpAfter || null,
        followUpDate: notebookScreenEditForm.followUpDate || null,
      },
    })
      .then((updated) => {
        setNotebookEntries((current) =>
          current.map((entry) => (entry.id === updated.id ? updated : entry)),
        );
        setSelectedNotebookScreenEntryId(updated.id);
        setNotebookScreenEditMode(false);
        setNotebookError(null);
        refreshNotebookEntries(selectedNotebookScreenCompany.id);
      })
      .catch((error) => {
        setNotebookError(String(error));
      });
  }

  function cancelNotebookScreenEdit() {
    if (selectedNotebookScreenEntry) {
      setNotebookScreenEditForm(notebookFormFromEntry(selectedNotebookScreenEntry));
    }

    setNotebookScreenEditMode(false);
  }

  function applyLookupResult(result: CompanyLookupResult) {
    setSelectedCompanyRegistryTicker(result.qualifiedTicker);
    setCompanyForm({
      exchange: result.exchange,
      ticker: result.ticker,
      displayName: result.displayName,
      isin: result.isin,
    });
    setLookupStatus(`Filled from ${result.source}: ${result.qualifiedTicker}`);
  }

  function applyRegistryEntryToCompanyForm(entry: CompanyRegistryEntry) {
    companyLookupVersionRef.current += 1;
    setSelectedCompanyRegistryTicker(entry.qualifiedTicker);
    setCompanyForm({
      exchange: entry.exchange,
      ticker: entry.ticker,
      displayName: entry.displayName,
      isin: entry.isin ?? "",
    });
    setLookupStatus(`Selected from GPW registry: ${entry.qualifiedTicker}`);
  }

  function lookupCompany() {
    const lookupVersion = companyLookupVersionRef.current;
    setLookupStatus("Looking up GPW registry...");

    invoke<CompanyLookupResult | null>("lookup_company", {
      input: {
        exchange: companyForm.exchange,
        ticker: companyForm.ticker || null,
        displayName: companyForm.displayName || null,
        isin: companyForm.isin || null,
      },
    })
      .then((result) => {
        if (lookupVersion !== companyLookupVersionRef.current) {
          return;
        }

        if (result) {
          applyLookupResult(result);
        } else {
          setLookupStatus("No GPW registry match.");
        }
        setCompaniesError(null);
      })
      .catch((error) => {
        if (lookupVersion !== companyLookupVersionRef.current) {
          return;
        }

        setLookupStatus(null);
        setCompaniesError(String(error));
      });
  }

  function lookupCompanyIfUseful() {
    if (skipNextCompanyLookupRef.current) {
      skipNextCompanyLookupRef.current = false;
      return;
    }

    if (companyForm.ticker || companyForm.displayName.length >= 3 || companyForm.isin) {
      lookupCompany();
    }
  }

  function createCompany(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    invoke<Company>("create_company", {
      input: {
        exchange: companyForm.exchange,
        ticker: companyForm.ticker,
        displayName: companyForm.displayName,
        isin: companyForm.isin || null,
        cik: null,
        lei: null,
      },
    })
      .then(() => {
        setCompanyForm({
          exchange: companyForm.exchange.toUpperCase(),
          ticker: "",
          displayName: "",
          isin: "",
        });
        setCompaniesError(null);
        refreshCompanies();
        refreshDatabaseStatus();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setCompaniesError(String(error));
      });
  }

  function addCompanyFromRegistry(entry: CompanyRegistryEntry) {
    setAddingRegistryTicker(entry.qualifiedTicker);

    invoke<Company>("create_company", {
      input: {
        exchange: entry.exchange,
        ticker: entry.ticker,
        displayName: entry.displayName,
        isin: entry.isin,
        cik: null,
        lei: null,
      },
    })
      .then(() => {
        setCompaniesError(null);
        return Promise.all([
          refreshCompanies(),
          refreshCompanyRegistryEntries(),
          refreshDatabaseStatus(),
          refreshWatchlistMemberships(),
        ]);
      })
      .catch((error) => {
        setCompaniesError(String(error));
      })
      .finally(() => {
        setAddingRegistryTicker(null);
      });
  }

  function deleteCompany(company: Company) {
    const confirmed = window.confirm(`Delete ${company.qualifiedTicker} from your local registry?`);

    if (!confirmed) {
      return;
    }

    invoke<void>("delete_company", { companyId: company.id })
      .then(() => {
        setCompaniesError(null);
        refreshCompanies();
        refreshDatabaseStatus();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setCompaniesError(String(error));
      });
  }

  function createWatchlist(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    invoke<Watchlist>("create_watchlist", {
      input: {
        name: watchlistName,
        description: null,
      },
    })
      .then(() => {
        setWatchlistName("");
        setWatchlistsError(null);
        refreshWatchlists();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setWatchlistsError(String(error));
      });
  }

  function updateWatchlistAssignment(companyId: string, watchlistId: string) {
    setWatchlistAssignments((current) => ({
      ...current,
      [companyId]: watchlistId,
    }));
  }

  function addCompanyToWatchlist(company: Company) {
    const watchlistId = watchlistAssignments[company.id] || watchlists[0]?.id;
    const watchlistName = watchlists.find((watchlist) => watchlist.id === watchlistId)?.name;

    if (!watchlistId) {
      setWatchlistsError("Create a watchlist before assigning companies.");
      return;
    }

    invoke<void>("add_company_to_watchlist", {
      input: {
        watchlistId,
        companyId: company.id,
      },
    })
      .then(() => {
        setWatchlistsError(null);
        showWatchlistFeedback(company.id, `Assigned to ${watchlistName ?? "watchlist"}`);
        refreshWatchlists();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setWatchlistsError(String(error));
      });
  }

  function removeCompanyFromWatchlist(company: Company) {
    const watchlistId = watchlistAssignments[company.id] || watchlists[0]?.id;
    const watchlistName = watchlists.find((watchlist) => watchlist.id === watchlistId)?.name;

    if (!watchlistId) {
      setWatchlistsError("Select a watchlist before removing companies.");
      return;
    }

    invoke<void>("remove_company_from_watchlist", {
      input: {
        watchlistId,
        companyId: company.id,
      },
    })
      .then(() => {
        setWatchlistsError(null);
        showWatchlistFeedback(company.id, `Removed from ${watchlistName ?? "watchlist"}`);
        refreshWatchlists();
        refreshWatchlistMemberships();
      })
      .catch((error) => {
        setWatchlistsError(String(error));
      });
  }

  function showWatchlistFeedback(companyId: string, message: string) {
    setWatchlistFeedback({ companyId, message });
    window.setTimeout(() => {
      setWatchlistFeedback((current) => (current?.companyId === companyId ? null : current));
    }, 1200);
  }

  function updateFeedItemState(item: FeedItem, update: (item: FeedItem) => FeedItem) {
    const nextItem = update(item);

    invoke<FeedItem>("update_feed_item_state", {
      input: {
        id: nextItem.id,
        read: !nextItem.unread,
        saved: nextItem.saved,
      },
    })
      .then((response) => {
        setFeedState((current) =>
          current.map((item) => (item.id === response.id ? response : item)),
        );
        setFeedError(null);
      })
      .catch((error) => {
        setFeedError(String(error));
      });
  }

  function updateSelectedFeedItem(update: (item: FeedItem) => FeedItem) {
    if (!selectedFeedItem) {
      return;
    }

    updateFeedItemState(selectedFeedItem, update);
  }

  function toggleFeedItemReadState(item: FeedItem) {
    updateFeedItemState(item, (current) => ({
      ...current,
      unread: !current.unread,
    }));
  }

  function markVisibleInboxAsRead() {
    const unreadItems = filteredFeedItems.filter((item) => item.unread);
    if (unreadItems.length === 0) {
      return;
    }

    Promise.all(
      unreadItems.map((item) =>
        invoke<FeedItem>("update_feed_item_state", {
          input: {
            id: item.id,
            read: true,
            saved: item.saved,
          },
        }),
      ),
    )
      .then((responses) => {
        const updatedById = new Map(responses.map((item) => [item.id, item]));
        setFeedState((current) =>
          current.map((item) => updatedById.get(item.id) ?? item),
        );
        setFeedError(null);
      })
      .catch((error) => {
        setFeedError(String(error));
      });
  }

  function selectFeedItemFromKeyboard(event: KeyboardEvent<HTMLElement>, item: FeedItem) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      setSelectedFeedItemId(item.id);
    }

    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();

      const rows = Array.from(
        event.currentTarget.parentElement?.querySelectorAll<HTMLElement>("[data-feed-row='true']") ??
          [],
      );
      const currentIndex = rows.indexOf(event.currentTarget);
      const direction = event.key === "ArrowDown" ? 1 : -1;
      const nextIndex = Math.min(Math.max(currentIndex + direction, 0), rows.length - 1);
      const nextRow = rows[nextIndex];
      const nextFeedItemId = nextRow?.dataset.feedItemId;

      if (nextRow && nextFeedItemId) {
        nextRow.focus();
        setSelectedFeedItemId(nextFeedItemId);
      }
    }
  }

  function clearInboxFilters() {
    setSearchQuery("");
    setInboxWatchlistFilter("all");
    setInboxCompanyFilter("all");
    setInboxTypeFilter("all");
    setInboxSourceFilter("all");
    setInboxStatusFilter("all");
  }

  function openCompanyWorkspace(company: Company) {
    setSelectedCompanyId((current) => (current === company.id ? null : company.id));
    setCompanyWorkspaceTab("Feed");
    setActiveSection("Companies");
  }

  function focusCompanyWorkspace(companyId: string) {
    setSelectedCompanyId(companyId);
    setCompanyWorkspaceTab("Feed");
    setActiveSection("Companies");
  }

  function openCompanyWorkspaceFromKeyboard(event: KeyboardEvent<HTMLElement>, company: Company) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      openCompanyWorkspace(company);
    }

    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();

      const rows = Array.from(
        event.currentTarget
          .closest("[data-company-list='true']")
          ?.querySelectorAll<HTMLElement>("[data-company-row='true']") ?? [],
      );
      const currentIndex = rows.indexOf(event.currentTarget);
      const direction = event.key === "ArrowDown" ? 1 : -1;
      const nextIndex = Math.min(Math.max(currentIndex + direction, 0), rows.length - 1);
      const nextRow = rows[nextIndex];
      const nextCompanyId = nextRow?.dataset.companyId;

      if (nextRow && nextCompanyId) {
        nextRow.focus();
        if (selectedCompanyId) {
          focusCompanyWorkspace(nextCompanyId);
        }
      }
    }
  }

  function openCompanyWorkspaceFromFeedItem(item: FeedItem) {
    const company = companies.find((candidate) => candidate.qualifiedTicker === item.company);

    if (!company) {
      return;
    }

    setSelectedCompanyId(company.id);
    setSelectedCompanyFeedItemId(item.id);
    setCompanyWorkspaceTab("Feed");
    setActiveSection("Companies");
  }

  function inspectCompanyFeedItem(item: FeedItem) {
    setSelectedFeedItemId(item.id);
    setInboxCompanyFilter(item.company);
    setActiveSection("Inbox");
  }

  function openOriginFeedItem(origin: NotebookOrigin, companyId: string) {
    const company = companiesById[companyId];
    const originFeedItem = origin.sourceId
      ? feedState.find((item) => item.id === origin.sourceId)
      : null;

    setSearchQuery("");
    setInboxWatchlistFilter("all");
    setInboxCompanyFilter(company?.qualifiedTicker ?? originFeedItem?.company ?? "all");
    setInboxTypeFilter("all");
    setInboxSourceFilter("all");
    setInboxStatusFilter("all");

    if (origin.sourceId) {
      setSelectedFeedItemId(origin.sourceId);
    }

    setActiveSection("Inbox");
  }

  function renderCompanyEventRow(event: CompanyEvent) {
    const dueLabel = companyEventDueLabel(event.eventDate);
    const dueClass = companyEventDueClass(event.eventDate);

    return (
      <div className="event-row-block" key={event.id}>
        <article
          aria-label={`Open event: ${event.title}`}
          className={[
            "event-row",
            event.manual ? "event-manual" : "",
            dueClass,
            selectedCompanyEventId === event.id ? "event-row-selected" : "",
          ]
            .filter(Boolean)
            .join(" ")}
          onClick={() => setSelectedCompanyEventId((current) => (current === event.id ? null : event.id))}
          onKeyDown={(keyboardEvent) => {
            if (keyboardEvent.key === "Enter" || keyboardEvent.key === " ") {
              keyboardEvent.preventDefault();
              setSelectedCompanyEventId((current) => (current === event.id ? null : event.id));
            }
          }}
          role="button"
          tabIndex={0}
        >
          <div className="event-date-box">
            <CalendarDays size={16} aria-hidden="true" />
            <strong>{event.eventDate}</strong>
            {event.eventTime ? <span>{event.eventTime}</span> : null}
            {dueLabel ? <em>{dueLabel}</em> : null}
          </div>
          <div className="event-row-main">
            <div className="event-title-line">
              <h2>{event.title}</h2>
              <span>{formatCompanyEventType(event.eventType)}</span>
            </div>
            <p>
              <strong>{event.company}</strong> · {event.companyName}
            </p>
          </div>
          <div className="event-row-status">
            <span>{formatCompanyEventStatus(event.status)}</span>
            <small>{event.manual ? "Manual" : formatCompanyEventSourceType(event.sourceType)}</small>
          </div>
        </article>

        {selectedCompanyEvent?.id === event.id ? (
          <div className="event-detail-panel" aria-label="Event details">
            <dl className="metadata-grid">
              <div>
                <dt>Company</dt>
                <dd>{event.company}</dd>
              </div>
              <div>
                <dt>Type</dt>
                <dd>{formatCompanyEventType(event.eventType)}</dd>
              </div>
              <div>
                <dt>Status</dt>
                <dd>{formatCompanyEventStatus(event.status)}</dd>
              </div>
              <div>
                <dt>Source</dt>
                <dd>{formatCompanyEventSourceType(event.sourceType)}</dd>
              </div>
              <div>
                <dt>Attribution</dt>
                <dd>{event.attribution ?? "Not set"}</dd>
              </div>
              <div>
                <dt>Fetched</dt>
                <dd>{formatTimestamp(event.fetchedAt, "Not fetched")}</dd>
              </div>
            </dl>
            {event.sourceUrl ? (
              <button
                className="secondary-button compact-button"
                onClick={() => {
                  void openUrl(event.sourceUrl as string);
                }}
                type="button"
              >
                <ExternalLink size={15} />
                Open source
              </button>
            ) : null}
          </div>
        ) : null}
      </div>
    );
  }

  function renderCompanyEventWeekCard(event: CompanyEvent) {
    const isSelected = selectedCompanyEventId === event.id;
    const dueLabel = companyEventDueLabel(event.eventDate);
    const dueClass = companyEventDueClass(event.eventDate);

    return (
      <div className="event-week-card-block" key={event.id}>
        <article
          aria-label={`Open event: ${event.title}`}
          className={[
            "event-week-card",
            event.manual ? "event-manual" : "",
            dueClass,
            isSelected ? "event-week-card-selected" : "",
          ]
            .filter(Boolean)
            .join(" ")}
          onClick={() => setSelectedCompanyEventId((current) => (current === event.id ? null : event.id))}
          onKeyDown={(keyboardEvent) => {
            if (keyboardEvent.key === "Enter" || keyboardEvent.key === " ") {
              keyboardEvent.preventDefault();
              setSelectedCompanyEventId((current) => (current === event.id ? null : event.id));
            }
          }}
          role="button"
          tabIndex={0}
        >
          <div className="event-week-card-topline">
            <strong>{event.company}</strong>
            <div>
              {dueLabel ? <em>{dueLabel}</em> : null}
              <span>{formatCompanyEventStatus(event.status)}</span>
            </div>
          </div>
          <h2>{event.title}</h2>
          <div className="event-week-card-meta">
            <span>{formatCompanyEventType(event.eventType)}</span>
            <span>{event.manual ? "Manual" : formatCompanyEventSourceType(event.sourceType)}</span>
          </div>
        </article>

        {isSelected ? (
          <div className="event-week-card-detail" aria-label="Event details">
            <div className="event-week-card-full-title">
              <span>Title</span>
              <strong>{event.title}</strong>
            </div>
            <dl>
              <div>
                <dt>Company</dt>
                <dd>{event.companyName}</dd>
              </div>
              <div>
                <dt>Date</dt>
                <dd>{event.eventTime ? `${event.eventDate} ${event.eventTime}` : event.eventDate}</dd>
              </div>
              <div>
                <dt>Source</dt>
                <dd>{formatCompanyEventSourceType(event.sourceType)}</dd>
              </div>
              <div>
                <dt>Attribution</dt>
                <dd>{event.attribution ?? "Not set"}</dd>
              </div>
            </dl>
            {event.sourceUrl ? (
              <button
                className="secondary-button compact-button"
                onClick={() => {
                  void openUrl(event.sourceUrl as string);
                }}
                type="button"
              >
                <ExternalLink size={15} />
                Open source
              </button>
            ) : null}
          </div>
        ) : null}
      </div>
    );
  }

  function renderNotebookOrigins(origins: NotebookOrigin[], companyId: string) {
    if (origins.length === 0) {
      return <span className="membership-empty">None</span>;
    }

    return (
      <div className="origin-link-list">
        {origins.map((origin) => {
          const label = origin.label ?? origin.sourceType.replace("_", " ");
          const canOpenFeedItem = origin.sourceType === "feed_item" && Boolean(origin.sourceId);
          const hasOriginActions = canOpenFeedItem || Boolean(origin.sourceUrl);

          return (
            <div className="origin-link" key={origin.id}>
              <span>{label}</span>
              {hasOriginActions ? (
                <div className="origin-actions">
                  {canOpenFeedItem ? (
                    <button
                      aria-label={`Open origin feed item: ${label}`}
                      className="secondary-button compact-button"
                      onClick={() => openOriginFeedItem(origin, companyId)}
                      type="button"
                    >
                      <Inbox size={14} />
                      Feed item
                    </button>
                  ) : null}
                  {origin.sourceUrl ? (
                    <a
                      aria-label={`Open origin source: ${label}`}
                      className="secondary-button compact-button"
                      href={origin.sourceUrl}
                      rel="noreferrer"
                      target="_blank"
                    >
                      <ExternalLink size={14} />
                      Source
                    </a>
                  ) : null}
                </div>
              ) : null}
            </div>
          );
        })}
      </div>
    );
  }

  function openCompanyInboxFilter(company: Company) {
    setSelectedFeedItemId(null);
    setInboxCompanyFilter(company.qualifiedTicker);
    setActiveSection("Inbox");
  }

  function toggleCompanyFeedItem(item: FeedItem) {
    setSelectedCompanyFeedItemId((current) => (current === item.id ? null : item.id));
  }

  function selectCompanyFeedItemFromKeyboard(event: KeyboardEvent<HTMLElement>, item: FeedItem) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      toggleCompanyFeedItem(item);
    }

    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();

      const rows = Array.from(
        event.currentTarget
          .closest("[data-company-feed-list='true']")
          ?.querySelectorAll<HTMLElement>("[data-company-feed-row='true']") ?? [],
      );
      const currentIndex = rows.indexOf(event.currentTarget);
      const direction = event.key === "ArrowDown" ? 1 : -1;
      const nextIndex = Math.min(Math.max(currentIndex + direction, 0), rows.length - 1);
      const nextRow = rows[nextIndex];
      const nextFeedItemId = nextRow?.dataset.companyFeedItemId;

      if (nextRow && nextFeedItemId) {
        nextRow.focus();
        if (selectedCompanyFeedItemId) {
          setSelectedCompanyFeedItemId(nextFeedItemId);
        }
      }
    }
  }

  function toggleSourceAdapter(adapterId: string) {
    setSelectedSourceAdapterId((current) => (current === adapterId ? null : adapterId));
    if (adapterId === "gpw-company-registry") {
      refreshCompanyRegistryEntries();
    } else {
      refreshUnmatchedSourceItems(adapterId);
    }
  }

  function toggleUnmatchedSourceItems(adapterId: string) {
    setExpandedUnmatchedAdapters((current) => ({
      ...current,
      [adapterId]: !current[adapterId],
    }));
    refreshUnmatchedSourceItems(adapterId);
  }

  function toggleCompanyRegistryList() {
    setCompanyRegistryListExpanded((current) => !current);
    refreshCompanyRegistryEntries();
  }

  function openSourceStatus() {
    const relevantAdapter =
      sourceAdapters.find((adapter) => adapter.lastError) ??
      sourceAdapters.find((adapter) => adapter.enabled) ??
      sourceAdapters[0] ??
      null;

    setSelectedSourceAdapterId(relevantAdapter?.id ?? null);
    if (relevantAdapter) {
      if (relevantAdapter.id === "gpw-company-registry") {
        refreshCompanyRegistryEntries();
      } else {
        refreshUnmatchedSourceItems(relevantAdapter.id);
      }
    }
    setActiveSection("Sources");
  }

  function toggleSourceAdapterFromKeyboard(
    event: KeyboardEvent<HTMLElement>,
    adapterId: string,
  ) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      toggleSourceAdapter(adapterId);
    }
  }

  function clampDetailPaneWidth(width: number) {
    const gridWidth = contentGridRef.current?.getBoundingClientRect().width ?? 0;
    const responsiveMaxWidth = gridWidth > 0 ? Math.min(detailPaneMaxWidth, gridWidth * 0.55) : detailPaneMaxWidth;

    return Math.round(Math.min(Math.max(width, detailPaneMinWidth), responsiveMaxWidth));
  }

  function resizeDetailPaneFromPointer(clientX: number) {
    const gridBounds = contentGridRef.current?.getBoundingClientRect();

    if (!gridBounds) {
      return;
    }

    setDetailPaneWidth(clampDetailPaneWidth(gridBounds.right - clientX));
  }

  function startDetailPaneResize(event: PointerEvent<HTMLDivElement>) {
    event.currentTarget.setPointerCapture(event.pointerId);
    resizeDetailPaneFromPointer(event.clientX);
  }

  function resizeDetailPane(event: PointerEvent<HTMLDivElement>) {
    if (!event.currentTarget.hasPointerCapture(event.pointerId)) {
      return;
    }

    resizeDetailPaneFromPointer(event.clientX);
  }

  function stopDetailPaneResize(event: PointerEvent<HTMLDivElement>) {
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  function resizeDetailPaneWithKeyboard(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      setDetailPaneWidth((current) => clampDetailPaneWidth(current + 24));
    }

    if (event.key === "ArrowRight") {
      event.preventDefault();
      setDetailPaneWidth((current) => clampDetailPaneWidth(current - 24));
    }
  }

  function formatPollInterval(seconds: number) {
    if (seconds >= 86400 && seconds % 86400 === 0) {
      const days = seconds / 86400;
      return days === 1 ? "1 day" : `${days} days`;
    }

    if (seconds >= 86400) {
      const days = Math.floor(seconds / 86400);
      const hours = Math.floor((seconds % 86400) / 3600);

      if (hours === 0) {
        return days === 1 ? "1 day" : `${days} days`;
      }

      return `${days}d ${hours}h`;
    }

    if (seconds >= 3600) {
      const hours = Math.floor(seconds / 3600);
      const minutes = Math.floor((seconds % 3600) / 60);

      if (minutes === 0) {
        return `${hours}h`;
      }

      return `${hours}h ${minutes}m`;
    }

    if (seconds < 60) {
      return `${seconds}s`;
    }

    if (seconds % 60 === 0) {
      return `${seconds / 60} min`;
    }

    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;

    return `${minutes} min ${remainingSeconds}s`;
  }

  function formatSourceLastResult(adapter: SourceAdapter) {
    if (adapter.lastItemsFetched === null) {
      return "None";
    }

    if (adapter.id === gpwRegistryAdapterId) {
      return `${adapter.lastItemsFetched} cached entries · ${
        adapter.lastItemsCreated ?? 0
      } refreshed or updated`;
    }

    const listingResult = `${adapter.lastItemsFetched} fetched · ${adapter.lastItemsCreated ?? 0} created · ${
      adapter.lastItemsMatched ?? 0
    } matched · ${adapter.lastItemsUnmatched ?? 0} unmatched`;

    if (adapter.lastDetailItemsAttempted === null) {
      return listingResult;
    }

    return `${listingResult} · details ${adapter.lastDetailItemsStored ?? 0}/${
      adapter.lastDetailItemsAttempted
    } stored · ${adapter.lastDetailItemsFailed ?? 0} failed`;
  }

  function formatSourceType(value: string) {
    const labels: Record<string, string> = {
      official_report: "Official Reports",
      official_report_secondary: "Secondary Official Reports",
      official_calendar: "Official Calendar",
      public_calendar: "Public Calendar",
      public_media: "Public Media",
      analysis: "Analysis",
      authenticated_research: "Authenticated Research",
      company_registry: "Company Registry",
    };

    return labels[value] ?? formatEnumLabel(value);
  }

  function formatFetchMode(value: string) {
    const labels: Record<string, string> = {
      public_page: "Public Page",
      rss: "RSS",
      public_json: "Public JSON",
      api: "API",
      manual: "Manual",
      authenticated: "Authenticated",
      paywalled: "Paywalled",
    };

    return labels[value] ?? formatEnumLabel(value);
  }

  function formatSourceAccess(adapter: SourceAdapter) {
    if (!adapter.enabled) {
      return "Disabled";
    }

    const labels: Record<string, string> = {
      public_page: "Public Web Page",
      rss: "Public RSS",
      public_json: "Public JSON",
      api: "Public API",
      manual: "Manual/Local",
      authenticated: "Authenticated",
      paywalled: "Paywalled",
    };

    return labels[adapter.fetchMode] ?? formatFetchMode(adapter.fetchMode);
  }

  function formatSourceSubtitle(adapter: SourceAdapter) {
    if (adapter.id === gpwRegistryAdapterId) {
      return "Company registry · Public GPW company list";
    }

    return `${formatSourceType(adapter.sourceType)} · ${formatFetchMode(adapter.fetchMode)}`;
  }

  function sourceLastResultLabel(adapter: SourceAdapter) {
    return adapter.id === gpwRegistryAdapterId ? "Cache result" : "Last result";
  }

  function sourcePolicyLabel(adapter: SourceAdapter) {
    return adapter.id === gpwRegistryAdapterId ? "Refresh policy" : "Rate limit";
  }

  function formatSourceScheduler(adapter: SourceAdapter) {
    if (!adapter.enabled) {
      return "Off";
    }

    if (adapter.id === gpwRegistryAdapterId) {
      return adapter.defaultPollIntervalSeconds > 0
        ? `In-app · ${formatPollInterval(adapter.defaultPollIntervalSeconds)}`
        : "Off";
    }

    if (!settings || settings.pollIntervalSeconds <= 0) {
      return "Off";
    }

    if (sourceRefreshFailureCount >= 2) {
      return `In-app · ${formatPollInterval(settings.pollIntervalSeconds)} · backoff ${formatPollInterval(
        Math.min(settings.pollIntervalSeconds * 2, 3600),
      )}`;
    }

    return `In-app · ${formatPollInterval(settings.pollIntervalSeconds)}`;
  }

  function formatSourceTrigger(adapter: SourceAdapter) {
    if (adapter.lastTrigger === "scheduler") {
      return "Scheduler";
    }

    if (adapter.lastTrigger === "manual") {
      return "Manual";
    }

    return "None";
  }

  function formatNextRefresh(adapter: SourceAdapter) {
    if (!adapter.enabled) {
      return "Off";
    }

    const nextRefreshAt =
      adapter.id === gpwRegistryAdapterId ? nextRegistryRefreshAt : nextSourceRefreshAtByAdapterId[adapter.id];

    if (!nextRefreshAt) {
      return "Off";
    }

    const seconds = Math.max(0, Math.ceil((nextRefreshAt - Date.now()) / 1000));
    return `In ${formatPollInterval(seconds)}`;
  }

  function sourceRefreshButtonLabel() {
    if (sourceRefreshState === "refreshing") {
      return "Refreshing sources";
    }

    if (sourceRefreshState === "done") {
      return "Sources refreshed";
    }

    if (sourceRefreshError) {
      return "Source refresh failed";
    }

    return "Refresh sources";
  }

  function sourceRefreshButtonTitle() {
    if (sourceRefreshError) {
      return `Source refresh failed: ${sourceRefreshError}`;
    }

    if (sourceRefreshResult) {
      return `Last refresh: ${sourceRefreshResult.itemsMatched}/${sourceRefreshResult.itemsFetched} matched`;
    }

    return "Fetch GPW ESPI/EBI public listings";
  }

  function openExternalUrl(url: string) {
    void openUrl(url).catch((error) => {
      console.error("Failed to open external URL", error);
    });
  }

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="brand">
          <div className="brand-mark">B</div>
          <div>
            <div className="brand-title">Brawler</div>
            <div className="brand-subtitle">codename</div>
          </div>
        </div>

        <nav className="nav-list">
          {sections.map((section) => {
            const Icon = section.icon;
            return (
              <button
                className={activeSection === section.label ? "nav-item nav-item-active" : "nav-item"}
                key={section.label}
                onClick={() => setActiveSection(section.label)}
                type="button"
                title={section.label}
              >
                <Icon size={18} aria-hidden="true" />
                <span>{section.label}</span>
                {section.label === "Inbox" && totalUnreadFeedItems > 0 ? (
                  <span className="nav-badge" aria-label={`${totalUnreadFeedItems} unread feed item`}>
                    {totalUnreadFeedItems}
                  </span>
                ) : null}
              </button>
            );
          })}
        </nav>

        <div className="sidebar-footer">
          <div className="status-pill" title="Rust command boundary health">
            <span className={health ? "status-dot status-ok" : "status-dot status-warn"} />
            {health ? `${health.status} ${health.version}` : "health pending"}
          </div>
        </div>
      </aside>

      <main className="workspace">
        <header className="topbar">
          <div className="search-box">
            <Search size={18} aria-hidden="true" />
            <input
              aria-label="Search feed"
              placeholder="Search companies, feed, notes"
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
            />
          </div>
          <div className="topbar-actions">
            <div className="ai-mode-pill" title="AI mode: source-grounded decision support">
              <span>AI</span>
              <strong>source</strong>
            </div>
            <button
              aria-label="Open source status"
              className={[
                "source-status-pill",
                sourceStatusSummary.tone === "ok" ? "source-status-pill-ok" : "",
                sourceStatusSummary.tone === "warn" ? "source-status-pill-warn" : "",
                sourceStatusSummary.tone === "danger" ? "source-status-pill-danger" : "",
              ]
                .filter(Boolean)
                .join(" ")}
              onClick={openSourceStatus}
              title={sourceStatusSummary.title}
              type="button"
            >
              <Activity size={14} aria-hidden="true" />
              <span>Sources</span>
              <strong>{sourceStatusSummary.label}</strong>
            </button>
            <button
              aria-label={
                dbRefreshState === "refreshing"
                  ? "Refreshing database-backed views"
                  : "Refresh database-backed views"
              }
              className={[
                "db-status-pill",
                dbRefreshState === "refreshing" ? "icon-button-spinning" : "",
                dbRefreshState === "done" ? "db-status-pill-success" : "",
              ]
                .filter(Boolean)
                .join(" ")}
              disabled={dbRefreshState === "refreshing"}
              onClick={refreshDatabaseBackedViews}
              type="button"
              title={
                dbRefreshState === "refreshing"
                  ? "Refreshing database-backed views"
                  : dbRefreshState === "done"
                    ? "Database-backed views refreshed"
                    : databaseStatus
                  ? `Database active: ${databaseStatus.appliedMigrations} migration, ${databaseStatus.sourceAdapters} source, ${databaseStatus.settings} settings`
                  : databaseError
                    ? `Database error: ${databaseError}`
                    : "Database pending"
              }
            >
              <span
                aria-label={
                  databaseError
                    ? "Database connection failed"
                    : databaseStatus
                      ? "Database connection active"
                      : "Database connection pending"
                }
                className={databaseIndicatorClass(databaseStatus, databaseError)}
                role="status"
              />
              <span>DB</span>
              {dbRefreshState === "done" ? (
                <CheckCircle2 size={14} aria-hidden="true" />
              ) : (
                <RefreshCw size={14} aria-hidden="true" />
              )}
            </button>
            <button
              aria-label={sourceRefreshButtonLabel()}
              className={[
                "icon-button",
                sourceRefreshState === "refreshing" ? "icon-button-spinning" : "",
                sourceRefreshState === "done" ? "db-status-pill-success" : "",
                sourceRefreshError ? "source-refresh-button-danger" : "",
              ]
                .filter(Boolean)
                .join(" ")}
              disabled={sourceRefreshState === "refreshing"}
              onClick={() => {
                void refreshSources("manual");
              }}
              type="button"
              title={sourceRefreshButtonTitle()}
            >
              {sourceRefreshState === "done" ? <CheckCircle2 size={18} /> : <RefreshCw size={18} />}
            </button>
            <label className="theme-control" title="Theme">
              {effectiveTheme === "dark" ? <Moon size={16} /> : <Sun size={16} />}
              <select value={theme} onChange={(event) => updateTheme(event.target.value as Theme)}>
                <option value="dark">Dark</option>
                <option value="light">Light</option>
                <option value="system">System</option>
              </select>
            </label>
          </div>
        </header>

        <section
          className={activeSection === "Inbox" ? "content-grid" : "content-grid content-grid-single"}
          ref={contentGridRef}
          style={
            activeSection === "Inbox"
              ? ({ "--detail-pane-width": `${detailPaneWidth}px` } as CSSProperties)
              : undefined
          }
        >
          {activeSection === "Inbox" ? (
            <section className="feed-panel" aria-labelledby="inbox-title">
              <div className="panel-header">
                <div>
                  <h1 id="inbox-title">Inbox</h1>
                  <p>Stored feed items filtered by local companies and watchlists.</p>
                </div>
                <div className="segmented-control" aria-label="Feed status filter">
                  <button
                    type="button"
                    className={inboxStatusFilter === "all" ? "segment-active" : undefined}
                    onClick={() => setInboxStatusFilter("all")}
                  >
                    <Inbox size={14} />
                    All
                  </button>
                  <button
                    type="button"
                    className={inboxStatusFilter === "unread" ? "segment-active" : undefined}
                    onClick={() => setInboxStatusFilter("unread")}
                  >
                    <Mail size={14} />
                    Unread
                  </button>
                  <button
                    type="button"
                    className={inboxStatusFilter === "saved" ? "segment-active" : undefined}
                    onClick={() => setInboxStatusFilter("saved")}
                  >
                    <Save size={14} />
                    Saved
                  </button>
                </div>
              </div>

              <div className="filter-reset-row" aria-label="Inbox filter reset">
                <div className="inbox-review-summary" aria-label="Inbox review summary">
                  <span>
                    <strong>{inboxReviewStats.visible}</strong> visible
                  </span>
                  <span>
                    <strong>{inboxReviewStats.unread}</strong> unread
                  </span>
                  <span>
                    <strong>{inboxReviewStats.saved}</strong> saved
                  </span>
                </div>
                <button
                  className="secondary-button compact-button"
                  disabled={inboxReviewStats.unread === 0}
                  onClick={markVisibleInboxAsRead}
                  type="button"
                >
                  <MailOpen size={15} />
                  Mark all read
                </button>
                <button
                  className="secondary-button compact-button"
                  disabled={deleteUnsavedFeedState === "refreshing"}
                  onClick={deleteUnsavedFeedItems}
                  type="button"
                >
                  {deleteUnsavedFeedState === "done" ? <CheckCircle2 size={15} /> : <Trash2 size={15} />}
                  {deleteUnsavedFeedState === "refreshing" ? "Deleting" : "Delete unsaved"}
                </button>
                <button
                  className="secondary-button compact-button"
                  disabled={!hasActiveInboxFilters}
                  onClick={clearInboxFilters}
                  type="button"
                >
                  <X size={15} />
                  Clear filters
                </button>
              </div>

              <div className="filter-toolbar" aria-label="Inbox filters">
                <label>
                  Watchlist
                  <select
                    aria-label="Inbox watchlist"
                    value={inboxWatchlistFilter}
                    onChange={(event) => setInboxWatchlistFilter(event.target.value)}
                  >
                    <option value="all">All watchlists</option>
                    {watchlists.map((watchlist) => (
                      <option key={watchlist.id} value={watchlist.id}>
                        {watchlist.name}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  Company
                  <select
                    aria-label="Inbox company"
                    value={inboxCompanyFilter}
                    onChange={(event) => setInboxCompanyFilter(event.target.value)}
                  >
                    <option value="all">All companies</option>
                    {companies.map((company) => (
                      <option key={company.id} value={company.qualifiedTicker}>
                        {company.qualifiedTicker}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  Type
                  <select
                    aria-label="Inbox type"
                    value={inboxTypeFilter}
                    onChange={(event) => setInboxTypeFilter(event.target.value)}
                  >
                    <option value="all">All types</option>
                    {feedTypes.map((type) => (
                      <option key={type} value={type}>
                        {type}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  Source
                  <select
                    aria-label="Inbox source"
                    value={inboxSourceFilter}
                    onChange={(event) => setInboxSourceFilter(event.target.value)}
                  >
                    <option value="all">All sources</option>
                    {feedSources.map((source) => (
                      <option key={source} value={source}>
                        {source}
                      </option>
                    ))}
                  </select>
                </label>
              </div>

              <div className="feed-list" aria-label="Feed items">
                {filteredFeedItems.map((item) => (
                  <article
                    aria-label={`Select feed item: ${item.title}`}
                    aria-current={selectedFeedItem?.id === item.id ? "true" : undefined}
                    className={[
                      "feed-row",
                      item.unread ? "unread" : "",
                      selectedFeedItem?.id === item.id ? "feed-row-selected" : "",
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    key={item.id}
                    data-feed-item-id={item.id}
                    data-feed-row="true"
                    onClick={() => setSelectedFeedItemId(item.id)}
                    onDoubleClick={() => toggleFeedItemReadState(item)}
                    onKeyDown={(event) => selectFeedItemFromKeyboard(event, item)}
                    role="button"
                    tabIndex={0}
                    title="Select feed item"
                  >
                    <div className="feed-row-main">
                      <div className="feed-meta">
                        <span>{item.company}</span>
                        <span>{item.type}</span>
                        <span>{item.source}</span>
                        <span>{formatTimestamp(item.time, "Unknown")}</span>
                      </div>
                      <h2>{item.title}</h2>
                      <p>{feedItemSummary(item)}</p>
                    </div>
                    {item.saved ? <span className="saved-pill">Saved</span> : null}
                    {item.unread ? <span className="unread-dot" title="Unread" /> : null}
                  </article>
                ))}
                {inboxEmptyState ? (
                  <div className="empty-state">
                    {inboxEmptyState === "no-companies" ? (
                      <>
                        <span>No companies tracked yet.</span>
                        <button
                          className="secondary-button compact-button"
                          onClick={() => setActiveSection("Companies")}
                          type="button"
                        >
                          <Plus size={15} />
                          Add company
                        </button>
                      </>
                    ) : null}

                    {inboxEmptyState === "no-feed" ? (
                      <>
                        <span>No stored feed items yet.</span>
                        <div className="empty-state-actions">
                          <button
                            className="secondary-button compact-button"
                            disabled={sourceRefreshState === "refreshing"}
                            onClick={() => {
                              void refreshSources("manual");
                            }}
                            title="Fetch GPW ESPI/EBI public listings"
                            type="button"
                          >
                            {sourceRefreshState === "done" ? <CheckCircle2 size={15} /> : <RefreshCw size={15} />}
                            {sourceRefreshState === "refreshing" ? "Refreshing" : "Refresh sources"}
                          </button>
                          <button
                            className="secondary-button compact-button"
                            onClick={openSourceStatus}
                            type="button"
                          >
                            <Activity size={15} />
                            Open Sources
                          </button>
                        </div>
                      </>
                    ) : null}

                    {inboxEmptyState === "no-matches" ? (
                      <>
                        <span>No feed items for selected filters.</span>
                        {hasActiveInboxFilters ? (
                          <button
                            className="secondary-button compact-button"
                            onClick={clearInboxFilters}
                            type="button"
                          >
                            <X size={15} />
                            Clear filters
                          </button>
                        ) : null}
                      </>
                    ) : null}
                  </div>
                ) : null}
                {feedError ? <p className="error-text">Feed command failed: {feedError}</p> : null}
                {deleteUnsavedFeedError ? (
                  <p className="error-text">Delete unsaved failed: {deleteUnsavedFeedError}</p>
                ) : null}
                {sourceRefreshError ? (
                  <p className="error-text">Source refresh failed: {sourceRefreshError}</p>
                ) : null}
              </div>
            </section>
          ) : null}

          {activeSection === "Companies" ? (
            <section className="feed-panel" aria-labelledby="companies-title">
              <div className="panel-header">
                <div>
                  <h1 id="companies-title">Companies</h1>
                  <p>Local company registry backed by SQLite.</p>
                </div>
              </div>

              <div className="companies-layout">
                <section className="watchlist-panel" aria-labelledby="watchlists-title">
                  <div className="subsection-header">
                    <div>
                      <h2 id="watchlists-title">Watchlists</h2>
                      <p>Local groups for companies.</p>
                    </div>
                    <form className="watchlist-form" onSubmit={createWatchlist}>
                      <input
                        aria-label="Watchlist name"
                        placeholder="Main GPW"
                        value={watchlistName}
                        onChange={(event) => setWatchlistName(event.target.value)}
                        required
                      />
                      <button className="primary-button" type="submit">
                        <Plus size={16} />
                        Create
                      </button>
                    </form>
                  </div>

                  <div className="watchlist-list" aria-label="Watchlist chips">
                    {watchlists.map((watchlist) => (
                      <div className="watchlist-chip" key={watchlist.id}>
                        <span>{watchlist.name}</span>
                        <strong>{watchlist.companyCount}</strong>
                      </div>
                    ))}
                    {watchlists.length === 0 ? (
                      <div className="empty-state">No watchlists yet.</div>
                    ) : null}
                  </div>

                  {watchlistsError ? (
                    <p className="error-text">Watchlist command failed: {watchlistsError}</p>
                  ) : null}
                </section>

                <form className="company-form" onSubmit={createCompany}>
                  <label>
                    Exchange
                    <span className="field-with-clear">
                      <input
                        ref={(element) => {
                          companyFieldRefs.current.exchange = element;
                        }}
                        required
                        value={companyForm.exchange}
                        onChange={(event) => updateCompanyForm("exchange", event.target.value)}
                      />
                      {companyForm.exchange.trim().toUpperCase() !== "GPW" ? (
                        <button
                          aria-label="Clear exchange"
                          className="field-clear-button"
                          onClick={() => clearCompanyFormField("exchange")}
                          onMouseDown={(event) => event.preventDefault()}
                          title="Clear exchange"
                          type="button"
                        >
                          <X size={13} />
                        </button>
                      ) : null}
                    </span>
                  </label>
                  <label>
                    Ticker
                    <span className="field-with-clear">
                      <input
                        ref={(element) => {
                          companyFieldRefs.current.ticker = element;
                        }}
                        required
                        value={companyForm.ticker}
                        onBlur={lookupCompanyIfUseful}
                        onChange={(event) => updateCompanyForm("ticker", event.target.value)}
                        placeholder="CDR"
                      />
                      {companyForm.ticker.trim().length > 0 ? (
                        <button
                          aria-label="Clear ticker"
                          className="field-clear-button"
                          onClick={() => clearCompanyFormField("ticker")}
                          onMouseDown={(event) => event.preventDefault()}
                          title="Clear ticker"
                          type="button"
                        >
                          <X size={13} />
                        </button>
                      ) : null}
                    </span>
                  </label>
                  <label>
                    Name
                    <span className="field-with-clear">
                      <input
                        ref={(element) => {
                          companyFieldRefs.current.displayName = element;
                        }}
                        required
                        value={companyForm.displayName}
                        onBlur={lookupCompanyIfUseful}
                        onChange={(event) => updateCompanyForm("displayName", event.target.value)}
                        placeholder="CD PROJEKT S.A."
                      />
                      {companyForm.displayName.trim().length > 0 ? (
                        <button
                          aria-label="Clear name"
                          className="field-clear-button"
                          onClick={() => clearCompanyFormField("displayName")}
                          onMouseDown={(event) => event.preventDefault()}
                          title="Clear name"
                          type="button"
                        >
                          <X size={13} />
                        </button>
                      ) : null}
                    </span>
                  </label>
                  <label>
                    ISIN
                    <span className="field-with-clear">
                      <input
                        ref={(element) => {
                          companyFieldRefs.current.isin = element;
                        }}
                        value={companyForm.isin}
                        onBlur={lookupCompanyIfUseful}
                        onChange={(event) => updateCompanyForm("isin", event.target.value)}
                        placeholder="PLOPTTC00011"
                      />
                      {companyForm.isin.trim().length > 0 ? (
                        <button
                          aria-label="Clear ISIN"
                          className="field-clear-button"
                          onClick={() => clearCompanyFormField("isin")}
                          onMouseDown={(event) => event.preventDefault()}
                          title="Clear ISIN"
                          type="button"
                        >
                          <X size={13} />
                        </button>
                      ) : null}
                    </span>
                  </label>
                  <button
                    className="secondary-button"
                    onClick={lookupCompany}
                    onMouseDown={(event) => event.preventDefault()}
                    type="button"
                  >
                    <LocateFixed size={16} />
                    Lookup
                  </button>
                  <button
                    className="primary-button"
                    onMouseDown={(event) => event.preventDefault()}
                    type="submit"
                  >
                    <Plus size={16} />
                    Add
                  </button>
                  {companyFormRegistryMatches.length > 0 ? (
                    <div className="company-registry-suggestions" aria-label="Company registry suggestions">
                      <span>Registry matches</span>
                      <div>
                        {companyFormRegistryMatches.map((entry) => (
                          <button
                            className="company-registry-suggestion"
                            key={entry.qualifiedTicker}
                            onClick={() => applyRegistryEntryToCompanyForm(entry)}
                            onMouseDown={(event) => event.preventDefault()}
                            title={`Use ${entry.qualifiedTicker}`}
                            type="button"
                          >
                            <strong>{entry.qualifiedTicker}</strong>
                            <span>{entry.displayName}</span>
                            <small>{entry.isin ?? "No ISIN"}</small>
                            {entry.tracked ? <em>Added</em> : null}
                          </button>
                        ))}
                      </div>
                    </div>
                  ) : null}
                </form>

                <div className="company-list-toolbar" aria-label="Company list search">
                  <label className="registry-search-field">
                    <Search size={15} />
                    <input
                      aria-label="Search tracked companies"
                      onChange={(event) => setCompanyListSearch(event.target.value)}
                      placeholder="Search tracked companies"
                      type="text"
                      value={companyListSearch}
                    />
                    {companyListSearch.trim().length > 0 ? (
                      <button
                        aria-label="Clear company search"
                        className="field-clear-button"
                        onClick={() => setCompanyListSearch("")}
                        onMouseDown={(event) => event.preventDefault()}
                        title="Clear company search"
                        type="button"
                      >
                        <X size={13} />
                      </button>
                    ) : null}
                  </label>
                  <span>
                    {filteredCompanies.length}/{companies.length} companies
                  </span>
                </div>

                <div className="company-list" aria-label="Companies list" data-company-list="true">
                  {filteredCompanies.map((company) => (
                    <div className="company-row-block" key={company.id}>
                      <article
                        aria-label={`Open ${company.qualifiedTicker} workspace`}
                        className={[
                          "company-row",
                          selectedCompany?.id === company.id ? "company-row-selected" : "",
                        ]
                          .filter(Boolean)
                          .join(" ")}
                        data-company-id={company.id}
                        data-company-row="true"
                        onClick={() => openCompanyWorkspace(company)}
                        onKeyDown={(event) => openCompanyWorkspaceFromKeyboard(event, company)}
                        role="button"
                        tabIndex={0}
                        title={`Open ${company.qualifiedTicker} workspace`}
                      >
                        <div className="company-row-main">
                          <h2>{company.qualifiedTicker}</h2>
                          <p>{company.displayName}</p>
                          <div
                            className="membership-list"
                            aria-label={`Watchlist memberships for ${company.qualifiedTicker}`}
                          >
                            {(membershipsByCompany[company.id] ?? []).map((membership) => (
                              <span className="membership-chip" key={membership.watchlistId}>
                                {membership.watchlistName}
                              </span>
                            ))}
                            {(membershipsByCompany[company.id] ?? []).length === 0 ? (
                              <span className="membership-empty">No watchlist</span>
                            ) : null}
                          </div>
                        </div>
                        <div className="company-row-actions" onClick={(event) => event.stopPropagation()}>
                          <span>{company.isin ?? "No ISIN"}</span>
                          <select
                            aria-label={`Watchlist for ${company.qualifiedTicker}`}
                            disabled={watchlists.length === 0}
                            value={watchlistAssignments[company.id] || watchlists[0]?.id || ""}
                            onChange={(event) =>
                              updateWatchlistAssignment(company.id, event.target.value)
                            }
                          >
                            {watchlists.map((watchlist) => (
                              <option key={watchlist.id} value={watchlist.id}>
                                {watchlist.name}
                              </option>
                            ))}
                          </select>
                          <button
                            className="secondary-button compact-button assign-button"
                            disabled={watchlists.length === 0}
                            onClick={() => addCompanyToWatchlist(company)}
                            type="button"
                          >
                            <Plus size={15} />
                            Assign
                          </button>
                          <button
                            className="secondary-button compact-button remove-button"
                            disabled={watchlists.length === 0}
                            onClick={() => removeCompanyFromWatchlist(company)}
                            type="button"
                          >
                            <X size={15} />
                            Remove
                          </button>
                          {watchlistFeedback?.companyId === company.id ? (
                            <span
                              aria-label={watchlistFeedback.message}
                              className="inline-success"
                              role="status"
                              title={watchlistFeedback.message}
                            >
                              <CheckCircle2 size={16} />
                            </span>
                          ) : null}
                          <button
                            className="icon-button danger-button"
                            onClick={() => deleteCompany(company)}
                            title={`Delete ${company.qualifiedTicker}`}
                            type="button"
                          >
                            <Trash2 size={16} />
                          </button>
                        </div>
                      </article>

                      {selectedCompany?.id === company.id ? (
                        <section className="company-workspace" aria-label="Company workspace">
                          <div className="company-workspace-header">
                            <div>
                              <span className="eyebrow">Company workspace</span>
                              <h2>{selectedCompany.qualifiedTicker}</h2>
                              <p>{selectedCompany.displayName}</p>
                            </div>
                            <div className="company-workspace-meta" aria-label="Selected company metadata">
                              <span>{selectedCompany.exchange}</span>
                              <span>{selectedCompany.isin ?? "No ISIN"}</span>
                              <span>{selectedCompanyFeedStats.total} feed</span>
                              <span>{selectedCompanyFeedStats.unread} unread</span>
                              <span>{selectedCompanyFeedStats.saved} saved</span>
                              {(membershipsByCompany[selectedCompany.id] ?? []).map((membership) => (
                                <span key={membership.watchlistId}>{membership.watchlistName}</span>
                              ))}
                            </div>
                          </div>

                          <div className="segmented-control company-tabs" aria-label="Company workspace tabs">
                            {(["Feed", "Notebook", "Claims", "Transcripts", "Metadata"] as const).map(
                              (tab) => {
                                const TabIcon =
                                  tab === "Feed"
                                    ? Inbox
                                    : tab === "Notebook"
                                      ? BookOpenText
                                      : tab === "Claims"
                                        ? CheckCircle2
                                        : tab === "Transcripts"
                                          ? Video
                                          : FileText;

                                return (
                                  <button
                                    className={companyWorkspaceTab === tab ? "segment-active" : undefined}
                                    key={tab}
                                    onClick={() => setCompanyWorkspaceTab(tab)}
                                    type="button"
                                  >
                                    <TabIcon size={14} />
                                    {tab}
                                  </button>
                                );
                              },
                            )}
                          </div>

                          {companyWorkspaceTab === "Feed" ? (
                            <div
                              className="company-tab-panel"
                              aria-label="Company feed"
                              data-company-feed-list="true"
                            >
                              {selectedCompanyFeedItems.map((item) => (
                                <div className="company-feed-row-block" key={item.id}>
                                  <article
                                    aria-label={`Open company feed item: ${item.title}`}
                                    className={[
                                      "company-feed-row",
                                      item.unread ? "unread" : "",
                                      selectedCompanyFeedItem?.id === item.id
                                        ? "company-feed-row-selected"
                                        : "",
                                    ]
                                      .filter(Boolean)
                                      .join(" ")}
                                    data-company-feed-item-id={item.id}
                                    data-company-feed-row="true"
                                    onClick={() => toggleCompanyFeedItem(item)}
                                    onKeyDown={(event) => selectCompanyFeedItemFromKeyboard(event, item)}
                                    role="button"
                                    tabIndex={0}
                                    title="Open company feed item details"
                                  >
                                    <div className="feed-row-main">
                                      <div className="feed-meta">
                                        <span>{item.type}</span>
                                        <span>{item.source}</span>
                                        <span>{formatTimestamp(item.time, "Unknown")}</span>
                                      </div>
                                      <h3>{item.title}</h3>
                                      <p>{feedItemSummary(item)}</p>
                                    </div>
                                    {item.saved ? <span className="saved-pill">Saved</span> : null}
                                    {item.unread ? <span className="unread-dot" title="Unread" /> : null}
                                  </article>

                                  {selectedCompanyFeedItem?.id === item.id ? (
                                    <aside className="company-feed-detail" aria-label="Company feed item details">
                                      <div>
                                        <span className="eyebrow">Selected item</span>
                                        <h3>{selectedCompanyFeedItem.title}</h3>
                                        <section className="feed-body-section" aria-label="Feed summary">
                                          <div className="feed-body-heading">
                                            <span>Summary</span>
                                          </div>
                                          <p className="feed-detail-body">{feedItemSummary(selectedCompanyFeedItem)}</p>
                                        </section>
                                        <details className="feed-body-section feed-body-disclosure" aria-label="Official report body">
                                          <summary className="feed-body-heading">
                                            <span>Official report body</span>
                                            <strong>{selectedCompanyFeedItem.bodyText ? "Stored" : "Not stored"}</strong>
                                          </summary>
                                          {selectedCompanyFeedItem.bodyText ? (
                                            <p className="feed-detail-body">{selectedCompanyFeedItem.bodyText}</p>
                                          ) : (
                                            <p className="feed-detail-empty">
                                              No official report body is stored for this item yet. Refresh sources and
                                              check Sources for detail warnings if this remains empty.
                                            </p>
                                          )}
                                        </details>
                                      </div>
                                      <div className="detail-actions" aria-label="Company feed item actions">
                                        <button
                                          className="secondary-button compact-button"
                                          onClick={() =>
                                            updateFeedItemState(selectedCompanyFeedItem, (feedItem) => ({
                                              ...feedItem,
                                              unread: !feedItem.unread,
                                            }))
                                          }
                                          type="button"
                                        >
                                          {selectedCompanyFeedItem.unread ? (
                                            <MailOpen size={15} />
                                          ) : (
                                            <Mail size={15} />
                                          )}
                                          {selectedCompanyFeedItem.unread ? "Mark read" : "Mark unread"}
                                        </button>
                                        <button
                                          className="secondary-button compact-button"
                                          onClick={() =>
                                            updateFeedItemState(selectedCompanyFeedItem, (feedItem) => ({
                                              ...feedItem,
                                              saved: !feedItem.saved,
                                            }))
                                          }
                                          type="button"
                                        >
                                          <Save size={15} />
                                          {selectedCompanyFeedItem.saved ? "Unsave" : "Save"}
                                        </button>
                                        <button
                                          className="secondary-button compact-button"
                                          onClick={() => inspectCompanyFeedItem(selectedCompanyFeedItem)}
                                          type="button"
                                        >
                                          <Inbox size={15} />
                                          Open in Inbox
                                        </button>
                                        <button
                                          className="secondary-button compact-button"
                                          onClick={() => openFeedItemNoteDraft(selectedCompanyFeedItem)}
                                          type="button"
                                        >
                                          <BookOpenText size={15} />
                                          Note
                                        </button>
                                        <a
                                          className="secondary-button compact-button"
                                          href={selectedCompanyFeedItem.sourceUrl}
                                          rel="noreferrer"
                                          target="_blank"
                                        >
                                          <ExternalLink size={15} />
                                          Open source
                                        </a>
                                      </div>
                                      <dl className="metadata-grid">
                                        <div>
                                          <dt>Source</dt>
                                          <dd>{selectedCompanyFeedItem.source}</dd>
                                        </div>
                                        <div>
                                          <dt>Type</dt>
                                          <dd>{selectedCompanyFeedItem.type}</dd>
                                        </div>
                                        <div>
                                          <dt>Published</dt>
                                          <dd>{formatTimestamp(selectedCompanyFeedItem.publishedAt, "Unknown")}</dd>
                                        </div>
                                        <div>
                                          <dt>Fetched</dt>
                                          <dd>{formatTimestamp(selectedCompanyFeedItem.fetchedAt, "Unknown")}</dd>
                                        </div>
                                        <div>
                                          <dt>Attribution</dt>
                                          <dd>{selectedCompanyFeedItem.attribution}</dd>
                                        </div>
                                        <div>
                                          <dt>Language</dt>
                                          <dd>{selectedCompanyFeedItem.language}</dd>
                                        </div>
                                      </dl>
                                      {selectedCompanyFeedItem.attachments.length > 0 ? (
                                        <div className="feed-attachment-list" aria-label="Company feed attachments">
                                          {selectedCompanyFeedItem.attachments.map((attachment) => (
                                            <a
                                              className="feed-attachment-link"
                                              href={attachment.url}
                                              key={attachment.id}
                                              rel="noreferrer"
                                              target="_blank"
                                            >
                                              <ExternalLink size={14} />
                                              {attachment.label}
                                            </a>
                                          ))}
                                        </div>
                                      ) : null}
                                    </aside>
                                  ) : null}
                                </div>
                              ))}
                              {selectedCompanyFeedItems.length === 0 ? (
                                <div className="empty-state company-feed-empty">
                                  <div>
                                    <strong>No stored feed items for {selectedCompany.qualifiedTicker} yet.</strong>
                                    <p>
                                      This company is tracked locally, but no sample or ingested items are attached to
                                      it yet.
                                    </p>
                                  </div>
                                  <button
                                    className="secondary-button compact-button"
                                    onClick={() => openCompanyInboxFilter(selectedCompany)}
                                  type="button"
                                >
                                  <Inbox size={15} />
                                  Open filtered Inbox
                                </button>
                                </div>
                              ) : null}
                            </div>
                          ) : null}

                          {companyWorkspaceTab === "Notebook" ? (
                            <div className="company-tab-panel notebook-panel" aria-label="Company notebook">
                              <div className="notebook-toolbar">
                                <div>
                                  <h3>Notebook</h3>
                                  <p>
                                    {selectedCompanyNotebookEntries.length} note
                                    {selectedCompanyNotebookEntries.length === 1 ? "" : "s"} for{" "}
                                    {selectedCompany.qualifiedTicker}
                                  </p>
                                </div>
                                <button
                                  className="primary-button compact-button"
                                  onClick={() => setNotebookComposerOpen((current) => !current)}
                                  type="button"
                                >
                                  {isNotebookComposerOpen ? <X size={15} /> : <Plus size={15} />}
                                  {isNotebookComposerOpen ? "Hide form" : "New note"}
                                </button>
                              </div>

                              {isNotebookComposerOpen ? (
                                <form className="notebook-form" onSubmit={createNotebookEntry}>
                                  <div className="notebook-form-grid">
                                    <label>
                                      Title
                                      <input
                                        aria-label="Notebook note title"
                                        value={notebookForm.title}
                                        onChange={(event) => updateNotebookForm("title", event.target.value)}
                                      />
                                    </label>
                                    <label>
                                      Kind
                                      <select
                                        aria-label="Notebook note kind"
                                        value={notebookForm.kind}
                                        onChange={(event) => updateNotebookForm("kind", event.target.value)}
                                      >
                                        <option value="manual">Manual</option>
                                        <option value="observation">Observation</option>
                                        <option value="claim">Claim</option>
                                        <option value="question">Question</option>
                                        <option value="follow_up">Follow-up</option>
                                      </select>
                                    </label>
                                    <label>
                                      Tags
                                      <input
                                        aria-label="Notebook note tags"
                                        placeholder="comma, separated"
                                        value={notebookForm.tags}
                                        onChange={(event) => updateNotebookForm("tags", event.target.value)}
                                      />
                                    </label>
                                    <label>
                                      Claim status
                                      <select
                                        aria-label="Notebook claim status"
                                        value={notebookForm.claimStatus}
                                        onChange={(event) => updateNotebookForm("claimStatus", event.target.value)}
                                      >
                                        <option value="">None</option>
                                        <option value="open">Open</option>
                                        <option value="delivered">Delivered</option>
                                        <option value="partially_delivered">Partially delivered</option>
                                        <option value="missed">Missed</option>
                                        <option value="unknown">Unknown</option>
                                        <option value="not_applicable">Not applicable</option>
                                      </select>
                                    </label>
                                    <NotebookDateField
                                      ariaLabel="Notebook event date"
                                      label="Event date"
                                      value={notebookForm.eventDate}
                                      onChange={(value) => updateNotebookForm("eventDate", value)}
                                    />
                                    <NotebookQuarterField
                                      ariaLabel="Notebook follow-up quarter"
                                      label="Follow-up quarter"
                                      value={notebookForm.followUpAfter}
                                      onChange={(value) => updateNotebookForm("followUpAfter", value)}
                                    />
                                    <NotebookDateField
                                      ariaLabel="Notebook follow-up date"
                                      label="Follow-up date"
                                      value={notebookForm.followUpDate}
                                      onChange={(value) => updateNotebookForm("followUpDate", value)}
                                    />
                                    <button
                                      className="primary-button compact-button notebook-submit-button"
                                      disabled={!notebookForm.title.trim() || !notebookForm.body.trim()}
                                      type="submit"
                                    >
                                      <Save size={15} />
                                      Save
                                    </button>
                                  </div>
                                  <label className="notebook-body-field">
                                    Body
                                    <textarea
                                      aria-label="Notebook note body"
                                      value={notebookForm.body}
                                      onChange={(event) => updateNotebookForm("body", event.target.value)}
                                    />
                                  </label>
                                </form>
                              ) : null}

                              <div className="notebook-workspace">
                                <div className="notebook-list" aria-label="Notebook entries">
                                  {selectedCompanyNotebookEntries.map((entry) => (
                                    <button
                                      aria-label={`Select notebook entry: ${entry.title}`}
                                      className={[
                                        "notebook-row",
                                        selectedNotebookEntry?.id === entry.id ? "notebook-row-selected" : "",
                                      ]
                                        .filter(Boolean)
                                        .join(" ")}
                                      key={entry.id}
                                      onClick={() => setSelectedNotebookEntryId(entry.id)}
                                      type="button"
                                    >
                                      <div>
                                        <div className="notebook-row-top">
                                          <h3>{entry.title}</h3>
                                          <span>{entry.kind.replace("_", " ")}</span>
                                        </div>
                                      </div>
                                      <div className="notebook-row-meta">
                                        {entry.claimStatus ? <span>{entry.claimStatus.replace("_", " ")}</span> : null}
                                        {entry.followUpAfter ? <span>{entry.followUpAfter}</span> : null}
                                        {entry.tags.slice(0, 2).map((tag) => (
                                          <span key={tag}>{tag}</span>
                                        ))}
                                      </div>
                                    </button>
                                  ))}
                                  {selectedCompanyNotebookEntries.length === 0 ? (
                                    <div className="empty-state">
                                      <span>No notebook entries for {selectedCompany.qualifiedTicker} yet.</span>
                                    </div>
                                  ) : null}
                                </div>
                                <form
                                  className="notebook-detail"
                                  aria-label="Notebook entry detail"
                                  onSubmit={saveNotebookEntry}
                                >
                                  {selectedNotebookEntry ? (
                                    isNotebookEditMode ? (
                                      <>
                                        <div className="notebook-entry-header">
                                          <label>
                                            Title
                                            <input
                                              aria-label="Selected notebook title"
                                              value={notebookEditForm.title}
                                              onChange={(event) =>
                                                updateNotebookEditForm("title", event.target.value)
                                              }
                                            />
                                          </label>
                                          <div className="notebook-detail-actions">
                                            <button
                                              className="secondary-button compact-button"
                                              onClick={cancelNotebookEdit}
                                              type="button"
                                            >
                                              <X size={15} />
                                              Cancel
                                            </button>
                                            <button
                                              className="primary-button compact-button"
                                              disabled={
                                                !isNotebookEditDirty ||
                                                !notebookEditForm.title.trim() ||
                                                !notebookEditForm.body.trim()
                                              }
                                              type="submit"
                                            >
                                              <Save size={15} />
                                              Save
                                            </button>
                                          </div>
                                        </div>
                                        <textarea
                                          aria-label="Selected notebook body"
                                          value={notebookEditForm.body}
                                          onChange={(event) => updateNotebookEditForm("body", event.target.value)}
                                        />
                                        <div className="notebook-detail-grid">
                                          <label>
                                            Kind
                                            <select
                                              aria-label="Selected notebook kind"
                                              value={notebookEditForm.kind}
                                              onChange={(event) => updateNotebookEditForm("kind", event.target.value)}
                                            >
                                              <option value="manual">Manual</option>
                                              <option value="observation">Observation</option>
                                              <option value="claim">Claim</option>
                                              <option value="question">Question</option>
                                              <option value="follow_up">Follow-up</option>
                                            </select>
                                          </label>
                                          <label>
                                            Claim status
                                            <select
                                              aria-label="Selected notebook claim status"
                                              value={notebookEditForm.claimStatus}
                                              onChange={(event) =>
                                                updateNotebookEditForm("claimStatus", event.target.value)
                                              }
                                            >
                                              <option value="">None</option>
                                              <option value="open">Open</option>
                                              <option value="delivered">Delivered</option>
                                              <option value="partially_delivered">Partially delivered</option>
                                              <option value="missed">Missed</option>
                                              <option value="unknown">Unknown</option>
                                              <option value="not_applicable">Not applicable</option>
                                            </select>
                                          </label>
                                          <label>
                                            Tags
                                            <input
                                              aria-label="Selected notebook tags"
                                              value={notebookEditForm.tags}
                                              onChange={(event) => updateNotebookEditForm("tags", event.target.value)}
                                            />
                                          </label>
                                          <NotebookDateField
                                            ariaLabel="Selected notebook event date"
                                            label="Event date"
                                            value={notebookEditForm.eventDate}
                                            onChange={(value) => updateNotebookEditForm("eventDate", value)}
                                          />
                                          <NotebookQuarterField
                                            ariaLabel="Selected notebook follow-up quarter"
                                            label="Follow-up quarter"
                                            value={notebookEditForm.followUpAfter}
                                            onChange={(value) => updateNotebookEditForm("followUpAfter", value)}
                                          />
                                          <NotebookDateField
                                            ariaLabel="Selected notebook follow-up date"
                                            label="Follow-up date"
                                            value={notebookEditForm.followUpDate}
                                            onChange={(value) => updateNotebookEditForm("followUpDate", value)}
                                          />
                                        </div>
                                      </>
                                    ) : (
                                      <>
                                        <div className="notebook-entry-header">
                                          <div>
                                            <span className="eyebrow">
                                              {selectedNotebookEntry.kind.replace("_", " ")}
                                            </span>
                                            <h3>{selectedNotebookEntry.title}</h3>
                                          </div>
                                          <button
                                            className="secondary-button compact-button"
                                            onClick={() => setNotebookEditMode(true)}
                                            type="button"
                                          >
                                            <BookOpenText size={15} />
                                            Edit
                                          </button>
                                        </div>
                                        <MarkdownNoteBody
                                          ariaLabel="Selected notebook body"
                                          body={selectedNotebookEntry.body}
                                        />
                                      </>
                                    )
                                  ) : (
                                    <div className="empty-state">
                                      <span>Select a note to inspect it.</span>
                                    </div>
                                  )}
                                  {selectedNotebookEntry ? (
                                    <>
                                      <div
                                        className="source-chip-list"
                                        aria-label={`Tags for ${selectedNotebookEntry.title}`}
                                      >
                                        {selectedNotebookEntry.tags.map((tag) => (
                                          <span className="membership-chip" key={tag}>
                                            {tag}
                                          </span>
                                        ))}
                                        {selectedNotebookEntry.tags.length === 0 ? (
                                          <span className="membership-empty">No tags</span>
                                        ) : null}
                                      </div>
                                      <dl className="metadata-grid notebook-entry-meta">
                                        <div>
                                          <dt>Status</dt>
                                          <dd>{selectedNotebookEntry.claimStatus ?? "Not set"}</dd>
                                        </div>
                                        <div>
                                          <dt>Event</dt>
                                          <dd>{selectedNotebookEntry.eventDate ?? "Not set"}</dd>
                                        </div>
                                        <div>
                                          <dt>Follow-up quarter</dt>
                                          <dd>{selectedNotebookEntry.followUpAfter ?? "Not set"}</dd>
                                        </div>
                                        <div>
                                          <dt>Follow-up date</dt>
                                          <dd>{selectedNotebookEntry.followUpDate ?? "Not set"}</dd>
                                        </div>
                                        <div>
                                          <dt>Origin</dt>
                                          <dd>{renderNotebookOrigins(selectedNotebookEntry.origins, selectedNotebookEntry.companyId)}</dd>
                                        </div>
                                      </dl>
                                    </>
                                  ) : null}
                                </form>
                              </div>
                              {notebookError ? (
                                <p className="error-text">Notebook command failed: {notebookError}</p>
                              ) : null}
                            </div>
                          ) : null}

                          {companyWorkspaceTab === "Claims" ? (
                            <div className="company-tab-panel claims-panel" aria-label="Company claims">
                              <div className="notebook-toolbar">
                                <div>
                                  <h3>Claims</h3>
                                  <p>
                                    {selectedCompanyClaimEntries.length} follow-up item
                                    {selectedCompanyClaimEntries.length === 1 ? "" : "s"} for{" "}
                                    {selectedCompany.qualifiedTicker}
                                  </p>
                                </div>
                              </div>
                              <div className="claims-list">
                                {selectedCompanyClaimEntries.map((entry) => (
                                  <div className="claim-row-block" key={entry.id}>
                                    <button
                                      aria-label={`Open claim: ${entry.title}`}
                                      className={[
                                        "notebook-row",
                                        selectedClaimEntry?.id === entry.id ? "notebook-row-selected" : "",
                                      ]
                                        .filter(Boolean)
                                        .join(" ")}
                                      onClick={() => toggleClaimEntry(entry)}
                                      type="button"
                                    >
                                      <div>
                                        <div className="notebook-row-top">
                                          <h3>{entry.title}</h3>
                                          <span>{entry.claimStatus?.replace("_", " ") ?? "open"}</span>
                                        </div>
                                      </div>
                                      <div className="notebook-row-meta">
                                        {entry.followUpAfter ? <span>{entry.followUpAfter}</span> : null}
                                        {entry.followUpDate ? <span>{entry.followUpDate}</span> : null}
                                        {entry.tags.slice(0, 2).map((tag) => (
                                          <span key={tag}>{tag}</span>
                                        ))}
                                      </div>
                                    </button>

                                    {selectedClaimEntry?.id === entry.id ? (
                                      <div className="claim-detail" aria-label="Claim detail">
                                        <div className="notebook-entry-header">
                                          <div>
                                            <span className="eyebrow">
                                              {entry.kind.replace("_", " ")}
                                            </span>
                                            <h3>{entry.title}</h3>
                                          </div>
                                          <div className="claim-status-control">
                                            <label>
                                              Status
                                              <select
                                                aria-label="Claim status"
                                                value={claimStatusDraft}
                                                onChange={(event) => setClaimStatusDraft(event.target.value)}
                                              >
                                                <option value="open">Open</option>
                                                <option value="delivered">Delivered</option>
                                                <option value="partially_delivered">Partially delivered</option>
                                                <option value="missed">Missed</option>
                                                <option value="unknown">Unknown</option>
                                                <option value="not_applicable">Not applicable</option>
                                              </select>
                                            </label>
                                            <button
                                              className="primary-button compact-button"
                                              disabled={(entry.claimStatus ?? "open") === claimStatusDraft}
                                              onClick={() => saveClaimStatus(entry)}
                                              type="button"
                                            >
                                              <Save size={15} />
                                              Save
                                            </button>
                                          </div>
                                        </div>
                                        <MarkdownNoteBody body={entry.body} />
                                        <dl className="metadata-grid notebook-entry-meta">
                                          <div>
                                            <dt>Event</dt>
                                            <dd>{entry.eventDate ?? "Not set"}</dd>
                                          </div>
                                          <div>
                                            <dt>Follow-up quarter</dt>
                                            <dd>{entry.followUpAfter ?? "Not set"}</dd>
                                          </div>
                                          <div>
                                            <dt>Follow-up date</dt>
                                            <dd>{entry.followUpDate ?? "Not set"}</dd>
                                          </div>
                                          <div>
                                            <dt>Origin</dt>
                                            <dd>{renderNotebookOrigins(entry.origins, entry.companyId)}</dd>
                                          </div>
                                        </dl>
                                      </div>
                                    ) : null}
                                  </div>
                                ))}
                                {selectedCompanyClaimEntries.length === 0 ? (
                                  <div className="empty-state">
                                    <span>No claim notes for {selectedCompany.qualifiedTicker} yet.</span>
                                  </div>
                                ) : null}
                              </div>
                              {notebookError ? (
                                <p className="error-text">Notebook command failed: {notebookError}</p>
                              ) : null}
                            </div>
                          ) : null}

                          {companyWorkspaceTab === "Transcripts" ? (
                            <div className="company-tab-panel empty-state">
                              YouTube transcript workflows start in Milestone 7.
                            </div>
                          ) : null}

                          {companyWorkspaceTab === "Metadata" ? (
                            <dl className="company-tab-panel metadata-grid" aria-label="Company metadata">
                              <div>
                                <dt>Qualified ticker</dt>
                                <dd>{selectedCompany.qualifiedTicker}</dd>
                              </div>
                              <div>
                                <dt>Exchange</dt>
                                <dd>{selectedCompany.exchange}</dd>
                              </div>
                              <div>
                                <dt>Ticker</dt>
                                <dd>{selectedCompany.ticker}</dd>
                              </div>
                              <div>
                                <dt>ISIN</dt>
                                <dd>{selectedCompany.isin ?? "Not set"}</dd>
                              </div>
                              <div>
                                <dt>CIK</dt>
                                <dd>{selectedCompany.cik ?? "Not set"}</dd>
                              </div>
                              <div>
                                <dt>LEI</dt>
                                <dd>{selectedCompany.lei ?? "Not set"}</dd>
                              </div>
                            </dl>
                          ) : null}
                        </section>
                      ) : null}
                    </div>
                  ))}
                  {companies.length === 0 ? (
                    <div className="empty-state">No companies yet.</div>
                  ) : null}
                  {companies.length > 0 && filteredCompanies.length === 0 ? (
                    <div className="empty-state">No companies match this search.</div>
                  ) : null}
                </div>

                {companiesError ? (
                  <p className="error-text">Companies command failed: {companiesError}</p>
                ) : null}
                {lookupStatus ? <p className="helper-text">{lookupStatus}</p> : null}
              </div>
            </section>
          ) : null}

          {activeSection === "Notebooks" ? (
            <section className="feed-panel" aria-labelledby="notebooks-title">
              <div className="panel-header">
                <div>
                  <h1 id="notebooks-title">Notebooks</h1>
                  <p>Company-first research notes for daily notes work.</p>
                </div>
              </div>

              <div className="notebooks-screen" aria-label="Notebooks workspace">
                <div className="notebooks-company-nav" aria-label="Notebook companies">
                  {companies.map((company) => {
                    const companyNotes = notebookEntries.filter((entry) => entry.companyId === company.id);
                    const openClaims = companyNotes.filter((entry) => entry.claimStatus === "open").length;
                    const followUpScheduled = companyNotes.filter(
                      (entry) => entry.followUpAfter || entry.followUpDate,
                    ).length;

                    return (
                      <div
                        className={[
                          "notebooks-company-row",
                          selectedNotebookScreenCompany?.id === company.id
                            ? "notebooks-company-row-selected"
                            : "",
                        ]
                          .filter(Boolean)
                          .join(" ")}
                        key={company.id}
                      >
                        <button
                          aria-label={`Open notebook company: ${company.qualifiedTicker}`}
                          className="notebooks-company-select"
                          onClick={() => selectNotebookScreenCompany(company)}
                          type="button"
                        >
                          <span>
                            <strong>{company.qualifiedTicker}</strong>
                            <small>{company.displayName}</small>
                          </span>
                        </button>
                        <div className="notebooks-company-cues">
                          <span
                            className="notebooks-company-count"
                            aria-label={`${companyNotes.length} notebook entries for ${company.qualifiedTicker}`}
                          >
                            {companyNotes.length}
                          </span>
                          {openClaims > 0 ? (
                            <button
                              aria-label={`Show open claims for ${company.qualifiedTicker}`}
                              className="notebooks-company-cue notebooks-company-cue-button"
                              onClick={() => showNotebookCompanyOpenClaims(company)}
                              type="button"
                            >
                              {openClaims} open
                            </button>
                          ) : null}
                          {followUpScheduled > 0 ? (
                            <button
                              aria-label={`Show follow-ups for ${company.qualifiedTicker}`}
                              className="notebooks-company-cue notebooks-company-cue-button"
                              onClick={() => showNotebookCompanyFollowUps(company)}
                              type="button"
                            >
                              {followUpScheduled} follow-up
                            </button>
                          ) : null}
                        </div>
                        <button
                          aria-label={`Open company workspace: ${company.qualifiedTicker}`}
                          className="notebooks-company-action"
                          onClick={() => focusCompanyWorkspace(company.id)}
                          title={`Open ${company.qualifiedTicker} workspace`}
                          type="button"
                        >
                          <LocateFixed size={14} />
                        </button>
                      </div>
                    );
                  })}
                  {companies.length === 0 ? (
                    <div className="empty-state">
                      <span>Add companies before using notebooks.</span>
                    </div>
                  ) : null}
                </div>

                <div className="notebooks-main" aria-label="Notebook screen entries">
                  <div className="notebooks-context-line">
                    <div>
                      <strong>{selectedNotebookScreenCompany?.qualifiedTicker ?? "No company selected"}</strong>
                      <span>
                        {selectedNotebookScreenEntries.length} visible note
                        {selectedNotebookScreenEntries.length === 1 ? "" : "s"}
                      </span>
                    </div>
                    <button
                      className="primary-button compact-button"
                      disabled={
                        !selectedNotebookScreenCompany ||
                        (isNotebookScreenComposerOpen &&
                          (!notebookScreenForm.title.trim() || !notebookScreenForm.body.trim()))
                      }
                      form={isNotebookScreenComposerOpen ? "notebook-screen-create-form" : undefined}
                      onClick={isNotebookScreenComposerOpen ? undefined : toggleNotebookScreenComposer}
                      type={isNotebookScreenComposerOpen ? "submit" : "button"}
                    >
                      {isNotebookScreenComposerOpen ? <Save size={15} /> : <Plus size={15} />}
                      {isNotebookScreenComposerOpen ? "Save" : "New note"}
                    </button>
                  </div>
                  <div className="filter-reset-row" aria-label="Notebook filter reset">
                    <div className="inbox-review-summary" aria-label="Notebook follow-up summary">
                      <span>
                        <strong>{selectedNotebookScreenEntries.length}</strong> visible
                      </span>
                    </div>
                    <button
                      className="secondary-button compact-button"
                      disabled={
                        notebookScreenKindFilter === "all" &&
                        notebookScreenClaimStatusFilter === "all" &&
                        notebookScreenFollowUpFilter === "all" &&
                        notebookScreenTagFilter.trim().length === 0
                      }
                      onClick={() => {
                        setNotebookScreenKindFilter("all");
                        setNotebookScreenClaimStatusFilter("all");
                        setNotebookScreenFollowUpFilter("all");
                        setNotebookScreenTagFilter("");
                      }}
                      type="button"
                    >
                      <X size={15} />
                      Clear filters
                    </button>
                  </div>
                  <div className="notebooks-filter-row" aria-label="Notebook filters">
                    <label>
                      Kind
                      <select
                        aria-label="Notebook kind filter"
                        value={notebookScreenKindFilter}
                        onChange={(event) => setNotebookScreenKindFilter(event.target.value)}
                      >
                        <option value="all">All</option>
                        <option value="manual">Manual</option>
                        <option value="observation">Observation</option>
                        <option value="claim">Claim</option>
                        <option value="question">Question</option>
                        <option value="follow_up">Follow-up</option>
                      </select>
                    </label>
                    <label>
                      Status
                      <select
                        aria-label="Notebook claim status filter"
                        value={notebookScreenClaimStatusFilter}
                        onChange={(event) => setNotebookScreenClaimStatusFilter(event.target.value)}
                      >
                        <option value="all">All</option>
                        <option value="open">Open</option>
                        <option value="delivered">Delivered</option>
                        <option value="partially_delivered">Partially delivered</option>
                        <option value="missed">Missed</option>
                        <option value="unknown">Unknown</option>
                        <option value="not_applicable">Not applicable</option>
                      </select>
                    </label>
                    <label>
                      Tag
                      <input
                        aria-label="Notebook tag filter"
                        placeholder="tag"
                        value={notebookScreenTagFilter}
                        onChange={(event) => setNotebookScreenTagFilter(event.target.value)}
                      />
                    </label>
                    <label>
                      Follow-up
                      <select
                        aria-label="Notebook follow-up filter"
                        value={notebookScreenFollowUpFilter}
                        onChange={(event) => setNotebookScreenFollowUpFilter(event.target.value)}
                      >
                        <option value="all">All</option>
                        <option value="has_follow_up">Has follow-up</option>
                        <option value="no_follow_up">No follow-up</option>
                      </select>
                    </label>
                  </div>

                  <div className="notebooks-notes-list">
                    {isNotebookScreenComposerOpen ? (
                      <form
                        id="notebook-screen-create-form"
                        className="notebook-form notebooks-create-form"
                        onSubmit={createNotebookScreenEntry}
                      >
                        <div className="notebooks-draft-header">
                          <button
                            className="minimal-button"
                            onClick={discardNotebookScreenDraft}
                            type="button"
                          >
                            <X size={12} />
                            Discard
                          </button>
                        </div>
                        <div className="notebook-form-grid">
                          <label>
                            Title
                            <input
                              aria-label="Notebook screen note title"
                              value={notebookScreenForm.title}
                              onChange={(event) => updateNotebookScreenForm("title", event.target.value)}
                            />
                          </label>
                          <label>
                            Kind
                            <select
                              aria-label="Notebook screen note kind"
                              value={notebookScreenForm.kind}
                              onChange={(event) => updateNotebookScreenForm("kind", event.target.value)}
                            >
                              <option value="manual">Manual</option>
                              <option value="observation">Observation</option>
                              <option value="claim">Claim</option>
                              <option value="question">Question</option>
                              <option value="follow_up">Follow-up</option>
                            </select>
                          </label>
                          <label>
                            Tags
                            <input
                              aria-label="Notebook screen note tags"
                              placeholder="comma, separated"
                              value={notebookScreenForm.tags}
                              onChange={(event) => updateNotebookScreenForm("tags", event.target.value)}
                            />
                          </label>
                          <label>
                            Claim status
                            <select
                              aria-label="Notebook screen note claim status"
                              value={notebookScreenForm.claimStatus}
                              onChange={(event) => updateNotebookScreenForm("claimStatus", event.target.value)}
                            >
                              <option value="">None</option>
                              <option value="open">Open</option>
                              <option value="delivered">Delivered</option>
                              <option value="partially_delivered">Partially delivered</option>
                              <option value="missed">Missed</option>
                              <option value="unknown">Unknown</option>
                              <option value="not_applicable">Not applicable</option>
                            </select>
                          </label>
                          <NotebookDateField
                            ariaLabel="Notebook screen note event date"
                            label="Event date"
                            value={notebookScreenForm.eventDate}
                            onChange={(value) => updateNotebookScreenForm("eventDate", value)}
                          />
                          <NotebookQuarterField
                            ariaLabel="Notebook screen note follow-up quarter"
                            label="Follow-up quarter"
                            value={notebookScreenForm.followUpAfter}
                            onChange={(value) => updateNotebookScreenForm("followUpAfter", value)}
                          />
                          <NotebookDateField
                            ariaLabel="Notebook screen note follow-up date"
                            label="Follow-up date"
                            value={notebookScreenForm.followUpDate}
                            onChange={(value) => updateNotebookScreenForm("followUpDate", value)}
                          />
                        </div>
                        <label className="notebook-body-field">
                          Body
                          <textarea
                            aria-label="Notebook screen note body"
                            value={notebookScreenForm.body}
                            onChange={(event) => updateNotebookScreenForm("body", event.target.value)}
                          />
                        </label>
                      </form>
                    ) : null}

                    {selectedNotebookScreenEntries.map((entry) => (
                      <div className="notebook-row-block" key={entry.id}>
                        <button
                          aria-label={`Select notebook screen entry: ${entry.title}`}
                          className={[
                            "notebook-row",
                            selectedNotebookScreenEntry?.id === entry.id
                              ? "notebook-row-selected"
                              : "",
                          ]
                            .filter(Boolean)
                            .join(" ")}
                          onClick={() => toggleNotebookScreenEntry(entry)}
                          type="button"
                        >
                          <div>
                            <div className="notebook-row-top">
                              <h3>{entry.title}</h3>
                              <span>{entry.kind.replace("_", " ")}</span>
                            </div>
                          </div>
                          <div className="notebook-row-meta">
                            {entry.claimStatus ? <span>{entry.claimStatus.replace("_", " ")}</span> : null}
                            {entry.followUpAfter ? <span>{entry.followUpAfter}</span> : null}
                            {entry.tags.slice(0, 2).map((tag) => (
                              <span key={tag}>{tag}</span>
                            ))}
                          </div>
                        </button>

                        {selectedNotebookScreenEntry?.id === entry.id ? (
                          <form
                            className="notebook-detail notebooks-inline-detail"
                            aria-label="Notebook screen entry detail"
                            onSubmit={saveNotebookScreenEntry}
                          >
                            {isNotebookScreenEditMode ? (
                          <>
                            <div className="notebook-entry-header">
                              <label>
                                Title
                                <input
                                  aria-label="Notebook screen selected title"
                                  value={notebookScreenEditForm.title}
                                  onChange={(event) =>
                                    updateNotebookScreenEditForm("title", event.target.value)
                                  }
                                />
                              </label>
                              <div className="notebook-detail-actions">
                                <button
                                  className="secondary-button compact-button"
                                  onClick={cancelNotebookScreenEdit}
                                  type="button"
                                >
                                  <X size={15} />
                                  Cancel
                                </button>
                                <button
                                  className="primary-button compact-button"
                                  disabled={
                                    !isNotebookScreenEditDirty ||
                                    !notebookScreenEditForm.title.trim() ||
                                    !notebookScreenEditForm.body.trim()
                                  }
                                  type="submit"
                                >
                                  <Save size={15} />
                                  Save
                                </button>
                              </div>
                            </div>
                            <textarea
                              aria-label="Notebook screen selected body"
                              value={notebookScreenEditForm.body}
                              onChange={(event) =>
                                updateNotebookScreenEditForm("body", event.target.value)
                              }
                            />
                            <div className="notebook-detail-grid">
                              <label>
                                Kind
                                <select
                                  aria-label="Notebook screen selected kind"
                                  value={notebookScreenEditForm.kind}
                                  onChange={(event) =>
                                    updateNotebookScreenEditForm("kind", event.target.value)
                                  }
                                >
                                  <option value="manual">Manual</option>
                                  <option value="observation">Observation</option>
                                  <option value="claim">Claim</option>
                                  <option value="question">Question</option>
                                  <option value="follow_up">Follow-up</option>
                                </select>
                              </label>
                              <label>
                                Claim status
                                <select
                                  aria-label="Notebook screen selected claim status"
                                  value={notebookScreenEditForm.claimStatus}
                                  onChange={(event) =>
                                    updateNotebookScreenEditForm("claimStatus", event.target.value)
                                  }
                                >
                                  <option value="">None</option>
                                  <option value="open">Open</option>
                                  <option value="delivered">Delivered</option>
                                  <option value="partially_delivered">Partially delivered</option>
                                  <option value="missed">Missed</option>
                                  <option value="unknown">Unknown</option>
                                  <option value="not_applicable">Not applicable</option>
                                </select>
                              </label>
                              <label>
                                Tags
                                <input
                                  aria-label="Notebook screen selected tags"
                                  value={notebookScreenEditForm.tags}
                                  onChange={(event) =>
                                    updateNotebookScreenEditForm("tags", event.target.value)
                                  }
                                />
                              </label>
                              <NotebookDateField
                                ariaLabel="Notebook screen selected event date"
                                label="Event date"
                                value={notebookScreenEditForm.eventDate}
                                onChange={(value) => updateNotebookScreenEditForm("eventDate", value)}
                              />
                              <NotebookQuarterField
                                ariaLabel="Notebook screen selected follow-up quarter"
                                label="Follow-up quarter"
                                value={notebookScreenEditForm.followUpAfter}
                                onChange={(value) => updateNotebookScreenEditForm("followUpAfter", value)}
                              />
                              <NotebookDateField
                                ariaLabel="Notebook screen selected follow-up date"
                                label="Follow-up date"
                                value={notebookScreenEditForm.followUpDate}
                                onChange={(value) => updateNotebookScreenEditForm("followUpDate", value)}
                              />
                            </div>
                          </>
                            ) : (
                          <>
                            <div className="notebook-entry-header">
                              <div>
                                <span className="eyebrow">
                                  {selectedNotebookScreenEntry.kind.replace("_", " ")}
                                </span>
                                <h3>{selectedNotebookScreenEntry.title}</h3>
                              </div>
                              <button
                                className="secondary-button compact-button"
                                onClick={() => setNotebookScreenEditMode(true)}
                                type="button"
                              >
                                <BookOpenText size={15} />
                                Edit
                              </button>
                            </div>
                            <MarkdownNoteBody
                              ariaLabel="Notebook screen selected body"
                              body={selectedNotebookScreenEntry.body}
                            />
                          </>
                            )}
                          <div
                            className="source-chip-list"
                            aria-label={`Tags for ${selectedNotebookScreenEntry.title}`}
                          >
                            {selectedNotebookScreenEntry.tags.map((tag) => (
                              <span className="membership-chip" key={tag}>
                                {tag}
                              </span>
                            ))}
                            {selectedNotebookScreenEntry.tags.length === 0 ? (
                              <span className="membership-empty">No tags</span>
                            ) : null}
                          </div>
                          <dl className="metadata-grid notebook-entry-meta">
                            <div>
                              <dt>Status</dt>
                              <dd>{selectedNotebookScreenEntry.claimStatus ?? "Not set"}</dd>
                            </div>
                            <div>
                              <dt>Follow-up quarter</dt>
                              <dd>{selectedNotebookScreenEntry.followUpAfter ?? "Not set"}</dd>
                            </div>
                            <div>
                              <dt>Follow-up date</dt>
                              <dd>{selectedNotebookScreenEntry.followUpDate ?? "Not set"}</dd>
                            </div>
                            <div>
                              <dt>Origin</dt>
                              <dd>
                                {renderNotebookOrigins(
                                  selectedNotebookScreenEntry.origins,
                                  selectedNotebookScreenEntry.companyId,
                                )}
                              </dd>
                            </div>
                          </dl>
                          </form>
                        ) : null}
                      </div>
                    ))}
                    {selectedNotebookScreenCompany && selectedNotebookScreenEntries.length === 0 ? (
                      <div className="empty-state">
                        <span>No notes for {selectedNotebookScreenCompany.qualifiedTicker} yet.</span>
                      </div>
                    ) : null}
                  </div>
                </div>
              </div>
              {notebookError ? <p className="error-text">Notebook command failed: {notebookError}</p> : null}
            </section>
          ) : null}

          {activeSection === "Events" ? (
            <section className="feed-panel" aria-labelledby="events-title">
              <div className="panel-header">
                <div>
                  <h1 id="events-title">Events</h1>
                  <p>Company calendar events across tracked companies.</p>
                </div>
                <button
                  className="secondary-button compact-button"
                  disabled={sourceRefreshState === "refreshing"}
                  onClick={() => {
                    void refreshEventSources("manual", companyEventWeekRange.start);
                  }}
                  type="button"
                >
                  {sourceRefreshState === "done" && selectedSourceAdapterId === "bankier-kalendarium-html" ? (
                    <CheckCircle2 size={15} />
                  ) : (
                    <RefreshCw size={15} />
                  )}
                  {sourceRefreshState === "refreshing" && sourceAdapterRefreshInFlight === "events"
                    ? "Refreshing"
                    : "Refresh event sources"}
                </button>
                <button className="primary-button compact-button" onClick={openCompanyEventComposer} type="button">
                  <Plus size={15} />
                  Add event
                </button>
              </div>

              <div className="filter-toolbar events-filter-toolbar" aria-label="Event view mode">
                <div className="segmented-control" role="group" aria-label="Event layout">
                  {(["week", "list"] as const).map((viewMode) => (
                    <button
                      className={companyEventViewMode === viewMode ? "segment-active" : ""}
                      key={viewMode}
                      onClick={() => {
                        setCompanyEventViewMode(viewMode);
                      }}
                      type="button"
                    >
                      {viewMode === "week" ? "Week" : "List"}
                    </button>
                  ))}
                </div>
                {companyEventViewMode === "week" ? (
                  <div className="week-toolbar" aria-label="Week navigation">
                    <button
                      className="secondary-button compact-button icon-only-button"
                      onClick={() => {
                        setCompanyEventWeekAnchorDate((current) =>
                          formatLocalDate(addLocalDays(parseLocalDate(current), -7)),
                        );
                      }}
                      type="button"
                      aria-label="Previous week"
                    >
                      <ChevronLeft size={15} />
                    </button>
                    <span>{formatWeekRange(companyEventWeekRange.start, companyEventWeekRange.end)}</span>
                    <button
                      className="secondary-button compact-button icon-only-button"
                      onClick={() => {
                        setCompanyEventWeekAnchorDate((current) =>
                          formatLocalDate(addLocalDays(parseLocalDate(current), 7)),
                        );
                      }}
                      type="button"
                      aria-label="Next week"
                    >
                      <ChevronRight size={15} />
                    </button>
                    <button
                      className="secondary-button compact-button"
                      onClick={() => setCompanyEventWeekAnchorDate(formatLocalDate(new Date()))}
                      type="button"
                    >
                      <LocateFixed size={15} />
                      Current week
                    </button>
                  </div>
                ) : (
                  <div className="segmented-control" role="group" aria-label="Event date range">
                    {(["upcoming", "historical", "all"] as const).map((mode) => (
                      <button
                        className={companyEventMode === mode ? "segment-active" : ""}
                        key={mode}
                        onClick={() => {
                          setCompanyEventMode(mode);
                        }}
                        type="button"
                      >
                        {mode === "upcoming" ? "Upcoming" : mode === "historical" ? "History" : "All"}
                      </button>
                    ))}
                  </div>
                )}
                <label>
                  Watchlist
                  <select
                    aria-label="Event watchlist filter"
                    value={companyEventWatchlistFilter}
                    onChange={(event) => setCompanyEventWatchlistFilter(event.target.value)}
                  >
                    <option value="all">All watchlists</option>
                    {watchlists.map((watchlist) => (
                      <option key={watchlist.id} value={watchlist.id}>
                        {watchlist.name}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  Company
                  <select
                    aria-label="Event company filter"
                    value={companyEventCompanyFilter}
                    onChange={(event) => setCompanyEventCompanyFilter(event.target.value)}
                  >
                    <option value="all">All companies</option>
                    {companies.map((company) => (
                      <option key={company.id} value={company.id}>
                        {company.qualifiedTicker}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  Type
                  <select
                    aria-label="Event type filter"
                    value={companyEventTypeFilter}
                    onChange={(event) => setCompanyEventTypeFilter(event.target.value)}
                  >
                    <option value="all">All types</option>
                    {companyEventTypes.map((eventType) => (
                      <option key={eventType} value={eventType}>
                        {formatCompanyEventType(eventType)}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  Status
                  <select
                    aria-label="Event status filter"
                    value={companyEventStatusFilter}
                    onChange={(event) => setCompanyEventStatusFilter(event.target.value)}
                  >
                    <option value="all">All statuses</option>
                    {companyEventStatuses.map((status) => (
                      <option key={status} value={status}>
                        {formatCompanyEventStatus(status)}
                      </option>
                    ))}
                  </select>
                </label>
                {companyEventViewMode === "list" ? (
                  <>
                    <NotebookDateField
                      ariaLabel="Event date from filter"
                      label="From"
                      value={companyEventDateFrom}
                      onChange={setCompanyEventDateFrom}
                    />
                    <NotebookDateField
                      ariaLabel="Event date to filter"
                      label="To"
                      value={companyEventDateTo}
                      onChange={setCompanyEventDateTo}
                    />
                  </>
                ) : null}
                <button
                  className="secondary-button compact-button"
                  disabled={
                    companyEventWatchlistFilter === "all" &&
                    companyEventCompanyFilter === "all" &&
                    companyEventTypeFilter === "all" &&
                    companyEventStatusFilter === "all" &&
                    companyEventDateFrom.trim().length === 0 &&
                    companyEventDateTo.trim().length === 0
                  }
                  onClick={clearCompanyEventFilters}
                  type="button"
                >
                  <X size={15} />
                  Clear filters
                </button>
              </div>

              {isCompanyEventComposerOpen ? (
                <div className="event-composer" aria-label="Create manual event">
                  <div className="event-composer-header">
                    <div>
                      <h2>Manual event</h2>
                      <p>Add a missing date for one tracked company.</p>
                    </div>
                    <button
                      className="secondary-button compact-button"
                      onClick={() => {
                        setCompanyEventComposerOpen(false);
                        setCompanyEventCreateError(null);
                      }}
                      type="button"
                    >
                      <X size={15} />
                      Discard
                    </button>
                  </div>
                  <div className="event-composer-grid">
                    <label>
                      Company
                      <select
                        aria-label="Manual event company"
                        value={companyEventForm.companyId}
                        onChange={(event) =>
                          setCompanyEventForm((current) => ({
                            ...current,
                            companyId: event.target.value,
                          }))
                        }
                      >
                        <option value="">Select company</option>
                        {companies.map((company) => (
                          <option key={company.id} value={company.id}>
                            {company.qualifiedTicker}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label>
                      Type
                      <select
                        aria-label="Manual event type"
                        value={companyEventForm.eventType}
                        onChange={(event) =>
                          setCompanyEventForm((current) => ({
                            ...current,
                            eventType: event.target.value,
                          }))
                        }
                      >
                        {companyEventTypeOptions.map((eventType) => (
                          <option key={eventType} value={eventType}>
                            {formatCompanyEventType(eventType)}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label>
                      Status
                      <select
                        aria-label="Manual event status"
                        value={companyEventForm.status}
                        onChange={(event) =>
                          setCompanyEventForm((current) => ({
                            ...current,
                            status: event.target.value,
                          }))
                        }
                      >
                        {companyEventStatusOptions.map((status) => (
                          <option key={status} value={status}>
                            {formatCompanyEventStatus(status)}
                          </option>
                        ))}
                      </select>
                    </label>
                    <NotebookDateField
                      ariaLabel="Manual event date"
                      label="Date"
                      value={companyEventForm.eventDate}
                      onChange={(value) =>
                        setCompanyEventForm((current) => ({
                          ...current,
                          eventDate: value,
                        }))
                      }
                    />
                    <label>
                      Time
                      <input
                        aria-label="Manual event time"
                        type="time"
                        value={companyEventForm.eventTime}
                        onChange={(event) =>
                          setCompanyEventForm((current) => ({
                            ...current,
                            eventTime: event.target.value,
                          }))
                        }
                      />
                    </label>
                    <label className="event-composer-title">
                      Title
                      <input
                        aria-label="Manual event title"
                        value={companyEventForm.title}
                        onChange={(event) =>
                          setCompanyEventForm((current) => ({
                            ...current,
                            title: event.target.value,
                          }))
                        }
                      />
                    </label>
                  </div>
                  <div className="event-composer-actions">
                    {companyEventCreateError ? (
                      <p className="error-text">{companyEventCreateError}</p>
                    ) : null}
                    <button className="primary-button compact-button" onClick={createCompanyEvent} type="button">
                      <Save size={15} />
                      Save
                    </button>
                  </div>
                </div>
              ) : null}

              <div className="events-layout" aria-label="Company events">
                {companyEventViewMode === "week" ? (
                  <div className="event-week-grid" aria-label="Working week events">
                    {companyEventWorkingWeekDays.map((day) => {
                        const dayEvents = companyEventsByDate[day.date] ?? [];

                        return (
                          <section
                            className="event-week-day"
                            key={day.date}
                            aria-label={`${day.label} ${day.date}`}
                          >
                            <div className="event-week-day-header">
                              <strong>{day.label}</strong>
                              <span>{day.date}</span>
                            </div>
                            <div className="event-week-day-body">
                              {dayEvents.length > 0 ? (
                                dayEvents.map(renderCompanyEventWeekCard)
                              ) : (
                                <div className="event-week-empty">No events</div>
                              )}
                            </div>
                          </section>
                        );
                      })}
                    {companyEventWeekendEvents.length > 0 ? (
                      <section className="event-weekend-row" aria-label="Weekend events">
                        <div className="event-week-day-header">
                          <strong>Weekend</strong>
                          <span>
                            {companyEventWeekendDays[0]?.date} - {companyEventWeekendDays[1]?.date}
                          </span>
                        </div>
                        <div className="event-weekend-list">
                          {companyEventWeekendEvents.map(renderCompanyEventWeekCard)}
                        </div>
                      </section>
                    ) : null}
                  </div>
                ) : (
                  companyEvents.map(renderCompanyEventRow)
                )}
                {companyEvents.length === 0 && companyEventViewMode === "list" ? (
                  <div className="empty-state">
                    <span>
                      No{" "}
                      {companyEventMode === "upcoming" ? "upcoming events" : "events"}
                      .
                    </span>
                  </div>
                ) : null}
                {companyEventsError ? (
                  <p className="error-text">Events command failed: {companyEventsError}</p>
                ) : null}
              </div>
            </section>
          ) : null}

          {activeSection === "Transcripts" ? (
            <section className="feed-panel transcripts-panel" aria-labelledby="transcripts-title">
              <div className="panel-header">
                <div>
                  <h1 id="transcripts-title">Transcripts</h1>
                  <p>Submit a YouTube URL. Link a company only when selected transcript segments should become company notes.</p>
                </div>
                <button
                  className="secondary-button compact-button"
                  onClick={() => {
                    void refreshTranscriptJobs();
                  }}
                  type="button"
                >
                  <RefreshCw size={15} />
                  Refresh jobs
                </button>
              </div>

              <dl className="transcript-runtime-strip" aria-label="Transcript runtime settings">
                <div>
                  <dt>Provider</dt>
                  <dd>{formatAiProvider(settings?.aiProviders.youtubeTranscriptionProvider)}</dd>
                </div>
                <div>
                  <dt>Credentials</dt>
                  <dd>{formatCredentialConfigured(geminiCredentialStatus)}</dd>
                </div>
                <div>
                  <dt>Storage</dt>
                  <dd>{formatCredentialStorage(geminiCredentialStatus?.storage)}</dd>
                </div>
                <div>
                  <dt>Timeout</dt>
                  <dd>{settings?.aiProviders.youtubeTranscriptionTimeoutSeconds ?? 300}s</dd>
                </div>
              </dl>

              <form className="event-composer" onSubmit={createTranscriptJob} aria-label="Create transcript job">
                <div className="event-composer-header">
                  <div>
                    <h2>New transcript job</h2>
                    <p>URL is required. Description and company are optional.</p>
                  </div>
                  <button
                    className="primary-button compact-button"
                    disabled={transcriptJobCreateState === "refreshing" || !transcriptJobForm.url.trim()}
                    title={transcriptJobForm.url.trim() ? "Create transcript job" : "URL is required"}
                    type="submit"
                  >
                    {transcriptJobCreateState === "done" ? <CheckCircle2 size={15} /> : <Plus size={15} />}
                    {transcriptJobCreateState === "refreshing" ? "Creating" : "Create job"}
                  </button>
                </div>
                <div className="event-composer-grid">
                  <label className="event-composer-title">
                    URL
                    <input
                      aria-label="Transcript URL"
                      placeholder="https://www.youtube.com/watch?v=..."
                      value={transcriptJobForm.url}
                      onChange={(event) =>
                        setTranscriptJobForm((current) => ({
                          ...current,
                          url: event.target.value,
                        }))
                      }
                      onBlur={() => {
                        if (transcriptJobForm.url.trim()) {
                          setTranscriptJobCreateError(transcriptUrlValidationMessage(transcriptJobForm.url));
                        }
                      }}
                    />
                  </label>
                  <label className="event-composer-title">
                    Description
                    <input
                      aria-label="Transcript description"
                      placeholder="Optional, e.g. CDR Q2 investor conference"
                      value={transcriptJobForm.label}
                      onChange={(event) =>
                        setTranscriptJobForm((current) => ({
                          ...current,
                          label: event.target.value,
                        }))
                      }
                    />
                  </label>
                  <label className="event-composer-title">
                    Company or ticker
                    <input
                      aria-label="Transcript company lookup"
                      placeholder="Optional, e.g. GPW:CDR, CDR, CD PROJEKT"
                      value={transcriptJobForm.companyQuery}
                      onChange={(event) =>
                        setTranscriptJobForm((current) => ({
                          ...current,
                          companyId: "",
                          companyQuery: event.target.value,
                        }))
                      }
                    />
                  </label>
                </div>
                {transcriptJobForm.companyQuery || transcriptJobForm.companyId ? (
                  <div className="company-registry-suggestions" aria-label="Transcript company suggestions">
                    {transcriptCompanySuggestions.length > 0 ? (
                      transcriptCompanySuggestions.map((company) => (
                        <div key={company.id}>
                          <button
                            className={
                              transcriptJobForm.companyId === company.id
                                ? "company-registry-suggestion company-registry-suggestion-selected"
                                : "company-registry-suggestion"
                            }
                            onClick={() => selectTranscriptCompany(company)}
                            type="button"
                          >
                            <strong>{company.qualifiedTicker}</strong>
                            <span>{company.displayName}</span>
                            {company.isin ? <small>{company.isin}</small> : null}
                          </button>
                        </div>
                      ))
                    ) : (
                      <span>No tracked company matches. Leave company empty to keep this transcript unlinked.</span>
                    )}
                  </div>
                ) : null}
                {transcriptJobCreateError ? <p className="error-text">{transcriptJobCreateError}</p> : null}
              </form>

              <div className="sources-layout" aria-label="Transcript jobs">
                {transcriptJobsError ? (
                  <p className="error-text">Transcript jobs unavailable: {transcriptJobsError}</p>
                ) : null}
                {transcriptJobs.length === 0 && !transcriptJobsError ? (
                  <div className="empty-state">
                    <span>No transcript jobs yet.</span>
                  </div>
                ) : null}
                {transcriptJobs.map((job) => {
                  const transcriptSegments = transcriptSegmentsByJobId[job.id] ?? [];
                  const transcriptSegmentSearch = transcriptSegmentSearchByJobId[job.id] ?? "";
                  const filteredTranscriptSegments = transcriptSegments.filter((segment) =>
                    transcriptSegmentMatchesQuery(segment, transcriptSegmentSearch),
                  );
                  const selectedTranscriptSegmentIds = selectedTranscriptSegmentIdsByJobId[job.id] ?? [];
                  const selectedTranscriptSegments = transcriptSegments.filter((segment) =>
                    selectedTranscriptSegmentIds.includes(segment.id),
                  );
                  const transcriptSegmentsError = transcriptSegmentsErrorByJobId[job.id];
                  const transcriptNoteError = transcriptNoteErrorByJobId[job.id];
                  const transcriptLinkQuery = transcriptLinkQueryByJobId[job.id] ?? "";
                  const transcriptLinkError = transcriptLinkErrorByJobId[job.id];
                  const transcriptLinkSuggestions = transcriptLinkQuery.trim()
                    ? companies
                        .filter((company) => {
                          const query = transcriptLinkQuery.trim().toLowerCase();

                          return (
                            company.qualifiedTicker.toLowerCase().includes(query) ||
                            company.ticker.toLowerCase().includes(query) ||
                            company.displayName.toLowerCase().includes(query) ||
                            (company.isin?.toLowerCase().includes(query) ?? false)
                          );
                        })
                        .slice(0, 6)
                    : [];
                  const isTranscriptJobSelected = selectedTranscriptJobId === job.id;
                  const isTranscriptNoteDraftOpen = transcriptNoteDraftJobId === job.id;
                  const transcriptDescriptionDraft =
                    transcriptDescriptionDraftByJobId[job.id] ?? job.sourceLabel ?? "";
                  const transcriptDescriptionError = transcriptDescriptionErrorByJobId[job.id];
                  const isTranscriptDescriptionDirty = transcriptDescriptionDraft !== (job.sourceLabel ?? "");

                  return (
                  <div className="source-row-block" key={job.id}>
                    <article
                      className={[
                        "source-row",
                        "transcript-row",
                        isTranscriptJobSelected ? "source-row-selected" : "",
                      ]
                        .filter(Boolean)
                        .join(" ")}
                      aria-label={`Open transcript job: ${job.sourceUrl}`}
                      onClick={() => toggleTranscriptJob(job)}
                      onKeyDown={(event) => toggleTranscriptJobFromKeyboard(event, job)}
                      role="button"
                      tabIndex={0}
                    >
                      <div className="source-row-main">
                        <div className="source-title-line">
                          <span
                            className={
                              job.status === "failed"
                                ? "status-dot status-danger"
                                : job.status === "completed"
                                  ? "status-dot status-ok"
                                  : "status-dot status-warn"
                            }
                            title={formatTranscriptStatus(job.status)}
                          />
                          <h2>{job.sourceLabel ?? "Untitled transcript"}</h2>
                        </div>
                        <p>
                          {job.sourceUrl} · {job.company ?? "Unlinked transcript"} · {formatAiProvider(job.providerId)} ·{" "}
                          {formatGeminiModel(settings?.aiProviders.youtubeTranscriptionModel)}
                        </p>
                        <div className="source-chip-list" aria-label={`Transcript job metadata for ${job.id}`}>
                          <span className="membership-chip">{formatTranscriptStatus(job.companyResolutionStatus)}</span>
                          <span className="membership-chip">{job.sourceType}</span>
                          {job.errorCode ? <span className="membership-chip">{formatEnumLabel(job.errorCode)}</span> : null}
                        </div>
                      </div>
                      <div className="source-row-status">
                        <span>{formatTranscriptStatus(job.status)}</span>
                        {job.status === "queued" || job.status === "failed" ? (
                          <button
                            className="secondary-button compact-button"
                            disabled={transcriptJobRunInFlight === job.id || !geminiCredentialStatus?.configured}
                            onClick={(event) => {
                              event.stopPropagation();
                              runTranscriptJob(job.id);
                            }}
                            title={
                              geminiCredentialStatus?.configured
                                ? "Retry Gemini transcription"
                                : "Configure Gemini API key in Settings before running transcription"
                            }
                            type="button"
                          >
                            <RefreshCw size={15} />
                            {transcriptJobRunInFlight === job.id ? "Running" : "Retry"}
                          </button>
                        ) : null}
                        <button
                          className="icon-button danger-button"
                          disabled={transcriptDeleteInFlight === job.id}
                          onClick={(event) => {
                            event.stopPropagation();
                            deleteTranscriptJob(job);
                          }}
                          title={`Delete transcript job ${job.sourceLabel ?? job.sourceUrl}`}
                          type="button"
                        >
                          <Trash2 size={15} />
                        </button>
                      </div>
                    </article>
                      {isTranscriptJobSelected ? (
                      <div className="transcript-detail-panel" aria-label="Transcript job details">
                        <div className="transcript-description-editor" aria-label="Transcript description editor">
                          <label>
                            Description
                            <input
                              aria-label="Edit transcript description"
                              placeholder="Optional, e.g. CDR Q2 investor conference"
                              value={transcriptDescriptionDraft}
                              onChange={(event) =>
                                setTranscriptDescriptionDraftByJobId((current) => ({
                                  ...current,
                                  [job.id]: event.target.value,
                                }))
                              }
                            />
                          </label>
                          <button
                            className="secondary-button compact-button"
                            disabled={
                              transcriptDescriptionSaveInFlight === job.id ||
                              !isTranscriptDescriptionDirty
                            }
                            onClick={() => updateTranscriptJobDescription(job)}
                            type="button"
                          >
                            <Save size={15} />
                            Save
                          </button>
                          {transcriptDescriptionError ? (
                            <p className="error-text">{transcriptDescriptionError}</p>
                          ) : null}
                        </div>
                        <dl className="source-status-grid source-status-detail">
                          <div>
                            <dt>Company</dt>
                            <dd>{job.company ?? "Unlinked"}</dd>
                          </div>
                          <div>
                            <dt>Status</dt>
                            <dd>{formatTranscriptStatus(job.status)}</dd>
                          </div>
                          <div>
                            <dt>Source</dt>
                            <dd>{job.sourceUrl}</dd>
                          </div>
                          <div>
                            <dt>Selected</dt>
                            <dd>{selectedTranscriptSegmentIds.length}</dd>
                          </div>
                        </dl>
                        {job.status === "failed" ? (
                          <div className="transcript-error-panel" aria-label="Transcript job error">
                            <strong>{job.errorCode ? formatEnumLabel(job.errorCode) : "Transcript job failed"}</strong>
                            <p>{job.error ?? "No detailed provider error was stored."}</p>
                          </div>
                        ) : null}
                        {!job.companyId ? (
                          <div className="transcript-link-panel" aria-label="Link transcript company">
                            <div>
                              <strong>Optional company link</strong>
                              <p className="muted-text">
                                Keep this transcript unlinked, or link it when selected segments should become a company notebook note.
                              </p>
                            </div>
                            <label>
                              Company or ticker
                              <input
                                aria-label="Transcript link company lookup"
                                placeholder="GPW:CDR, CDR, CD PROJEKT"
                                value={transcriptLinkQuery}
                                onChange={(event) => updateTranscriptLinkQuery(job.id, event.target.value)}
                              />
                            </label>
                            {transcriptLinkQuery ? (
                              <div
                                className="company-registry-suggestions"
                                aria-label="Transcript link company suggestions"
                              >
                                {transcriptLinkSuggestions.length > 0 ? (
                                  transcriptLinkSuggestions.map((company) => (
                                    <div key={company.id}>
                                      <button
                                        className="company-registry-suggestion"
                                        disabled={transcriptLinkInFlight === job.id}
                                        onClick={() => linkTranscriptJobCompany(job.id, company)}
                                        type="button"
                                      >
                                        <strong>{company.qualifiedTicker}</strong>
                                        <span>{company.displayName}</span>
                                        {company.isin ? <small>{company.isin}</small> : null}
                                      </button>
                                    </div>
                                  ))
                                ) : (
                                  <span>No tracked company matches. The transcript can stay unlinked.</span>
                                )}
                              </div>
                            ) : null}
                            {transcriptLinkError ? <p className="error-text">{transcriptLinkError}</p> : null}
                          </div>
                        ) : null}
                        {job.status !== "completed" ? (
                          <p className="muted-text">Transcript segments will be available after the job completes.</p>
                        ) : null}
                        {transcriptSegmentsError ? (
                          <p className="error-text">Transcript segments unavailable: {transcriptSegmentsError}</p>
                        ) : null}
                        {job.status === "completed" && transcriptSegments.length === 0 && !transcriptSegmentsError ? (
                          <div className="empty-state">
                            <span>No transcript segments stored for this job.</span>
                          </div>
                        ) : null}
                        {transcriptSegments.length > 0 ? (
                          <div className="transcript-search-panel">
                            <label>
                              Search transcript
                              <span className="transcript-search-input-row">
                                <input
                                  aria-label="Search transcript segments"
                                  placeholder="Search text, speaker, language, timestamp"
                                  value={transcriptSegmentSearch}
                                  onChange={(event) =>
                                    setTranscriptSegmentSearchByJobId((current) => ({
                                      ...current,
                                      [job.id]: event.target.value,
                                    }))
                                  }
                                />
                                {transcriptSegmentSearch ? (
                                  <button
                                    aria-label="Clear transcript search"
                                    className="icon-button transcript-search-clear"
                                    onClick={() =>
                                      setTranscriptSegmentSearchByJobId((current) => ({
                                        ...current,
                                        [job.id]: "",
                                      }))
                                    }
                                    title="Clear transcript search"
                                    type="button"
                                  >
                                    <X size={13} />
                                  </button>
                                ) : null}
                              </span>
                            </label>
                            <span className="transcript-search-count">
                              {filteredTranscriptSegments.length}/{transcriptSegments.length}
                            </span>
                          </div>
                        ) : null}
                        {transcriptSegments.length > 0 ? (
                          <div className="transcript-segment-list" aria-label="Transcript segments">
                            {filteredTranscriptSegments.length > 0 ? (
                              filteredTranscriptSegments.map((segment) => (
                                <label className="transcript-segment-row" key={segment.id}>
                                  <input
                                    aria-label={`Select transcript segment ${transcriptSegmentTimestamp(segment)}`}
                                    checked={selectedTranscriptSegmentIds.includes(segment.id)}
                                    onChange={() => toggleTranscriptSegment(job.id, segment.id)}
                                    type="checkbox"
                                  />
                                  <span className="transcript-segment-time">
                                    {transcriptSegmentTimestamp(segment)}
                                  </span>
                                  <span className="transcript-segment-text">
                                    {highlightSearchMatch(segment.text, transcriptSegmentSearch)}
                                  </span>
                                </label>
                              ))
                            ) : (
                              <div className="empty-state">
                                <span>No transcript segments match this search.</span>
                              </div>
                            )}
                          </div>
                        ) : null}
                        {job.status === "completed" && transcriptSegments.length > 0 ? (
                          <div className="transcript-note-actions">
                            <button
                              className="primary-button compact-button"
                              disabled={!job.companyId || selectedTranscriptSegmentIds.length === 0}
                              onClick={() => openTranscriptNoteDraft(job, selectedTranscriptSegments)}
                              title={
                                job.companyId
                                  ? "Create a company notebook draft from selected segments"
                                  : "Link a company before saving selected segments as a company notebook note"
                              }
                              type="button"
                            >
                              <Plus size={15} />
                              Create company note draft
                            </button>
                            {transcriptNoteError ? <p className="error-text">{transcriptNoteError}</p> : null}
                          </div>
                        ) : null}
                        {isTranscriptNoteDraftOpen ? (
                          <form
                            className="transcript-note-draft"
                            onSubmit={(event) => createTranscriptNotebookEntry(job, event)}
                            aria-label="Transcript note draft"
                          >
                            <div className="event-composer-header">
                              <div>
                                <h2>Notebook note draft</h2>
                                <p>Edit the note before saving it to the company notebook.</p>
                              </div>
                              <button
                                className="secondary-button compact-button"
                                onClick={discardTranscriptNoteDraft}
                                type="button"
                              >
                                <X size={15} />
                                Discard
                              </button>
                            </div>
                            <div className="event-composer-grid">
                              <label className="event-composer-title">
                                Title
                                <input
                                  aria-label="Transcript note title"
                                  value={transcriptNoteForm.title}
                                  onChange={(event) => updateTranscriptNoteForm("title", event.target.value)}
                                />
                              </label>
                              <label>
                                Kind
                                <select
                                  aria-label="Transcript note kind"
                                  value={transcriptNoteForm.kind}
                                  onChange={(event) => updateTranscriptNoteForm("kind", event.target.value)}
                                >
                                  <option value="manual">Manual</option>
                                  <option value="observation">Observation</option>
                                  <option value="claim">Claim</option>
                                  <option value="question">Question</option>
                                  <option value="follow_up">Follow-up</option>
                                </select>
                              </label>
                              <label>
                                Status
                                <select
                                  aria-label="Transcript note status"
                                  value={transcriptNoteForm.claimStatus}
                                  onChange={(event) => updateTranscriptNoteForm("claimStatus", event.target.value)}
                                >
                                  <option value="">Not set</option>
                                  <option value="open">Open</option>
                                  <option value="delivered">Delivered</option>
                                  <option value="partially_delivered">Partially delivered</option>
                                  <option value="missed">Missed</option>
                                  <option value="unknown">Unknown</option>
                                  <option value="not_applicable">Not applicable</option>
                                </select>
                              </label>
                              <label>
                                Tags
                                <input
                                  aria-label="Transcript note tags"
                                  value={transcriptNoteForm.tags}
                                  onChange={(event) => updateTranscriptNoteForm("tags", event.target.value)}
                                />
                              </label>
                              <NotebookDateField
                                ariaLabel="Transcript note event date"
                                label="Event date"
                                value={transcriptNoteForm.eventDate}
                                onChange={(value) => updateTranscriptNoteForm("eventDate", value)}
                              />
                              <NotebookQuarterField
                                ariaLabel="Transcript note follow-up quarter"
                                label="Follow-up quarter"
                                value={transcriptNoteForm.followUpAfter}
                                onChange={(value) => updateTranscriptNoteForm("followUpAfter", value)}
                              />
                              <NotebookDateField
                                ariaLabel="Transcript note follow-up date"
                                label="Follow-up date"
                                value={transcriptNoteForm.followUpDate}
                                onChange={(value) => updateTranscriptNoteForm("followUpDate", value)}
                              />
                            </div>
                            <label className="notebook-body-field">
                              Body
                              <textarea
                                aria-label="Transcript note body"
                                value={transcriptNoteForm.body}
                                onChange={(event) => updateTranscriptNoteForm("body", event.target.value)}
                              />
                            </label>
                            <div className="event-composer-actions">
                              <button
                                className="primary-button compact-button"
                                disabled={transcriptNoteSaveInFlight === job.id}
                                type="submit"
                              >
                                <Save size={15} />
                                {transcriptNoteSaveInFlight === job.id ? "Saving" : "Save"}
                              </button>
                            </div>
                          </form>
                        ) : null}
                      </div>
                    ) : null}
                  </div>
                  );
                })}
              </div>
            </section>
          ) : null}

          {activeSection === "Sources" ? (
            <section className="feed-panel" aria-labelledby="sources-title">
              <div className="panel-header">
                <div>
                  <h1 id="sources-title">Sources</h1>
                  <p>Local source adapter status before remote ingestion is wired.</p>
                </div>
                <button
                  className="secondary-button compact-button"
                  disabled={sourceRefreshState === "refreshing"}
                  onClick={() => {
                    void refreshSources("manual");
                  }}
                  type="button"
                >
                  {sourceRefreshState === "done" ? <CheckCircle2 size={15} /> : <RefreshCw size={15} />}
                  {sourceRefreshState === "refreshing" ? "Refreshing" : "Refresh sources"}
                </button>
              </div>

              <div className="sources-layout" aria-label="Source adapters">
                {sourceRefreshResult ? (
                  <dl className="source-status-grid source-refresh-summary" aria-label="Last source refresh summary">
                    <div>
                      <dt>Fetched</dt>
                      <dd aria-label="Fetched source items">{sourceRefreshResult.itemsFetched}</dd>
                    </div>
                    <div>
                      <dt>Created</dt>
                      <dd aria-label="Created source items">{sourceRefreshResult.itemsCreated}</dd>
                    </div>
                    <div>
                      <dt>Matched</dt>
                      <dd aria-label="Matched source items">{sourceRefreshResult.itemsMatched}</dd>
                    </div>
                    <div>
                      <dt>Unmatched</dt>
                      <dd aria-label="Unmatched source items">{sourceRefreshResult.itemsUnmatched}</dd>
                    </div>
                    <div>
                      <dt>Details</dt>
                      <dd aria-label="Stored source detail bodies">
                        {sourceRefreshResult.detailItemsStored}/{sourceRefreshResult.detailItemsAttempted}
                      </dd>
                    </div>
                    <div>
                      <dt>Detail failures</dt>
                      <dd aria-label="Failed source detail bodies">{sourceRefreshResult.detailItemsFailed}</dd>
                    </div>
                  </dl>
                ) : null}
                {sourceAdapters.map((adapter) => (
                  <div className="source-row-block" key={adapter.id}>
                    <article
                      aria-label={`Open source adapter: ${adapter.displayName}`}
                      className={[
                        "source-row",
                        selectedSourceAdapterId === adapter.id ? "source-row-selected" : "",
                      ]
                        .filter(Boolean)
                        .join(" ")}
                      onClick={() => toggleSourceAdapter(adapter.id)}
                      onKeyDown={(event) => toggleSourceAdapterFromKeyboard(event, adapter.id)}
                      role="button"
                      tabIndex={0}
                      title={`Open ${adapter.displayName} details`}
                    >
                      <div className="source-row-main">
                        <div className="source-title-line">
                          <span
                            className={adapter.enabled ? "status-dot status-ok" : "status-dot status-warn"}
                            title={adapter.enabled ? "Enabled" : "Disabled"}
                          />
                          <h2>{adapter.displayName}</h2>
                          <span className="source-id">{adapter.id}</span>
                        </div>
                        <p>
                          {formatSourceSubtitle(adapter)}
                        </p>
                        <div className="source-chip-list" aria-label={`Markets for ${adapter.displayName}`}>
                          {adapter.markets.map((market) => (
                            <span className="membership-chip" key={market}>
                              {market}
                            </span>
                          ))}
                          {adapter.markets.length === 0 ? (
                            <span className="membership-empty">No markets</span>
                          ) : null}
                        </div>
                      </div>
                      <div className="source-row-status">
                        <span>{adapter.lastError ?? (adapter.enabled ? "Ready" : "Disabled")}</span>
                      </div>
                    </article>
                    {selectedSourceAdapterId === adapter.id ? (
                      <div className="source-detail-panel" aria-label="Source adapter details">
                        <div className="source-detail-actions">
                          <button
                            className="secondary-button compact-button"
                            disabled={
                              !adapter.enabled ||
                              sourceRefreshState === "refreshing" ||
                              (adapter.id === gpwRegistryAdapterId
                                ? registryRefreshState === "refreshing"
                                : sourceAdapterRefreshInFlight !== null)
                            }
                            onClick={() => {
                              void refreshSingleSource(adapter, "manual");
                            }}
                            title={
                              adapter.enabled
                                ? `Refresh ${adapter.displayName}`
                                : `${adapter.displayName} is disabled`
                            }
                            type="button"
                          >
                            {adapter.id === gpwRegistryAdapterId && registryRefreshState === "done" ? (
                              <CheckCircle2 size={15} />
                            ) : (
                              <RefreshCw size={15} />
                            )}
                            {adapter.id === gpwRegistryAdapterId
                              ? registryRefreshState === "refreshing"
                                ? "Refreshing"
                                : "Refresh source"
                              : sourceAdapterRefreshInFlight === adapter.id
                                ? "Refreshing"
                                : "Refresh source"}
                          </button>
                          {sourceRefreshError && sourceAdapterRefreshInFlight === null ? (
                            <span className="error-text">Source refresh failed: {sourceRefreshError}</span>
                          ) : null}
                        </div>
                        <dl className="source-status-grid source-status-detail">
                          <div>
                            <dt>Scheduler</dt>
                            <dd>{formatSourceScheduler(adapter)}</dd>
                          </div>
                          <div>
                            <dt>Next poll</dt>
                            <dd>{formatNextRefresh(adapter)}</dd>
                          </div>
                          <div>
                            <dt>Last attempt</dt>
                            <dd>{formatTimestamp(adapter.lastAttemptAt, "Never")}</dd>
                          </div>
                          <div>
                            <dt>Last trigger</dt>
                            <dd>{formatSourceTrigger(adapter)}</dd>
                          </div>
                          <div>
                            <dt>Last success</dt>
                            <dd>{formatTimestamp(adapter.lastSuccessAt, "Never")}</dd>
                          </div>
                          <div>
                            <dt>Last error</dt>
                            <dd>{formatTimestamp(adapter.lastErrorAt, "None")}</dd>
                          </div>
                          <div>
                            <dt>{sourceLastResultLabel(adapter)}</dt>
                            <dd>{formatSourceLastResult(adapter)}</dd>
                          </div>
                          {adapter.id === gpwRegistryAdapterId ? null : (
                            <div>
                              <dt>Detail warning</dt>
                              <dd>{adapter.lastDetailWarning ?? "None"}</dd>
                            </div>
                          )}
                          <div>
                            <dt>Status</dt>
                            <dd>{adapter.lastError ?? (adapter.enabled ? "Ready" : "Disabled")}</dd>
                          </div>
                          <div>
                            <dt>Access</dt>
                            <dd>{formatSourceAccess(adapter)}</dd>
                          </div>
                          <div>
                            <dt>{sourcePolicyLabel(adapter)}</dt>
                            <dd>{adapter.rateLimitPolicy}</dd>
                          </div>
                          <div>
                            <dt>Source page</dt>
                            <dd>
                              <button
                                aria-label={`Open source page for ${adapter.displayName}`}
                                className="source-page-link"
                                onClick={() => openExternalUrl(adapter.sourceUrl)}
                                type="button"
                              >
                                <ExternalLink size={14} />
                                Open source page
                              </button>
                            </dd>
                          </div>
                          <div>
                            <dt>Policy</dt>
                            <dd>{adapter.policyNote}</dd>
                          </div>
                        </dl>
                        {adapter.id === "gpw-company-registry" ? (
                          <>
                            <div className="source-registry-actions" aria-label="Company registry refresh">
                              <button
                                className="secondary-button compact-button"
                                disabled={registryRefreshState === "refreshing"}
                                onClick={() => {
                                  void refreshCompanyRegistry("manual");
                                }}
                                type="button"
                              >
                                {registryRefreshState === "done" ? <CheckCircle2 size={15} /> : <RefreshCw size={15} />}
                                {registryRefreshState === "refreshing" ? "Refreshing" : "Refresh registry"}
                              </button>
                              {registryRefreshResult ? (
                                <span>
                                  {registryRefreshResult.entriesUpserted}/{registryRefreshResult.entriesFetched} cached
                                </span>
                              ) : null}
                              {registryRefreshError ? (
                                <span className="error-text">Registry refresh failed: {registryRefreshError}</span>
                              ) : null}
                            </div>
                            <div className="source-collapsible-panel" aria-label="GPW company registry entries">
                              <button
                                aria-expanded={isCompanyRegistryListExpanded}
                                className="source-collapsible-header"
                                onClick={toggleCompanyRegistryList}
                                type="button"
                              >
                                <span>Companies</span>
                                <span className="source-collapsible-header-meta">
                                  <strong>{companyRegistryEntries.length}</strong>
                                  <ChevronDown
                                    className={isCompanyRegistryListExpanded ? "chevron-open" : ""}
                                    size={15}
                                  />
                                </span>
                              </button>
                              {isCompanyRegistryListExpanded ? (
                                <div className="source-registry-list">
                                  <label className="registry-search-field">
                                    <Search size={15} />
                                    <input
                                      aria-label="Search GPW company registry"
                                      onChange={(event) => setCompanyRegistrySearch(event.target.value)}
                                      placeholder="Search ticker, company, ISIN"
                                      type="search"
                                      value={companyRegistrySearch}
                                    />
                                  </label>
                                  <span className="source-registry-count">
                                    {filteredCompanyRegistryEntries.length}/{companyRegistryEntries.length} companies
                                  </span>
                                  {filteredCompanyRegistryEntries.map((entry) => (
                                    <div className="source-registry-row" key={entry.qualifiedTicker}>
                                      <span>{entry.qualifiedTicker}</span>
                                      <strong title={entry.displayName}>{entry.displayName}</strong>
                                      <small>{entry.isin ?? "No ISIN"}</small>
                                      <button
                                        className="secondary-button compact-button"
                                        disabled={entry.tracked || addingRegistryTicker === entry.qualifiedTicker}
                                        onClick={() => addCompanyFromRegistry(entry)}
                                        title={entry.tracked ? `${entry.qualifiedTicker} already added` : `Add ${entry.qualifiedTicker}`}
                                        type="button"
                                      >
                                        {entry.tracked ? <CheckCircle2 size={14} /> : <Plus size={14} />}
                                        {entry.tracked ? "Added" : "Add"}
                                      </button>
                                    </div>
                                  ))}
                                  {companyRegistryEntries.length === 0 ? (
                                    <span className="membership-empty">
                                      No cached companies yet. Refresh registry first.
                                    </span>
                                  ) : null}
                                  {companyRegistryEntries.length > 0 && filteredCompanyRegistryEntries.length === 0 ? (
                                    <span className="membership-empty">No registry companies match this search.</span>
                                  ) : null}
                                  {companyRegistryEntriesError ? (
                                    <span className="error-text">
                                      Company registry list failed: {companyRegistryEntriesError}
                                    </span>
                                  ) : null}
                                </div>
                              ) : null}
                            </div>
                          </>
                        ) : (
                          <div className="source-collapsible-panel" aria-label="Unmatched source item diagnostics">
                            <button
                              aria-expanded={Boolean(expandedUnmatchedAdapters[adapter.id])}
                              className="source-collapsible-header"
                              onClick={() => toggleUnmatchedSourceItems(adapter.id)}
                              type="button"
                            >
                              <span>Unmatched</span>
                              <span className="source-collapsible-header-meta">
                                <strong>{unmatchedSourceItems[adapter.id]?.length ?? 0}</strong>
                                <ChevronDown
                                  className={expandedUnmatchedAdapters[adapter.id] ? "chevron-open" : ""}
                                  size={15}
                                />
                              </span>
                            </button>
                            {expandedUnmatchedAdapters[adapter.id] ? (
                              <div className="source-unmatched-list">
                                {(unmatchedSourceItems[adapter.id] ?? []).map((item) => (
                                  <a
                                    className="source-unmatched-row"
                                    href={item.sourceUrl}
                                    key={item.id}
                                    rel="noreferrer"
                                    target="_blank"
                                    title={item.title}
                                  >
                                    <span>{item.companyName}</span>
                                    <strong>{item.title}</strong>
                                    <small>{formatTimestamp(item.publishedAt || item.fetchedAt, "Unknown")}</small>
                                  </a>
                                ))}
                                {(unmatchedSourceItems[adapter.id] ?? []).length === 0 ? (
                                  <span className="membership-empty">No unmatched items stored.</span>
                                ) : null}
                              </div>
                            ) : null}
                          </div>
                        )}
                      </div>
                    ) : null}
                  </div>
                ))}
                {sourceAdapters.length === 0 ? (
                  <div className="empty-state">No source adapters configured.</div>
                ) : null}
                {sourceAdaptersError ? (
                  <p className="error-text">Source command failed: {sourceAdaptersError}</p>
                ) : null}
                {sourceRefreshError ? (
                  <p className="error-text">Source refresh failed: {sourceRefreshError}</p>
                ) : null}
                {unmatchedSourceItemsError ? (
                  <p className="error-text">Unmatched source diagnostics failed: {unmatchedSourceItemsError}</p>
                ) : null}
              </div>
            </section>
          ) : null}

          {activeSection === "Settings" ? (
            <section className="feed-panel" aria-labelledby="settings-title">
              <div className="panel-header">
                <div>
                  <h1 id="settings-title">Settings</h1>
                  <p>SQLite-backed local runtime settings.</p>
                </div>
              </div>

              <div className="settings-layout" aria-label="Application settings">
                <section className="settings-group" aria-labelledby="settings-appearance-title">
                  <h2 id="settings-appearance-title">Appearance</h2>
                  <div className="settings-row">
                    <label>
                      Theme
                      <select
                        aria-label="Settings theme"
                        value={theme}
                        onChange={(event) => updateTheme(event.target.value as Theme)}
                      >
                        <option value="dark">Dark</option>
                        <option value="light">Light</option>
                        <option value="system">System</option>
                      </select>
                    </label>
                    <div className="settings-summary">
                      <span>Palette</span>
                      <strong>{settings?.accentPalette ?? "night-neon"}</strong>
                    </div>
                  </div>
                </section>

                <section className="settings-group" aria-labelledby="settings-sources-title">
                  <h2 id="settings-sources-title">Sources</h2>
                  <div className="settings-row">
                    <label>
                      Poll interval
                      <select
                        aria-label="Settings source poll interval"
                        value={settings?.pollIntervalSeconds ?? 900}
                        onChange={(event) => updatePollInterval(Number(event.target.value))}
                      >
                        <option value={300}>5 min</option>
                        <option value={900}>15 min</option>
                        <option value={1800}>30 min</option>
                        <option value={3600}>1 hour</option>
                      </select>
                    </label>
                  </div>
                  <dl className="settings-grid">
                    <div>
                      <dt>Settings source</dt>
                      <dd>{settings?.settingsSource ?? "sqlite"}</dd>
                    </div>
                    <div>
                      <dt>Poll interval</dt>
                      <dd>{formatPollInterval(settings?.pollIntervalSeconds ?? 900)}</dd>
                    </div>
                  </dl>
                </section>

                <section className="settings-group" aria-labelledby="settings-cleanup-title">
                  <h2 id="settings-cleanup-title">Feed Cleanup</h2>
                  <dl className="settings-grid">
                    <div>
                      <dt>Feed cleanup</dt>
                      <dd>On</dd>
                    </div>
                    <div>
                      <dt>Feed retention</dt>
                      <dd>{feedPruneRetentionDays} days</dd>
                    </div>
                    <div>
                      <dt>Cleanup interval</dt>
                      <dd>Daily</dd>
                    </div>
                    <div>
                      <dt>Last cleanup</dt>
                      <dd>{formatTimestamp(feedPruneResult?.prunedAt, "Not run this session")}</dd>
                    </div>
                    <div>
                      <dt>Last cleanup deleted</dt>
                      <dd>{feedPruneResult ? feedPruneResult.itemsDeleted : "Not run this session"}</dd>
                    </div>
                    <div>
                      <dt>Protected feed items</dt>
                      <dd>Saved</dd>
                    </div>
                  </dl>
                </section>

                <section className="settings-group" aria-labelledby="settings-import-title">
                  <h2 id="settings-import-title">Import And Export</h2>
                  <dl className="settings-grid">
                    <div>
                      <dt>YAML import/export</dt>
                      <dd>{settings?.yamlImportExportStatus ?? "accepted_deferred"}</dd>
                    </div>
                    <div>
                      <dt>Settings format</dt>
                      <dd>{settings?.settingsImportExportFormat ?? "yaml"}</dd>
                    </div>
                  </dl>
                </section>

                <section className="settings-group" aria-labelledby="settings-ai-title">
                  <h2 id="settings-ai-title">AI</h2>
                  <dl className="settings-grid">
                    <div>
                      <dt>YouTube transcription</dt>
                      <dd>{formatAiProvider(settings?.aiProviders.youtubeTranscriptionProvider)}</dd>
                    </div>
                    <div>
                      <dt>YouTube transcription provider ID</dt>
                      <dd>{settings?.aiProviders.youtubeTranscriptionProvider ?? "provider_gemini"}</dd>
                    </div>
                    <div>
                      <dt>YouTube transcription model</dt>
                      <dd>{formatGeminiModel(settings?.aiProviders.youtubeTranscriptionModel)}</dd>
                    </div>
                    <div>
                      <dt>YouTube transcription timeout</dt>
                      <dd>{settings?.aiProviders.youtubeTranscriptionTimeoutSeconds ?? 300}s</dd>
                    </div>
                    <div>
                      <dt>YouTube transcription credentials</dt>
                      <dd>{formatCredentialConfigured(geminiCredentialStatus)}</dd>
                    </div>
                    <div>
                      <dt>Credential storage</dt>
                      <dd>{formatCredentialStorage(geminiCredentialStatus?.storage)}</dd>
                    </div>
                    <div>
                      <dt>Credential kind</dt>
                      <dd>{formatCredentialKind(geminiCredentialStatus?.secretKind)}</dd>
                    </div>
                    <div>
                      <dt>YouTube transcription disclosure</dt>
                      <dd>
                        Starting a transcript job sends the YouTube URL and video content to Gemini.
                      </dd>
                    </div>
                    <div>
                      <dt>YouTube transcription scope</dt>
                      <dd>Gemini is used only for YouTube transcription.</dd>
                    </div>
                    <div>
                      <dt>General AI provider</dt>
                      <dd>{formatAiProvider(settings?.aiProviders.generalAnalysisProvider)}</dd>
                    </div>
                    <div>
                      <dt>AI analysis mode</dt>
                      <dd>{settings?.aiAnalysisMode ?? "source_grounded"}</dd>
                    </div>
                  </dl>
                  <div className="settings-row">
                    <label>
                      Gemini transcription model
                      <select
                        aria-label="Gemini transcription model"
                        value={settings?.aiProviders.youtubeTranscriptionModel ?? "gemini-2.5-flash"}
                        onChange={(event) => updateYoutubeTranscriptionModel(event.target.value)}
                      >
                        <option value="gemini-2.5-flash-lite">Gemini 2.5 Flash-Lite</option>
                        <option value="gemini-2.5-flash">Gemini 2.5 Flash</option>
                        <option value="gemini-3.1-flash-lite">Gemini 3.1 Flash-Lite</option>
                        <option value="gemini-3.5-flash">Gemini 3.5 Flash</option>
                      </select>
                    </label>
                    <div className="settings-summary">
                      <span>Default</span>
                      <strong>Cheapest supported</strong>
                    </div>
                  </div>
                  <div className="settings-row">
                    <label>
                      Gemini transcription timeout
                      <select
                        aria-label="Gemini transcription timeout"
                        value={settings?.aiProviders.youtubeTranscriptionTimeoutSeconds ?? 300}
                        onChange={(event) => updateYoutubeTranscriptionTimeout(Number(event.target.value))}
                      >
                        <option value={45}>45 seconds</option>
                        <option value={90}>90 seconds</option>
                        <option value={180}>3 minutes</option>
                        <option value={300}>5 minutes</option>
                        <option value={600}>10 minutes</option>
                      </select>
                    </label>
                    <div className="settings-summary">
                      <span>Default</span>
                      <strong>5 minutes</strong>
                    </div>
                  </div>
                  <form className="credential-form" onSubmit={saveGeminiApiKey}>
                    <label>
                      Gemini API key
                      <input
                        aria-label="Gemini API key"
                        autoComplete="off"
                        placeholder={geminiCredentialStatus?.configured ? "Replace configured key" : "Paste API key"}
                        type="password"
                        value={geminiApiKeyDraft}
                        onChange={(event) => setGeminiApiKeyDraft(event.target.value)}
                      />
                    </label>
                    <div className="credential-actions">
                      <button
                        className="action-button"
                        disabled={geminiCredentialInFlight || !geminiApiKeyDraft.trim()}
                        type="submit"
                      >
                        <Save size={14} />
                        Save
                      </button>
                      <button
                        className="ghost-button"
                        disabled={geminiCredentialInFlight}
                        onClick={clearGeminiApiKey}
                        type="button"
                      >
                        <Trash2 size={14} />
                        Clear
                      </button>
                    </div>
                  </form>
                  <button
                    className="ghost-button settings-link-button"
                    onClick={() => {
                      void openUrl("https://aistudio.google.com/app/apikey");
                    }}
                    type="button"
                    title="Open Google AI Studio API keys page"
                  >
                    <ExternalLink size={14} />
                    Get Gemini API key
                  </button>
                  {geminiCredentialStatus?.devFallbackAvailable ? (
                    <p className="settings-note">
                      Development fallback is active through environment configuration.
                    </p>
                  ) : null}
                  {geminiCredentialStatus?.error ? (
                    <p className="error-text">Credential backend: {geminiCredentialStatus.error}</p>
                  ) : null}
                  {geminiCredentialError ? (
                    <p className="error-text">Credential command failed: {geminiCredentialError}</p>
                  ) : null}
                </section>

                {settingsError ? (
                  <p className="error-text">Settings command failed: {settingsError}</p>
                ) : null}
              </div>
            </section>
          ) : null}

          {activeSection === "Inbox" ? (
            <div
              aria-label="Resize feed details"
              aria-orientation="vertical"
              aria-valuemax={detailPaneMaxWidth}
              aria-valuemin={detailPaneMinWidth}
              aria-valuenow={detailPaneWidth}
              className="pane-resizer"
              onKeyDown={resizeDetailPaneWithKeyboard}
              onPointerDown={startDetailPaneResize}
              onPointerMove={resizeDetailPane}
              onPointerUp={stopDetailPaneResize}
              role="separator"
              tabIndex={0}
              title="Drag to resize feed details"
            />
          ) : null}

          {activeSection === "Inbox" ? (
            <aside className="detail-pane" aria-label="Feed item details">
              <div className="detail-icon">
                <FileText size={24} />
              </div>
              {selectedFeedItem ? (
                <>
                  <h2>{selectedFeedItem.title}</h2>
                  <section className="feed-body-section" aria-label="Feed summary">
                    <div className="feed-body-heading">
                      <span>Summary</span>
                    </div>
                    <p className="feed-detail-body">{feedItemSummary(selectedFeedItem)}</p>
                  </section>
                  <details className="feed-body-section feed-body-disclosure" aria-label="Official report body">
                    <summary className="feed-body-heading">
                      <span>Official report body</span>
                      <strong>{selectedFeedItem.bodyText ? "Stored" : "Not stored"}</strong>
                    </summary>
                    {selectedFeedItem.bodyText ? (
                      <p className="feed-detail-body">{selectedFeedItem.bodyText}</p>
                    ) : (
                      <p className="feed-detail-empty">
                        No official report body is stored for this item yet. Refresh sources and check Sources for
                        detail warnings if this remains empty.
                      </p>
                    )}
                  </details>
                  <div className="detail-actions" aria-label="Feed item actions">
                    <button
                      className="secondary-button compact-button"
                      onClick={() =>
                        updateSelectedFeedItem((item) => ({
                          ...item,
                          unread: !item.unread,
                        }))
                      }
                      type="button"
                    >
                      {selectedFeedItem.unread ? <MailOpen size={15} /> : <Mail size={15} />}
                      {selectedFeedItem.unread ? "Mark read" : "Mark unread"}
                    </button>
                    <button
                      className="secondary-button compact-button"
                      onClick={() =>
                        updateSelectedFeedItem((item) => ({
                          ...item,
                          saved: !item.saved,
                        }))
                      }
                      type="button"
                    >
                      <Save size={15} />
                      {selectedFeedItem.saved ? "Unsave" : "Save"}
                    </button>
                    {selectedFeedCompany ? (
                      <button
                        className="secondary-button compact-button"
                        onClick={() => openCompanyWorkspaceFromFeedItem(selectedFeedItem)}
                        type="button"
                      >
                        <Building2 size={15} />
                        Open company
                      </button>
                    ) : null}
                    {selectedFeedCompany ? (
                      <button
                        className="secondary-button compact-button"
                        onClick={() => openFeedItemNoteDraft(selectedFeedItem)}
                        type="button"
                      >
                        <BookOpenText size={15} />
                        Note
                      </button>
                    ) : null}
                    <a
                      className="secondary-button compact-button"
                      href={selectedFeedItem.sourceUrl}
                      rel="noreferrer"
                      target="_blank"
                    >
                      <ExternalLink size={15} />
                      Open source
                    </a>
                  </div>
                  <dl>
                    <div>
                      <dt>Company</dt>
                      <dd>{selectedFeedItem.company}</dd>
                    </div>
                    <div>
                      <dt>Source</dt>
                      <dd>{selectedFeedItem.source}</dd>
                    </div>
                    <div>
                      <dt>Source URL</dt>
                      <dd>
                        <a href={selectedFeedItem.sourceUrl} rel="noreferrer" target="_blank">
                          {selectedFeedItem.sourceUrl}
                        </a>
                      </dd>
                    </div>
                    <div>
                      <dt>Published</dt>
                      <dd>{formatTimestamp(selectedFeedItem.publishedAt, "Unknown")}</dd>
                    </div>
                    <div>
                      <dt>Fetched</dt>
                      <dd>{formatTimestamp(selectedFeedItem.fetchedAt, "Unknown")}</dd>
                    </div>
                    <div>
                      <dt>Attribution</dt>
                      <dd>{selectedFeedItem.attribution}</dd>
                    </div>
                  </dl>
                  {selectedFeedItem.attachments.length > 0 ? (
                    <div className="feed-attachment-list" aria-label="Feed attachments">
                      {selectedFeedItem.attachments.map((attachment) => (
                        <a
                          className="feed-attachment-link"
                          href={attachment.url}
                          key={attachment.id}
                          rel="noreferrer"
                          target="_blank"
                        >
                          <ExternalLink size={14} />
                          {attachment.label}
                        </a>
                      ))}
                    </div>
                  ) : null}
                </>
              ) : (
                <>
                  <h2>No item selected</h2>
                  <p>Select a feed item to inspect source details and origin links.</p>
                </>
              )}
              {healthError ? <p className="error-text">Health command failed: {healthError}</p> : null}
              {databaseError ? (
                <p className="error-text">Database command failed: {databaseError}</p>
              ) : null}
            </aside>
          ) : null}
        </section>
      </main>
    </div>
  );
}
