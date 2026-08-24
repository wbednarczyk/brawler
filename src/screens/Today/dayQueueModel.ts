import type { AlertRule, AttentionEvent } from "../../api/attention";
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
 *
 * The backend's `mediaItem` is FLAT — one row per (feed item × matched
 * company), no server-side day-clustering (FIX WAVE A finding 8: the old
 * server-side UTC-day cluster mis-assigned items across LOCAL midnight). This
 * module re-clusters `mediaItem` rows into `MediaCluster` display rows itself,
 * grouped by (companyId, LOCAL day) — the same local-day boundary every other
 * row already buckets by, so clustering falls naturally out of bucketing:
 * every `mediaItem` in one day bucket that shares a companyId is one cluster.
 */

export type RelativeDay = "today" | "yesterday" | "earlier";

/** A synthesized display row for one company's `Public media` items on one
 * local day (F2 wave B finding 8) — built here from flat `mediaItem` rows,
 * never sent by the backend. Keeps the `mediaCluster` kind name the row kit /
 * tests already used pre-DTO-change; only its origin (frontend vs backend)
 * changed. */
export type MediaCluster = {
  kind: "mediaCluster";
  companyId: string;
  qualifiedTicker: string;
  /** `YYYY-MM-DD`, the user's LOCAL calendar day (matches the owning bucket). */
  day: string;
  count: number;
  earliestPublishedAt: string;
  latestPublishedAt: string;
  /** The 3 most recent member titles, newest first. */
  topTitles: string[];
  feedItemIds: string[];
  /** True when at least one member is unread. */
  unread: boolean;
};

/** One day-queue display row: every `TodayItem` kind except the flat
 * `mediaItem` (which this module clusters into `MediaCluster` before a row
 * ever renders) plus the synthesized cluster itself. */
export type DayRow = Exclude<TodayItem, { kind: "mediaItem" }> | MediaCluster;

export type DayBucket = {
  /** `YYYY-MM-DD`, the user's LOCAL calendar day. */
  day: string;
  relativeDay: RelativeDay;
  /** Newest-first within the bucket. */
  items: DayRow[];
  /** Newest-first within the bucket; dismissed events are excluded (they live
   * in Archive, plan decision 7 — never in the day queue). */
  attention: AttentionEvent[];
  total: number;
  unseen: number;
};

export type PrimaryCandidate =
  | { kind: "attention"; event: AttentionEvent }
  | { kind: "item"; item: DayRow };

/** The row's own sort timestamp — exported for TodayScreen's within-day
 * item/attention interleave (both are already sorted newest-first per list;
 * merging them needs the same key this module sorts by internally). */
export function itemTimestamp(item: DayRow): string {
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

/** The raw `TodayItem`'s bucketing timestamp — INTERNAL, used before
 * clustering collapses `mediaItem` rows into a `MediaCluster` (which then
 * sorts by `itemTimestamp` above). */
function rawItemTimestamp(item: TodayItem): string {
  switch (item.kind) {
    case "filing":
      return item.publishedAt;
    case "mediaItem":
      return item.publishedAt;
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
 * Whether a row carries its own read/seen signal (fix wave B finding 6):
 * `filing` reads the feed read-model's `read` flag; `mediaCluster` is seen
 * once EVERY member is read (a cluster with one unread article among ten
 * stays open); `autopilotRun` is seen once its notification state moves past
 * `unread`; `calendar` is always seen — an upcoming announcement carries no
 * decision of its own, it is informational only; `nonArrival` is NEVER seen —
 * an unresolved missing report keeps its day open until the shared
 * `report_delay` predicate either witnesses the report or raises the flag
 * (which hands the row to root-fed attention, plan decision 2), never by a
 * seen-flip alone.
 */
export function isItemSeen(item: DayRow): boolean {
  switch (item.kind) {
    case "filing":
      return item.read;
    case "mediaCluster":
      return !item.unread;
    case "autopilotRun":
      return item.run.notificationState !== "unread";
    case "calendar":
      return true;
    case "nonArrival":
      return false;
  }
}

// A row with no landing destination (a calendar row carries no action —
// Wiersze.dc.html "bez akcji — czeka") is never eligible as the header's
// primary CTA.
function isActionable(item: DayRow): boolean {
  return item.kind !== "calendar";
}

/** THE authoritative "is this day done" check (plan decision 5): every row
 * read/seen. Used both to auto-collapse a day and as half of the undo gesture
 * ("Oznacz jako nieprzejrzany" is the other half, via `todayReviewedDays`). */
export function allSeen(bucket: DayBucket): boolean {
  return bucket.unseen === 0;
}

/** One (company, day) media accumulator mid-clustering — internal to
 * `bucketByLocalDay`, never exposed (the finished `MediaCluster` is). */
type MediaAcc = {
  companyId: string;
  qualifiedTicker: string;
  members: Array<{ feedItemId: string; publishedAt: string; title: string; read: boolean }>;
};

type WorkingBucket = {
  day: string;
  relativeDay: RelativeDay;
  rows: DayRow[];
  /** Keyed by companyId — every `mediaItem` row landing in this day bucket
   * accumulates here first; folded into `rows` as `MediaCluster`s once every
   * item has been assigned (so a later member never reopens an already-built
   * cluster). */
  media: Map<string, MediaAcc>;
  attention: AttentionEvent[];
};

export function bucketByLocalDay(
  items: TodayItem[],
  attention: AttentionEvent[],
  now: Date,
): DayBucket[] {
  const byDay = new Map<string, WorkingBucket>();

  function bucketFor(day: string): WorkingBucket {
    let bucket = byDay.get(day);
    if (!bucket) {
      bucket = { day, relativeDay: relativeDayOf(day, now), rows: [], media: new Map(), attention: [] };
      byDay.set(day, bucket);
    }
    return bucket;
  }

  const todayKey = formatLocalDate(now);
  for (const item of items) {
    let day = localDayKey(rawItemTimestamp(item));
    // NOTHING buckets above DZIŚ: a future calendar announcement belongs to
    // the DZIŚ section (Main.dc.html "zapowiedzi z kalendarza"), and any other
    // future-stamped row (clock skew, anomalous seed) must not spawn a future
    // day that eats one of the two visible display slots and pushes the real
    // queue into the "Earlier" rollup (caught by density/J1, S5).
    if (day > todayKey) day = todayKey;
    const bucket = bucketFor(day);
    if (item.kind === "mediaItem") {
      // Cluster key is (companyId, day) — the day is already fixed to this
      // bucket, so grouping is purely per-company within it. A `feedItemId`
      // matched to multiple companies arrives as multiple `mediaItem` rows
      // (one per company, S1) and so lands in each company's own cluster —
      // membership preserved with no extra join here.
      let acc = bucket.media.get(item.companyId);
      if (!acc) {
        acc = { companyId: item.companyId, qualifiedTicker: item.qualifiedTicker, members: [] };
        bucket.media.set(item.companyId, acc);
      }
      acc.members.push({
        feedItemId: item.feedItemId,
        publishedAt: item.publishedAt,
        title: item.title,
        read: item.read,
      });
    } else {
      bucket.rows.push(item);
    }
  }
  for (const event of attention) {
    if (event.dismissed) continue;
    // Same no-section-above-DZIŚ clamp as items: a future `firedAt` (clock
    // skew) must not eat a visible display slot.
    let day = localDayKey(event.firedAt);
    if (day > todayKey) day = todayKey;
    bucketFor(day).attention.push(event);
  }

  const result: DayBucket[] = [];
  for (const working of byDay.values()) {
    for (const acc of working.media.values()) {
      // Newest-first, same convention as every other row list — earliest/
      // latest and the top-3 titles read straight off the sorted ends.
      const members = [...acc.members].sort((a, b) => compareDesc(a.publishedAt, b.publishedAt));
      working.rows.push({
        kind: "mediaCluster",
        companyId: acc.companyId,
        qualifiedTicker: acc.qualifiedTicker,
        day: working.day,
        count: members.length,
        earliestPublishedAt: members[members.length - 1].publishedAt,
        latestPublishedAt: members[0].publishedAt,
        topTitles: members.slice(0, 3).map((member) => member.title),
        feedItemIds: members.map((member) => member.feedItemId),
        unread: members.some((member) => !member.read),
      });
    }
    // A true 3-way compare (not `< ? 1 : -1`, which returns -1 for EVERY tied
    // pair and so never actually preserves insertion order on a tie — two
    // items published in the same second, a real possibility, would sort by
    // implementation-defined comparator internals instead of staying stable).
    working.rows.sort((a, b) => compareDesc(itemTimestamp(a), itemTimestamp(b)));
    working.attention.sort((a, b) => compareDesc(a.firedAt, b.firedAt));
    const total = working.rows.length + working.attention.length;
    const unseen =
      working.rows.filter((row) => !isItemSeen(row)).length +
      working.attention.filter((event) => !event.seen).length;
    result.push({
      day: working.day,
      relativeDay: working.relativeDay,
      items: working.rows,
      attention: working.attention,
      total,
      unseen,
    });
  }

  return result.sort((a, b) => compareDesc(a.day, b.day));
}

/**
 * Whether an attention event IS the `report_delay` signal's own event (fix
 * wave B finding 1b) — the finest identity the data actually carries: the
 * event only exists because an enabled `AlertRule` with
 * `signalCategory === "report_delay"` matched (`evaluate_signal_rules`,
 * `storage/attention.rs`) — a company_signal event under ANY other category
 * is a different flag and must not suppress a non-arrival row. `evidenceRef`
 * itself is an opaque backend-derived id (`company_signals.id`, hashed from
 * the calendar event id) with no stable frontend-side reconstruction, so this
 * is the precise match the data allows without duplicating backend id
 * derivation on the frontend.
 */
export function isReportDelaySignal(event: AttentionEvent, rulesById: Map<string, AlertRule>): boolean {
  if (event.evidenceType !== "company_signal" || !event.ruleId) return false;
  return rulesById.get(event.ruleId)?.signalCategory === "report_delay";
}

/**
 * Render-level non-arrival suppression (plan decision 2 / fix wave B finding
 * 1b): a `nonArrival` row for a company that already has a fired
 * `report_delay` attention event is dropped, regardless of fetch order
 * between `get_today_view` and the root-fed attention state — this guarantees
 * exactly one row (never zero, never two) independent of which of the two
 * independent fetches lands first or refreshes stale. Matches by companyId
 * only (see `isReportDelaySignal`) — the coarsest grain the data allows, but
 * a company genuinely juggling two SIMULTANEOUS overdue periodic reports is
 * not a real V1 scenario (one report calendar per company).
 */
export function suppressCapturedNonArrivals(
  items: TodayItem[],
  attentionEvents: AttentionEvent[],
  rulesById: Map<string, AlertRule>,
): TodayItem[] {
  const capturedCompanyIds = new Set(
    attentionEvents
      .filter((event) => !event.dismissed && isReportDelaySignal(event, rulesById))
      .map((event) => event.companyId),
  );
  return items.filter((item) => !(item.kind === "nonArrival" && capturedCompanyIds.has(item.companyId)));
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
