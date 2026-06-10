import type { ReactNode } from "react";
import type { TranscriptSegment } from "../../api/types";

export function formatTranscriptStatus(value: string) {
  return formatTranscriptEnumLabel(value);
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

function formatTranscriptEnumLabel(value: string) {
  return value
    .split("_")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}
