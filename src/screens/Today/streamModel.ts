/**
 * The Today attention-stream model (ADR 0087 decision 1). A PURE, deterministic
 * pipeline that turns per-category row inputs into an ordered render list:
 *
 *   dedup (one item per company+category+evidence, keeping the newest)
 *     → group (same category+company collapses into one group row, members
 *       newest-first, with a count)
 *     → rank (severity urgent→notable→routine, then recency)
 *
 * CRITICAL (ADR 0087 dec. 2): the model NEVER infers importance from strings.
 * Every input carries an EXPLICIT typed `severity` — the backend `severity`
 * field on the attention/autopilot payloads (ADR 0087 dec. 2, mapped once in
 * `storage::severity`); ranking reads only that field.
 */

export type Severity = "urgent" | "notable" | "routine";

export type StreamCategory = "autopilot" | "attention" | "verify" | "changed" | "upcoming";

/** One underlying item, carrying enough to dedup, group, rank, and render it. */
export type StreamItem<T> = {
  /** Stable unique id (React key + last-resort ordering tiebreak). */
  id: string;
  category: StreamCategory;
  /** Grouping key part; a stable per-company identity (id or ticker fallback). */
  companyId: string;
  /** Typed importance — fed in explicitly, never inferred from strings here. */
  severity: Severity;
  /**
   * The systemic-cause key within a category (e.g. an attention event's
   * `source_reconciliation` / `insider_transaction`), used ONLY for the urgent
   * cross-company aggregate (ADR 0087 amendment 2026-07-23). Absent for rows that
   * never aggregate on cause; the model falls back to the `category` then.
   */
  subCategory?: string;
  /** ISO-8601 recency key, or null (sorts oldest/last). */
  timestamp: string | null;
  /** Dedup identity within a (company, category): repeats of it collapse. */
  evidenceKey: string;
  /** Opaque render payload — the model does not inspect it. */
  payload: T;
};

/** A ranked render row: a single item (count 1) or a collapsed group (count N). */
export type StreamGroup<T> = {
  kind: "group";
  /** `${category}::${companyId}` — stable identity for keys and sort ties. */
  key: string;
  category: StreamCategory;
  companyId: string;
  /** Most-severe member's severity (drives ranking + the row's severity cue). */
  severity: Severity;
  /** Newest member's systemic-cause key (the urgent-aggregate grouping key). */
  subCategory?: string;
  /** Newest member's timestamp (drives recency ranking + the row date). */
  timestamp: string | null;
  count: number;
  /** Members, newest-first. */
  members: StreamItem<T>[];
};

/**
 * A cross-company aggregate row that folds many company rows sharing one cause
 * into a single line so the owner's real "wall" reads as one entry. Three kinds
 * (ADR 0087 dec. 1 + the 2026-07-23 amendment):
 *
 * - **routine** — more than `ROUTINE_AGGREGATE_THRESHOLD` same-`category` routine
 *   rows (e.g. 28 companies × 1 routine autopilot run each), the mockup's original
 *   "×K spółek" row (category-keyed).
 * - **urgent** — `URGENT_AGGREGATE_MIN`+ urgent rows sharing one systemic cause
 *   (`subCategory`, e.g. many companies each with a missed-report reconciliation).
 *   These lead the stream and keep `urgent` severity.
 * - **notable** — more than `NOTABLE_AGGREGATE_THRESHOLD` notable rows sharing one
 *   cause (e.g. events that aged out of `urgent`, or a multi-company price-alert
 *   day). Ranks below urgent, above routine.
 *
 * Urgent and notable are keyed by cause, so **different causes never merge** and
 * **severity partitions first** (a cause's urgent and notable rows never fold
 * together). Routine stays category-keyed.
 */
export type StreamAggregate<T> = {
  kind: "aggregate";
  /** `aggregate::${category}` (routine) or `aggregate::${severity}::${cause}` (urgent/notable). */
  key: string;
  category: StreamCategory;
  /** The folded rows' shared severity (an urgent/notable systemic cause, or a routine wall). */
  severity: Severity;
  /** Newest collapsed group's timestamp. */
  timestamp: string | null;
  /** Number of companies (collapsed group rows) folded in. */
  count: number;
  /** The collapsed group rows, newest-first. */
  members: StreamGroup<T>[];
};

/** A row in the final Today render list: a company group or a cross-company aggregate. */
export type StreamRenderRow<T> = StreamGroup<T> | StreamAggregate<T>;

// Collapse cross-company routine rows only when there are MORE THAN this many of
// one category (so 1–3 stay as their own rows; 4+ fold into a "×K spółek" row).
export const ROUTINE_AGGREGATE_THRESHOLD = 3;

// Collapse cross-company NOTABLE rows when there are MORE THAN this many sharing
// one cause (ADR 0087 amendment 2026-07-23): the routine "more than N" wall
// semantics, applied to notable so a wall that merely aged out of `urgent` still
// folds into one line (its own const from the routine threshold — same value, but
// a notable wall is a distinct concept and independently tunable).
export const NOTABLE_AGGREGATE_THRESHOLD = 3;

// Collapse cross-company URGENT rows when this many OR MORE share one systemic
// cause: a single urgent cause hitting several companies (e.g. many missed-report
// reconciliations) is ONE alarm, not a wall. Its own const — urgent collapses far
// more eagerly (2, not 4) because a repeated urgent cause reads as systemic.
export const URGENT_AGGREGATE_MIN = 2;

const SEVERITY_RANK: Record<Severity, number> = { urgent: 0, notable: 1, routine: 2 };

/**
 * The ONE frontend-side severity entry (ADR 0087 dec. 2). Every other severity is
 * derived once in `storage::severity` and shipped typed on the backend payloads;
 * claims-to-verify carry no backend severity, so the Today stream classifies them
 * here — the single, spec-anchored exception (product-spec §Attention Routing
 * severity table, the row marked "frontend stream model"). A claim OVERDUE for
 * verification escalates to `notable` so it ranks above routine rows (and below
 * urgent); a claim merely due-but-not-yet-overdue stays `routine`. A claim is NOT
 * an attention event — this severity never raises a toast.
 */
export function verifyClaimSeverity(bucket: "overdue" | "due"): Severity {
  return bucket === "overdue" ? "notable" : "routine";
}

function moreSevere(a: Severity, b: Severity): Severity {
  return SEVERITY_RANK[a] <= SEVERITY_RANK[b] ? a : b;
}

/** Newer timestamp first; null sorts last. Returns <0 when `a` should precede `b`. */
function byRecencyDesc(a: string | null, b: string | null): number {
  if (a === b) return 0;
  if (a === null) return 1;
  if (b === null) return -1;
  return b.localeCompare(a);
}

/**
 * Run the dedup → group → rank pipeline. Input order does not matter; output is
 * deterministic (a stable tiebreak on the group key keeps equal rows in a fixed
 * order).
 */
export function buildStream<T>(items: StreamItem<T>[]): StreamGroup<T>[] {
  // 1. dedup — collapse repeats of (company, category, evidence), newest wins.
  const deduped = new Map<string, StreamItem<T>>();
  for (const it of items) {
    const dedupKey = `${it.category}::${it.companyId}::${it.evidenceKey}`;
    const prior = deduped.get(dedupKey);
    if (!prior || byRecencyDesc(it.timestamp, prior.timestamp) < 0) {
      deduped.set(dedupKey, it);
    }
  }

  // 2. group — bucket by (category, company); members newest-first.
  const buckets = new Map<string, StreamItem<T>[]>();
  for (const it of deduped.values()) {
    const groupKey = `${it.category}::${it.companyId}`;
    const bucket = buckets.get(groupKey);
    if (bucket) bucket.push(it);
    else buckets.set(groupKey, [it]);
  }

  const groups: StreamGroup<T>[] = [];
  for (const [key, membersRaw] of buckets) {
    const members = [...membersRaw].sort((a, b) => byRecencyDesc(a.timestamp, b.timestamp));
    const first = members[0];
    const severity = members.reduce<Severity>((acc, m) => moreSevere(acc, m.severity), "routine");
    groups.push({
      kind: "group",
      key,
      category: first.category,
      companyId: first.companyId,
      severity,
      // The newest member's cause represents the group for urgent aggregation.
      subCategory: first.subCategory,
      timestamp: first.timestamp,
      count: members.length,
      members,
    });
  }

  // 3. rank — severity (urgent first), then recency (newest first), then key.
  groups.sort((a, b) => {
    const bySeverity = SEVERITY_RANK[a.severity] - SEVERITY_RANK[b.severity];
    if (bySeverity !== 0) return bySeverity;
    const byRecency = byRecencyDesc(a.timestamp, b.timestamp);
    if (byRecency !== 0) return byRecency;
    return a.key.localeCompare(b.key);
  });

  return groups;
}

/**
 * Second grouping stage (ADR 0087 dec. 1, mockup ×K-spółek): after the ranked
 * company+category groups are built, collapse the same-category ROUTINE rows
 * into a single cross-company aggregate whenever there are more than
 * `ROUTINE_AGGREGATE_THRESHOLD` of them. The aggregate takes the slot of the
 * newest collapsed row (routine rows already trail urgent/notable), so ranking
 * is preserved. Urgent/notable rows always pass through untouched.
 */
export function collapseRoutineAggregates<T>(
  groups: StreamGroup<T>[],
): StreamRenderRow<T>[] {
  const routineCountByCategory = new Map<StreamCategory, number>();
  for (const group of groups) {
    if (group.severity === "routine") {
      routineCountByCategory.set(
        group.category,
        (routineCountByCategory.get(group.category) ?? 0) + 1,
      );
    }
  }

  const collapsing = new Set<StreamCategory>();
  for (const [category, count] of routineCountByCategory) {
    if (count > ROUTINE_AGGREGATE_THRESHOLD) collapsing.add(category);
  }
  if (collapsing.size === 0) return groups;

  // Collect the members (newest-first, i.e. the ranked order) per collapsed category.
  const membersByCategory = new Map<StreamCategory, StreamGroup<T>[]>();
  for (const group of groups) {
    if (group.severity === "routine" && collapsing.has(group.category)) {
      const bucket = membersByCategory.get(group.category);
      if (bucket) bucket.push(group);
      else membersByCategory.set(group.category, [group]);
    }
  }

  const out: StreamRenderRow<T>[] = [];
  const emitted = new Set<StreamCategory>();
  for (const group of groups) {
    if (group.severity === "routine" && collapsing.has(group.category)) {
      if (emitted.has(group.category)) continue;
      emitted.add(group.category);
      const members = membersByCategory.get(group.category) ?? [group];
      out.push({
        kind: "aggregate",
        key: `aggregate::${group.category}`,
        category: group.category,
        severity: "routine",
        timestamp: members[0].timestamp,
        count: members.length,
        members,
      });
    } else {
      out.push(group);
    }
  }
  return out;
}

/** The systemic-cause key a group aggregates on within its severity (its cause, else its category). */
function causeKey<T>(group: StreamGroup<T>): string {
  return group.subCategory ?? group.category;
}

/** A group is cause-aggregation-eligible when it is an urgent or notable GROUP row. */
function isCauseEligible<T>(row: StreamRenderRow<T>): row is StreamGroup<T> {
  return row.kind === "group" && (row.severity === "urgent" || row.severity === "notable");
}

/** Minimum member count for a severity's same-cause wall to collapse (inclusive). */
function causeAggregateMin(severity: Severity): number {
  // Urgent collapses eagerly (a repeated urgent cause is systemic, one alarm);
  // notable mirrors the routine "more than N" wall (1–3 stay, 4+ fold).
  return severity === "urgent" ? URGENT_AGGREGATE_MIN : NOTABLE_AGGREGATE_THRESHOLD + 1;
}

/**
 * Urgent + notable cross-company collapse (ADR 0087 amendment 2026-07-23, the
 * live-checkpoint P1 fix + its notable-wall follow-up). Runs AFTER the routine
 * collapse over the mixed render rows: urgent/notable GROUP rows are partitioned
 * by `(severity, cause)` and each partition reaching its threshold
 * (`URGENT_AGGREGATE_MIN` / more-than-`NOTABLE_AGGREGATE_THRESHOLD`) folds into one
 * aggregate ranked by its newest member. Because severity is part of the partition
 * key, **severity partitions first** — a cause's urgent and notable rows never fold
 * together — and **different causes never merge**. Routine aggregates and any
 * sub-threshold group pass through untouched, preserving the ranked order.
 */
export function collapseCauseAggregates<T>(rows: StreamRenderRow<T>[]): StreamRenderRow<T>[] {
  const partitionKey = (group: StreamGroup<T>) => `${group.severity}::${causeKey(group)}`;

  // Count members per (severity, cause) partition.
  const stats = new Map<string, { severity: Severity; count: number }>();
  for (const row of rows) {
    if (!isCauseEligible(row)) continue;
    const key = partitionKey(row);
    const prior = stats.get(key);
    if (prior) prior.count += 1;
    else stats.set(key, { severity: row.severity, count: 1 });
  }

  const collapsing = new Set<string>();
  for (const [key, { severity, count }] of stats) {
    if (count >= causeAggregateMin(severity)) collapsing.add(key);
  }
  if (collapsing.size === 0) return rows;

  // Collect the collapsing members (in ranked, newest-first order) per partition.
  const membersByKey = new Map<string, StreamGroup<T>[]>();
  for (const row of rows) {
    if (!isCauseEligible(row)) continue;
    const key = partitionKey(row);
    if (!collapsing.has(key)) continue;
    const bucket = membersByKey.get(key);
    if (bucket) bucket.push(row);
    else membersByKey.set(key, [row]);
  }

  const out: StreamRenderRow<T>[] = [];
  const emitted = new Set<string>();
  for (const row of rows) {
    if (isCauseEligible(row) && collapsing.has(partitionKey(row))) {
      const key = partitionKey(row);
      if (emitted.has(key)) continue;
      emitted.add(key);
      const members = membersByKey.get(key) ?? [row];
      out.push({
        kind: "aggregate",
        key: `aggregate::${row.severity}::${causeKey(row)}`,
        category: row.category,
        severity: row.severity,
        timestamp: members[0].timestamp,
        count: members.length,
        members,
      });
    } else {
      out.push(row);
    }
  }
  return out;
}
