import { describe, expect, it } from "vitest";

import {
  buildStream,
  collapseCauseAggregates,
  collapseRoutineAggregates,
  NOTABLE_AGGREGATE_THRESHOLD,
  ROUTINE_AGGREGATE_THRESHOLD,
  URGENT_AGGREGATE_MIN,
  verifyClaimSeverity,
  type Severity,
  type StreamCategory,
  type StreamItem,
} from "./streamModel";

/**
 * Pure stream-model tests (ADR 0087 decision 1). The heart of the Today
 * redesign: a deterministic dedup → group → rank pipeline whose inputs carry an
 * EXPLICIT typed `severity` — the frontend never infers importance from strings
 * (ADR 0087 dec. 2). Severity is fed in directly here so ranking is proven
 * independently of the placeholder adapter wired in TodayScreen (D3a replaces it
 * with the backend field).
 */

function item(overrides: Partial<StreamItem<string>> = {}): StreamItem<string> {
  // Spread (not `??`) so an explicit `timestamp: null` survives.
  const merged = {
    id: "i1",
    category: "autopilot" as StreamCategory,
    companyId: "co_a",
    severity: "routine" as Severity,
    timestamp: "2026-07-10T09:00:00Z" as string | null,
    evidenceKey: undefined as string | undefined,
    payload: undefined as string | undefined,
    ...overrides,
  };
  return {
    ...merged,
    evidenceKey: merged.evidenceKey ?? merged.id,
    payload: merged.payload ?? merged.id,
  };
}

describe("buildStream — dedup (ADR 0087 dec. 1a)", () => {
  it("collapses repeats of the same (company, category, evidence), keeping the newest", () => {
    const groups = buildStream([
      item({ id: "old", evidenceKey: "ev", timestamp: "2026-07-01T00:00:00Z", payload: "old" }),
      item({ id: "new", evidenceKey: "ev", timestamp: "2026-07-09T00:00:00Z", payload: "new" }),
    ]);
    expect(groups).toHaveLength(1);
    expect(groups[0].count).toBe(1);
    expect(groups[0].members[0].payload).toBe("new");
  });

  it("does not dedup items that differ in evidence (they become group members)", () => {
    const groups = buildStream([
      item({ id: "a", evidenceKey: "ev_a" }),
      item({ id: "b", evidenceKey: "ev_b" }),
    ]);
    expect(groups).toHaveLength(1);
    expect(groups[0].count).toBe(2);
  });

  it("does not dedup the same evidence across different companies", () => {
    const groups = buildStream([
      item({ id: "a", companyId: "co_a", evidenceKey: "ev" }),
      item({ id: "b", companyId: "co_b", evidenceKey: "ev" }),
    ]);
    expect(groups).toHaveLength(2);
  });
});

describe("buildStream — group (ADR 0087 dec. 1b)", () => {
  it("collapses same category+company into one group with a count and newest-first members", () => {
    const groups = buildStream([
      item({ id: "mid", evidenceKey: "e1", timestamp: "2026-07-05T00:00:00Z" }),
      item({ id: "new", evidenceKey: "e2", timestamp: "2026-07-09T00:00:00Z" }),
      item({ id: "old", evidenceKey: "e3", timestamp: "2026-07-01T00:00:00Z" }),
    ]);
    expect(groups).toHaveLength(1);
    expect(groups[0].count).toBe(3);
    expect(groups[0].members.map((m) => m.id)).toEqual(["new", "mid", "old"]);
    // Group timestamp is the newest member's.
    expect(groups[0].timestamp).toBe("2026-07-09T00:00:00Z");
  });

  it("keeps different categories for one company as separate groups", () => {
    const groups = buildStream([
      item({ id: "a", category: "autopilot", evidenceKey: "e1" }),
      item({ id: "b", category: "verify", evidenceKey: "e2" }),
    ]);
    expect(groups).toHaveLength(2);
    expect(new Set(groups.map((g) => g.category))).toEqual(new Set(["autopilot", "verify"]));
  });

  it("gives a group the most-severe member's severity", () => {
    const groups = buildStream([
      item({ id: "a", evidenceKey: "e1", severity: "routine" }),
      item({ id: "b", evidenceKey: "e2", severity: "urgent" }),
      item({ id: "c", evidenceKey: "e3", severity: "notable" }),
    ]);
    expect(groups[0].severity).toBe("urgent");
  });
});

describe("buildStream — rank (ADR 0087 dec. 1c)", () => {
  it("orders urgent → notable → routine regardless of recency", () => {
    const groups = buildStream([
      item({ id: "r", companyId: "co_r", severity: "routine", timestamp: "2026-07-20T00:00:00Z" }),
      item({ id: "u", companyId: "co_u", severity: "urgent", timestamp: "2026-07-01T00:00:00Z" }),
      item({ id: "n", companyId: "co_n", severity: "notable", timestamp: "2026-07-10T00:00:00Z" }),
    ]);
    expect(groups.map((g) => g.severity)).toEqual(["urgent", "notable", "routine"]);
  });

  it("orders newest-first within the same severity", () => {
    const groups = buildStream([
      item({ id: "old", companyId: "co_old", severity: "notable", timestamp: "2026-07-01T00:00:00Z" }),
      item({ id: "new", companyId: "co_new", severity: "notable", timestamp: "2026-07-20T00:00:00Z" }),
    ]);
    expect(groups.map((g) => g.companyId)).toEqual(["co_new", "co_old"]);
  });

  it("sorts groups deterministically when severity and timestamp tie", () => {
    const build = () =>
      buildStream([
        item({ id: "b", companyId: "co_b", timestamp: "2026-07-10T00:00:00Z" }),
        item({ id: "a", companyId: "co_a", timestamp: "2026-07-10T00:00:00Z" }),
      ]).map((g) => g.companyId);
    expect(build()).toEqual(build());
  });
});

describe("buildStream — edges", () => {
  it("returns an empty list for no items", () => {
    expect(buildStream([])).toEqual([]);
  });

  it("sorts null timestamps last, without crashing", () => {
    const groups = buildStream([
      item({ id: "n", companyId: "co_n", severity: "routine", timestamp: null }),
      item({ id: "d", companyId: "co_d", severity: "routine", timestamp: "2026-07-10T00:00:00Z" }),
    ]);
    expect(groups.map((g) => g.companyId)).toEqual(["co_d", "co_n"]);
  });

  it("stays stable and correct under a dense mix (grouping + dedup + rank together)", () => {
    const groups = buildStream([
      // co_a autopilot: 3 items, one a dedup repeat of e1
      item({ id: "a1", companyId: "co_a", category: "autopilot", evidenceKey: "e1", severity: "routine", timestamp: "2026-07-02T00:00:00Z" }),
      item({ id: "a1b", companyId: "co_a", category: "autopilot", evidenceKey: "e1", severity: "routine", timestamp: "2026-07-03T00:00:00Z" }),
      item({ id: "a2", companyId: "co_a", category: "autopilot", evidenceKey: "e2", severity: "routine", timestamp: "2026-07-04T00:00:00Z" }),
      // co_b attention urgent
      item({ id: "b1", companyId: "co_b", category: "attention", evidenceKey: "e3", severity: "urgent", timestamp: "2026-07-01T00:00:00Z" }),
      // co_c verify notable
      item({ id: "c1", companyId: "co_c", category: "verify", evidenceKey: "e4", severity: "notable", timestamp: "2026-07-05T00:00:00Z" }),
    ]);
    // co_a's e1 repeat deduped → the autopilot group has 2 members.
    const autopilot = groups.find((g) => g.companyId === "co_a");
    expect(autopilot?.count).toBe(2);
    // Rank: urgent (co_b) → notable (co_c) → routine (co_a).
    expect(groups.map((g) => g.companyId)).toEqual(["co_b", "co_c", "co_a"]);
  });
});

/** Build N single-item routine groups in one category, across distinct companies. */
function routineGroups(category: StreamCategory, n: number) {
  return buildStream(
    Array.from({ length: n }, (_, i) =>
      item({
        id: `${category}_${i}`,
        category,
        companyId: `co_${category}_${i}`,
        severity: "routine",
        evidenceKey: `${category}_${i}`,
        timestamp: `2026-07-${String(10 + i).padStart(2, "0")}T00:00:00Z`,
      }),
    ),
  );
}

describe("collapseRoutineAggregates — cross-company routine collapse (ADR 0087, mockup ×N spółek)", () => {
  it(`collapses more than ${ROUTINE_AGGREGATE_THRESHOLD} same-category routine rows into one aggregate`, () => {
    const rows = collapseRoutineAggregates(routineGroups("autopilot", 5));
    expect(rows).toHaveLength(1);
    const aggregate = rows[0];
    expect(aggregate.kind).toBe("aggregate");
    if (aggregate.kind !== "aggregate") throw new Error("expected aggregate");
    // count = number of companies collapsed.
    expect(aggregate.count).toBe(5);
    expect(aggregate.members).toHaveLength(5);
    // Members are newest-first (the ranked recency order).
    expect(aggregate.members[0].timestamp).toBe("2026-07-14T00:00:00Z");
    expect(aggregate.members[4].timestamp).toBe("2026-07-10T00:00:00Z");
  });

  it("does not collapse at or below the threshold", () => {
    const rows = collapseRoutineAggregates(routineGroups("autopilot", ROUTINE_AGGREGATE_THRESHOLD));
    expect(rows).toHaveLength(ROUTINE_AGGREGATE_THRESHOLD);
    expect(rows.every((row) => row.kind === "group")).toBe(true);
  });

  it("collapses exactly at threshold + 1 (boundary)", () => {
    const rows = collapseRoutineAggregates(
      routineGroups("autopilot", ROUTINE_AGGREGATE_THRESHOLD + 1),
    );
    expect(rows).toHaveLength(1);
    expect(rows[0].kind).toBe("aggregate");
  });

  it("never collapses urgent or notable rows cross-company, even above the threshold", () => {
    const groups = buildStream(
      Array.from({ length: 5 }, (_, i) =>
        item({
          id: `u_${i}`,
          category: "attention",
          companyId: `co_${i}`,
          severity: "urgent",
          evidenceKey: `u_${i}`,
        }),
      ),
    );
    const rows = collapseRoutineAggregates(groups);
    expect(rows).toHaveLength(5);
    expect(rows.every((row) => row.kind === "group")).toBe(true);
  });

  it("collapses only per-category, leaving other-category and non-routine rows intact", () => {
    const groups = buildStream([
      // 4 routine autopilot rows across 4 companies → collapse.
      ...Array.from({ length: 4 }, (_, i) =>
        item({ id: `a_${i}`, category: "autopilot", companyId: `co_a_${i}`, severity: "routine", evidenceKey: `a_${i}` }),
      ),
      // 2 routine verify rows → stay as groups (below threshold).
      item({ id: "v1", category: "verify", companyId: "co_v_1", severity: "routine", evidenceKey: "v1" }),
      item({ id: "v2", category: "verify", companyId: "co_v_2", severity: "routine", evidenceKey: "v2" }),
      // 1 urgent row → leads, never collapsed.
      item({ id: "u1", category: "attention", companyId: "co_u_1", severity: "urgent", evidenceKey: "u1", timestamp: "2026-08-01T00:00:00Z" }),
    ]);
    const rows = collapseRoutineAggregates(groups);
    const kinds = rows.map((row) => row.kind);
    // 1 urgent group + 1 autopilot aggregate + 2 verify groups = 4 rows.
    expect(rows).toHaveLength(4);
    expect(kinds.filter((k) => k === "aggregate")).toHaveLength(1);
    // The urgent group still leads (aggregate is routine, ranks after it).
    expect(rows[0].kind).toBe("group");
    expect((rows[0] as { severity: Severity }).severity).toBe("urgent");
  });

  it("returns the groups unchanged when nothing crosses the threshold", () => {
    const groups = routineGroups("autopilot", 2);
    expect(collapseRoutineAggregates(groups)).toBe(groups);
  });
});

/** Build N single-item groups of one severity sharing one cause (subCategory), across distinct companies. */
function causeGroups(subCategory: string, n: number, severity: Severity = "urgent") {
  return buildStream(
    Array.from({ length: n }, (_, i) =>
      item({
        id: `${severity}_${subCategory}_${i}`,
        category: "attention",
        companyId: `co_${severity}_${subCategory}_${i}`,
        severity,
        subCategory,
        evidenceKey: `${severity}_${subCategory}_${i}`,
        timestamp: `2026-07-${String(10 + i).padStart(2, "0")}T00:00:00Z`,
      }),
    ),
  );
}

describe("collapseCauseAggregates — urgent collapse (ADR 0087 amendment 2026-07-23)", () => {
  it(`collapses ${URGENT_AGGREGATE_MIN}+ same-cause urgent rows into one leading urgent aggregate`, () => {
    // The live-checkpoint P1: 14 companies each with one source_reconciliation
    // event → a single leading PILNE aggregate ×14, not 14 separate rows.
    const rows = collapseCauseAggregates(causeGroups("source_reconciliation", 14));
    expect(rows).toHaveLength(1);
    const aggregate = rows[0];
    expect(aggregate.kind).toBe("aggregate");
    if (aggregate.kind !== "aggregate") throw new Error("expected aggregate");
    expect(aggregate.severity).toBe("urgent");
    expect(aggregate.count).toBe(14);
    expect(aggregate.members).toHaveLength(14);
    // Ranked by its newest member (newest-first).
    expect(aggregate.timestamp).toBe("2026-07-23T00:00:00Z");
    expect(aggregate.members[0].timestamp).toBe("2026-07-23T00:00:00Z");
  });

  it("keeps DIFFERENT urgent causes as separate aggregates, never merged", () => {
    const rows = collapseCauseAggregates([
      ...causeGroups("source_reconciliation", 2),
      ...causeGroups("insider_transaction", 2),
    ]);
    const aggregates = rows.filter((row) => row.kind === "aggregate");
    expect(aggregates).toHaveLength(2);
    expect(new Set(aggregates.map((a) => a.key))).toEqual(
      new Set(["aggregate::urgent::source_reconciliation", "aggregate::urgent::insider_transaction"]),
    );
  });

  it("leaves a lone urgent-of-a-cause (and a single of each different cause) as plain rows", () => {
    const rows = collapseCauseAggregates([
      ...causeGroups("source_reconciliation", 1),
      ...causeGroups("insider_transaction", 1),
    ]);
    expect(rows).toHaveLength(2);
    expect(rows.every((row) => row.kind === "group")).toBe(true);
  });

  it("the urgent aggregate keeps urgent severity and never falls into the routine-collapse path", () => {
    // The full pipeline: urgent recon wall + a routine autopilot wall coexist as
    // one urgent aggregate (leading) and one routine aggregate (trailing).
    const groups = buildStream([
      ...Array.from({ length: 5 }, (_, i) =>
        item({
          id: `rec_${i}`,
          category: "attention",
          companyId: `co_rec_${i}`,
          severity: "urgent",
          subCategory: "source_reconciliation",
          evidenceKey: `rec_${i}`,
          timestamp: `2026-08-${String(10 + i).padStart(2, "0")}T00:00:00Z`,
        }),
      ),
      ...Array.from({ length: ROUTINE_AGGREGATE_THRESHOLD + 1 }, (_, i) =>
        item({
          id: `ap_${i}`,
          category: "autopilot",
          companyId: `co_ap_${i}`,
          severity: "routine",
          evidenceKey: `ap_${i}`,
        }),
      ),
    ]);
    const rows = collapseCauseAggregates(collapseRoutineAggregates(groups));
    const aggregates = rows.filter((row) => row.kind === "aggregate");
    expect(aggregates).toHaveLength(2);
    // The urgent aggregate leads and stays urgent.
    expect(rows[0].kind).toBe("aggregate");
    expect(rows[0].severity).toBe("urgent");
    // The routine aggregate is separate and stays routine.
    const routineAgg = aggregates.find((a) => a.severity === "routine");
    expect(routineAgg).toBeDefined();
    expect(routineAgg?.count).toBe(ROUTINE_AGGREGATE_THRESHOLD + 1);
  });

  it("returns the rows unchanged when no cause reaches the minimum", () => {
    const rows = causeGroups("source_reconciliation", 1);
    expect(collapseCauseAggregates(rows)).toBe(rows);
  });
});

describe("collapseCauseAggregates — notable collapse (ADR 0087 amendment 2026-07-23 follow-up)", () => {
  it(`collapses MORE THAN ${NOTABLE_AGGREGATE_THRESHOLD} same-cause notable rows into one notable aggregate`, () => {
    // 14 events that aged out of urgent (now notable) sharing one cause → one
    // NOTABLE aggregate ×14 — a wall that changed color is still folded to a line.
    const rows = collapseCauseAggregates(
      causeGroups("source_reconciliation", 14, "notable"),
    );
    expect(rows).toHaveLength(1);
    const aggregate = rows[0];
    expect(aggregate.kind).toBe("aggregate");
    if (aggregate.kind !== "aggregate") throw new Error("expected aggregate");
    expect(aggregate.severity).toBe("notable");
    expect(aggregate.count).toBe(14);
    expect(aggregate.key).toBe("aggregate::notable::source_reconciliation");
  });

  it("keeps 3-or-fewer same-cause notable rows as their own separate rows (routine wall semantics)", () => {
    const rows = collapseCauseAggregates(
      causeGroups("source_reconciliation", NOTABLE_AGGREGATE_THRESHOLD, "notable"),
    );
    expect(rows).toHaveLength(NOTABLE_AGGREGATE_THRESHOLD);
    expect(rows.every((row) => row.kind === "group")).toBe(true);
  });

  it("collapses different notable causes per-cause, never merging them (a price-alert day)", () => {
    const rows = collapseCauseAggregates([
      ...causeGroups("price_enters_range", 4, "notable"),
      ...causeGroups("price_week52_low", 4, "notable"),
    ]);
    const aggregates = rows.filter((row) => row.kind === "aggregate");
    expect(aggregates).toHaveLength(2);
    expect(new Set(aggregates.map((a) => a.key))).toEqual(
      new Set(["aggregate::notable::price_enters_range", "aggregate::notable::price_week52_low"]),
    );
  });

  it("partitions by severity first: urgent + notable of the SAME cause never merge into one aggregate", () => {
    // 2 fresh (urgent) + 4 aged (notable) reconciliations, same cause → a leading
    // urgent aggregate AND a separate notable aggregate, never one merged row.
    const rows = collapseCauseAggregates([
      ...causeGroups("source_reconciliation", 2, "urgent"),
      ...causeGroups("source_reconciliation", 4, "notable"),
    ]);
    const aggregates = rows.filter((row) => row.kind === "aggregate");
    expect(aggregates).toHaveLength(2);
    const urgentAgg = aggregates.find((a) => a.severity === "urgent");
    const notableAgg = aggregates.find((a) => a.severity === "notable");
    expect(urgentAgg?.count).toBe(2);
    expect(notableAgg?.count).toBe(4);
    // The urgent aggregate ranks above the notable one.
    expect(rows.indexOf(urgentAgg!)).toBeLessThan(rows.indexOf(notableAgg!));
  });
});

/**
 * The ONE frontend-side severity entry (ADR 0087 dec. 2, product-spec §Attention
 * Routing severity table row marked FE-side): a claim OVERDUE for verification
 * escalates to `notable` so it ranks above routine rows (and below urgent); a
 * claim merely due-but-not-yet-overdue stays `routine`. Every other verify/
 * changed/upcoming input remains category-level routine, and a claim raises NO
 * toast — it is not an attention event.
 */
describe("verifyClaimSeverity — overdue-claim escalation (ADR 0087, product-spec §Attention Routing FE-side row)", () => {
  it("escalates an overdue claim to notable", () => {
    expect(verifyClaimSeverity("overdue")).toBe("notable");
  });

  it("keeps a due-but-not-yet-overdue claim routine", () => {
    expect(verifyClaimSeverity("due")).toBe("routine");
  });

  it("orders an overdue-claim (notable) verify row above routine and below urgent, and never folds it into a routine aggregate", () => {
    // Build a stream where the overdue claim is the only notable, alongside an
    // urgent attention row and MORE THAN the threshold of routine autopilot rows
    // (which collapse into an aggregate). The escalated verify row must sit
    // between them and survive as its own group, never inside the aggregate.
    const groups = buildStream([
      item({ id: "u", companyId: "co_u", category: "attention", severity: "urgent", evidenceKey: "u" }),
      item({
        id: "claim",
        companyId: "co_claim",
        category: "verify",
        severity: verifyClaimSeverity("overdue"),
        evidenceKey: "claim",
        timestamp: "2026-07-15T00:00:00Z",
      }),
      ...Array.from({ length: ROUTINE_AGGREGATE_THRESHOLD + 1 }, (_, i) =>
        item({
          id: `ap_${i}`,
          companyId: `co_ap_${i}`,
          category: "autopilot",
          severity: "routine",
          evidenceKey: `ap_${i}`,
        }),
      ),
    ]);
    const rows = collapseRoutineAggregates(groups);
    // Verify row is its own group (notable), never an aggregate member.
    const verifyRow = rows.find(
      (row) => row.kind === "group" && row.category === "verify",
    );
    expect(verifyRow?.severity).toBe("notable");
    // Ranking: urgent → notable(verify) → routine(aggregate).
    const severities = rows.map((row) => row.severity);
    expect(severities.indexOf("urgent")).toBeLessThan(severities.indexOf("notable"));
    expect(severities.indexOf("notable")).toBeLessThan(severities.lastIndexOf("routine"));
    // The routine autopilot rows collapsed; the notable verify row did NOT.
    expect(rows.some((row) => row.kind === "aggregate")).toBe(true);
    expect(rows.filter((row) => row.category === "verify" && row.kind === "aggregate")).toHaveLength(0);
  });
});
