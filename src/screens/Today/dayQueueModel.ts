import type { AttentionEvent } from "../../api/attention";
import type { TodayDeltaSummary } from "../../api/generated/TodayDeltaSummary";
import type { TodayItem } from "../../api/generated/TodayItem";
import { addLocalDays, formatLocalDate } from "../../shared/format/datetime";
import type { LocaleCode } from "../../shared/locale";
import { FILING_FORMS, REPORT_FORMS, pluralNoun } from "../../shared/locale/plural";

/**
 * Pure Dziś v2 day-bucketing model (F2 S3, plan decision 1). Groups the flat
 * `TodayView.items` (S1) with the root-fed `AttentionEvent[]` (ADR 0106 dec.
 * 5) into local-calendar-day buckets, and is the ONE authoritative layer for
 * unseen counts / `allSeen` — S1's read model carries no read/seen
 * aggregation of its own (that split was deliberate: attention stays root-fed
 * and only merges with Dziś on the frontend). Every function here is pure and
 * takes `now` explicitly — never `Date.now()` internally — so bucketing,
 * "today"/"yesterday" labels, and the primary-action pick stay deterministic
 * under test.
 */

export type RelativeDay = "today" | "yesterday" | "earlier";

export type DayBucket = {
  /** `YYYY-MM-DD`, the user's LOCAL calendar day. */
  day: string;
  relativeDay: RelativeDay;
  /** Newest-first within the bucket. */
  items: TodayItem[];
  /** Newest-first within the bucket; dismissed events are excluded (they live
   * in Archive, plan decision 7 — never in the day queue). */
  attention: AttentionEvent[];
  total: number;
  unseen: number;
};

export type PrimaryCandidate =
  | { kind: "attention"; event: AttentionEvent }
  | { kind: "item"; item: TodayItem };

function itemTimestamp(item: TodayItem): string {
  switch (item.kind) {
    case "filing":
      return item.publishedAt;
    case "mediaCluster":
      return item.latestPublishedAt;
    case "nonArrival":
      return item.eventDate;
    case "calendar":
      return item.eventDate;
    case "autopilotRun":
      return item.run.createdAt;
  }
}

const BARE_DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

// A bare `YYYY-MM-DD` domain date (calendar/non-arrival `eventDate`) IS
// already the local calendar-day key — returning it as-is avoids the classic
// pitfall of round-tripping it through `new Date(...)`, which the JS spec
// parses as UTC MIDNIGHT for a date-only ISO string (so a west-of-UTC reader
// would see it shift a day earlier). A full timestamp (with a time part,
// always UTC `Z` from the backend — S1) gets a real UTC→local conversion via
// `Date`'s local getters.
function localDayKey(iso: string): string {
  if (BARE_DATE_RE.test(iso)) return iso;
  const parsed = new Date(iso);
  return Number.isNaN(parsed.getTime()) ? iso.slice(0, 10) : formatLocalDate(parsed);
}

function relativeDayOf(day: string, now: Date): RelativeDay {
  if (day === formatLocalDate(now)) return "today";
  if (day === formatLocalDate(addLocalDays(now, -1))) return "yesterday";
  return "earlier";
}

/**
 * Whether an item carries its own read/seen signal. Only `filing` does
 * (`TodayItem.read`, sourced from the feed read-model) — `mediaCluster`,
 * `nonArrival`, `calendar`, and `autopilotRun` carry no per-item seen state in
 * the DTO (S1 never tracked one for them), so they always count as unseen; a
 * day holding one only collapses via the manual `todayReviewedDays` override
 * (plan decision 5), never by derivation alone.
 */
export function isItemSeen(item: TodayItem): boolean {
  return item.kind === "filing" && item.read;
}

// A row with no landing destination (a calendar row carries no action —
// Wiersze.dc.html "bez akcji — czeka") is never eligible as the header's
// primary CTA.
function isActionable(item: TodayItem): boolean {
  return item.kind !== "calendar";
}

/** THE authoritative "is this day done" check (plan decision 5): every row
 * read/seen. Used both to auto-collapse a day and as half of the undo gesture
 * ("Oznacz jako nieprzejrzany" is the other half, via `todayReviewedDays`). */
export function allSeen(bucket: DayBucket): boolean {
  return bucket.unseen === 0;
}

export function bucketByLocalDay(
  items: TodayItem[],
  attention: AttentionEvent[],
  now: Date,
): DayBucket[] {
  const byDay = new Map<string, DayBucket>();

  function bucketFor(day: string): DayBucket {
    let bucket = byDay.get(day);
    if (!bucket) {
      bucket = {
        day,
        relativeDay: relativeDayOf(day, now),
        items: [],
        attention: [],
        total: 0,
        unseen: 0,
      };
      byDay.set(day, bucket);
    }
    return bucket;
  }

  for (const item of items) {
    bucketFor(localDayKey(itemTimestamp(item))).items.push(item);
  }
  for (const event of attention) {
    if (event.dismissed) continue;
    bucketFor(localDayKey(event.firedAt)).attention.push(event);
  }

  for (const bucket of byDay.values()) {
    bucket.items.sort((a, b) => (itemTimestamp(a) < itemTimestamp(b) ? 1 : -1));
    bucket.attention.sort((a, b) => (a.firedAt < b.firedAt ? 1 : -1));
    bucket.total = bucket.items.length + bucket.attention.length;
    bucket.unseen =
      bucket.items.filter((item) => !isItemSeen(item)).length +
      bucket.attention.filter((event) => !event.seen).length;
  }

  return Array.from(byDay.values()).sort((a, b) => (a.day < b.day ? 1 : -1));
}

/**
 * The single most-urgent actionable item for the header CTA (plan decision
 * 8): urgent attention > unread report filing > non-arrival > newest unseen
 * otherwise, scanning newest day first. `null` on a clean morning (no primary
 * — the mockup's empty state is deliberately CTA-less).
 */
export function pickPrimary(buckets: DayBucket[]): PrimaryCandidate | null {
  for (const bucket of buckets) {
    const urgent = bucket.attention.find((event) => !event.seen && event.severity === "urgent");
    if (urgent) return { kind: "attention", event: urgent };
  }
  for (const bucket of buckets) {
    const report = bucket.items.find(
      (item) => item.kind === "filing" && item.presentationKind === "report" && !item.read,
    );
    if (report) return { kind: "item", item: report };
  }
  for (const bucket of buckets) {
    const nonArrival = bucket.items.find((item) => item.kind === "nonArrival");
    if (nonArrival) return { kind: "item", item: nonArrival };
  }
  for (const bucket of buckets) {
    const entries: Array<{ ts: string; candidate: PrimaryCandidate }> = [
      ...bucket.items
        .filter((item) => isActionable(item) && !isItemSeen(item))
        .map((item) => ({ ts: itemTimestamp(item), candidate: { kind: "item" as const, item } })),
      ...bucket.attention
        .filter((event) => !event.seen)
        .map((event) => ({ ts: event.firedAt, candidate: { kind: "attention" as const, event } })),
    ];
    entries.sort((a, b) => (a.ts < b.ts ? 1 : -1));
    if (entries[0]) return entries[0].candidate;
  }
  return null;
}

/**
 * The delta header's headline sentence ("Od Twojej ostatniej wizyty: 1
 * raport, 3 komunikaty", Main.dc.html): report/filing counts only — media
 * moves to a secondary note the caller composes separately, since a raw media
 * count alone is not itself an action.
 */
export function formatDeltaHeadline(
  summary: TodayDeltaSummary,
  locale: LocaleCode,
  text: (value: string) => string,
): string {
  const parts: string[] = [];
  if (summary.reportCount > 0) {
    parts.push(`${summary.reportCount} ${pluralNoun(locale, summary.reportCount, REPORT_FORMS)}`);
  }
  if (summary.filingCount > 0) {
    parts.push(`${summary.filingCount} ${pluralNoun(locale, summary.filingCount, FILING_FORMS)}`);
  }
  if (parts.length === 0) {
    return text("Nothing new since your last visit");
  }
  return `${text("Since your last visit")}: ${parts.join(", ")}`;
}
