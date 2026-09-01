import type { ReactNode } from "react";
import type { TranscriptSegment } from "../../api/types";

// F4b S2 (docs/plans/f4b-contracts/s2-transcripts.md item 2, decision 3): the
// 4-label status dictionary — the plText key IS the visible English text
// (`text()` looks it up verbatim), so each entry must be a plText key with no
// collision against an existing, differently-worded usage elsewhere. "Queued"
// already matches; "Ready" repoints an orphaned key (nothing else called it);
// "Failed" collides with Today's autopilot-run label ("Niepowodzenie"), so the
// transcript-scoped status uses a distinct key ("Transcript failed") instead.
const STATUS_LABEL: Record<string, string> = {
  queued: "Queued",
  running: "In progress",
  completed: "Ready",
  failed: "Transcript failed",
};

export function transcriptStatusLabel(status: string): string {
  return STATUS_LABEL[status] ?? "Queued";
}

// Rust provider error codes (src-tauri/src/providers/transcripts/types.rs
// `TranscriptProviderError::code`) — a closed dictionary via `text()`, never
// `formatEnumLabel` (contract item 2: raw enum labels are retired from this
// screen).
const ERROR_CODE_LABEL: Record<string, string> = {
  provider_not_configured: "Gemini is not configured",
  provider_limit: "Gemini usage limit reached",
  provider_unavailable: "Gemini is unavailable right now",
  provider_error: "Gemini reported an error",
  network_error: "Network error",
  invalid_source_url: "Invalid recording link",
  parse_error: "Could not read the transcript",
  unknown: "Unknown transcription error",
};

export function transcriptErrorCodeLabel(code: string | null): string {
  return (code && ERROR_CODE_LABEL[code]) || ERROR_CODE_LABEL.unknown;
}

// Row title (contract item 2, decision "Tytuł ludzki"): the recording's own
// label when set, else a generic fallback — never the raw URL.
export function transcriptJobTitle(sourceLabel: string | null): string {
  return sourceLabel && sourceLabel.trim() ? sourceLabel : "Recording from YouTube";
}

export function transcriptUrlValidationMessage(value: string) {
  const trimmed = value.trim();

  if (!trimmed) {
    return "URL is required.";
  }

  if (!isYouTubeUrl(trimmed)) {
    return "Use a YouTube URL from youtube.com or youtu.be.";
  }

  return null;
}

export function transcriptSegmentTimestamp(segment: TranscriptSegment) {
  if (segment.startSeconds === null && segment.endSeconds === null) {
    return "No timestamp";
  }

  if (segment.endSeconds === null) {
    return formatTranscriptSecond(segment.startSeconds ?? 0);
  }

  return `${formatTranscriptSecond(segment.startSeconds ?? 0)}-${formatTranscriptSecond(segment.endSeconds)}`;
}

export function transcriptSegmentMatchesQuery(segment: TranscriptSegment, query: string) {
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

export function highlightSearchMatch(text: string, query: string): ReactNode {
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

function formatTranscriptSecond(value: number) {
  const safeValue = Math.max(0, Math.floor(value));
  const minutes = Math.floor(safeValue / 60);
  const seconds = safeValue % 60;

  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}
