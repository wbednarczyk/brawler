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

/** The row's own sort timestamp — exported for TodayScreen's within-day
 * item/attention interleave (both are already sorted newest-first per list;
 * merging them needs the same key this module sorts by internally). */
export function itemTimestamp(item: TodayItem): string {
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

// Newest-first string compare that actually returns 0 on a tie (unlike a
// bare `a < b ? 1 : -1`, which never does and so cannot promise stable
// ordering for equal keys — ISO timestamps compare correctly as strings).
function compareDesc(a: string, b: string): number {
  if (a === b) return 0;
  return a < b ? 1 : -1;
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

  const todayKey = formatLocalDate(now);
  for (const item of items) {
    let day = localDayKey(itemTimestamp(item));
    // NOTHING buckets above DZIŚ: a future calendar announcement belongs to
    // the DZIŚ section (Main.dc.html "zapowiedzi z kalendarza"), and any other
    // future-stamped row (clock skew, anomalous seed) must not spawn a future
    // day that eats one of the two visible display slots and pushes the real
    // queue into the "Earlier" rollup (caught by density/J1, S5).
    if (day > todayKey) day = todayKey;
    bucketFor(day).items.push(item);
  }
  for (const event of attention) {
    if (event.dismissed) continue;
    // Same no-section-above-DZIŚ clamp as items: a future `firedAt` (clock
    // skew) must not eat a visible display slot.
    let day = localDayKey(event.firedAt);
    if (day > todayKey) day = todayKey;
    bucketFor(day).attention.push(event);
  }

  for (const bucket of byDay.values()) {
    // A true 3-way compare (not `< ? 1 : -1`, which returns -1 for EVERY tied
    // pair and so never actually preserves insertion order on a tie — two
    // items published in the same second, a real possibility, would sort by
    // implementation-defined comparator internals instead of staying stable).
    bucket.items.sort((a, b) => compareDesc(itemTimestamp(a), itemTimestamp(b)));
    bucket.attention.sort((a, b) => compareDesc(a.firedAt, b.firedAt));
    bucket.total = bucket.items.length + bucket.attention.length;
    bucket.unseen =
      bucket.items.filter((item) => !isItemSeen(item)).length +
      bucket.attention.filter((event) => !event.seen).length;
  }

  return Array.from(byDay.values()).sort((a, b) => compareDesc(a.day, b.day));
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
    entries.sort((a, b) => compareDesc(a.ts, b.ts));
    if (entries[0]) return entries[0].candidate;
  }
  return null;
}

/** A single collapsed rollup line standing in for every day bucket beyond the
 * two freshest-with-content display slots (DZIŚ + WCZORAJ) — plan decision 5
 * / contract §5, Delta.dc.html "WCZEŚNIEJ W TYM TYGODNIU · pn–wt · 14
 * pozycji". */
export type EarlierRollup = {
  /** Newest-first, same order as `bucketByLocalDay`'s output — the buckets
   * folded into this rollup, rendered individually once expanded. */
  buckets: DayBucket[];
  count: number;
  unseen: number;
  oldestDay: string;
  newestDay: string;
};

/**
 * Splits bucketed days into the two always-visible display slots (the
 * freshest two calendar days with content — usually DZIŚ + WCZORAJ, but
 * whichever two are newest if either is empty) and a rollup summarizing
 * everything older (plan decision 5 / contract §5). `null` earlier when
 * there is nothing left to roll up.
 */
export function splitDisplayBuckets(buckets: DayBucket[]): { visible: DayBucket[]; earlier: EarlierRollup | null } {
  const visible = buckets.slice(0, 2);
  const rest = buckets.slice(2);
  if (rest.length === 0) return { visible, earlier: null };
  return {
    visible,
    earlier: {
      buckets: rest,
      count: rest.reduce((sum, bucket) => sum + bucket.total, 0),
      unseen: rest.reduce((sum, bucket) => sum + bucket.unseen, 0),
      oldestDay: rest[rest.length - 1].day,
      newestDay: rest[0].day,
    },
  };
}

/**
 * S-tier row cap (plan Dense row "S top-3+zwiń", Compact.dc.html): a day
 * section narrower than 420px shows only the newest 3 rows plus a "+N ·
 * Otwórz dzień" recovery line. The pure decision only — mirrors
 * `EventsScreen`'s `resolveEventViewMode` split: the width MEASUREMENT is
 * browser-only (jsdom has no container queries/ResizeObserver;
 * `density-matrix.spec.ts` proves the live behavior), this is what a
 * measured `narrow` turns into.
 */
export function capDayRows<T>(
  rows: T[],
  narrow: boolean,
  expanded: boolean,
  cap = 3,
): { visible: T[]; hidden: number } {
  if (!narrow || expanded || rows.length <= cap) return { visible: rows, hidden: 0 };
  return { visible: rows.slice(0, cap), hidden: rows.length - cap };
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
