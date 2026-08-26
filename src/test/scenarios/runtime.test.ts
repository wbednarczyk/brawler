import { describe, expect, it } from "vitest";

import { companyId, COMPANY_SPECS, makeEvent, makeManagementClaim } from "./entities";
import { createMockRuntime, knownCommands, READ_COMMANDS } from "./runtime";
import { buildScenario } from "./scenarios";
import type { CompanyView } from "../../api/generated/CompanyView";

/** True for a populated array, a non-empty bucketed read model, or any object. */
function isNonEmpty(value: unknown): boolean {
  if (Array.isArray(value)) return value.length > 0;
  if (value && typeof value === "object") {
    const buckets = Object.values(value).filter(Array.isArray);
    if (buckets.length > 0) return buckets.some((bucket) => (bucket as unknown[]).length > 0);
    return Object.keys(value).length > 0;
  }
  return value !== null && value !== undefined;
}

describe("mock runtime — completeness guardrail (ADR 0048 Keystone B/C)", () => {
  // Every whole-collection read must return non-trivial data under minimal AND
  // rich, so a new feature that forgets to seed its entity fails this gate
  // rather than silently shipping an empty screen.
  for (const scenario of ["minimal", "rich"] as const) {
    for (const command of READ_COMMANDS) {
      it(`${command} returns data under ${scenario}`, async () => {
        const runtime = createMockRuntime(scenario);
        const result = await runtime.invoke(command, {});
        expect(isNonEmpty(result), `${command} was empty under ${scenario}`).toBe(true);
      });
    }
  }

  it("empty scenario keeps singletons but empties collections", async () => {
    const runtime = createMockRuntime("empty");
    expect(await runtime.invoke("list_companies", {})).toEqual([]);
    expect(await runtime.invoke("list_feed_items", {})).toEqual([]);
    const settings = await runtime.invoke("get_settings", {});
    expect(settings).toBeTruthy();
  });
});

describe("mock runtime — command coverage", () => {
  it("handles every command the api layer can invoke", () => {
    // The authoritative contract surface (src/api/*.ts callCommand names). If a
    // new command is added to the api layer, add its handler here too.
    const commands = knownCommands();
    expect(commands.length).toBeGreaterThanOrEqual(150);
    // Spot-check representative commands across every domain are routed.
    for (const command of [
      "health",
      "list_companies",
      "create_watchlist",
      "update_feed_item_state",
      "list_research_evidence",
      "set_claim_verdict",
      "create_financial_fact",
      "evaluate_framework",
      "refresh_sources",
      "search",
    ]) {
      expect(commands).toContain(command);
    }
  });

  it("rejects an unknown command (the add-a-case signal)", async () => {
    const runtime = createMockRuntime("minimal");
    await expect(runtime.invoke("not_a_real_command", {})).rejects.toThrow(/Unhandled mock command/);
  });

  it("routes structured-first fundamentals provenance (ADR 0061)", async () => {
    const runtime = createMockRuntime("minimal");
    // Seed provenance for two facts (the optional store the badges read).
    runtime.data = {
      ...runtime.data,
      factProvenance: [
        { factId: "f1", sourceTier: "esef", validationStatus: "passed", driftJson: null, citation: "Assets" },
        { factId: "f2", sourceTier: "pdf", validationStatus: "flagged", driftJson: "{}", citation: null },
      ],
    } as typeof runtime.data;

    const forF1 = (await runtime.invoke("list_fact_provenance", { input: { factIds: ["f1"] } })) as Array<{
      factId: string;
      sourceTier: string;
    }>;
    expect(forF1).toHaveLength(1);
    expect(forF1[0]).toMatchObject({ factId: "f1", sourceTier: "esef" });

    // The flagged-FACTS read JOINs provenance → fact → period → definition (epic
    // #229 T5), so a provenance row whose fact is absent yields nothing — exactly
    // as the real backend's join does. Seed the fact side for the flagged row.
    runtime.data = {
      ...runtime.data,
      financialPeriods: [
        {
          id: "p1",
          companyId: "c1",
          fiscalYear: 2026,
          periodType: "FY",
          periodEndDate: "2026-12-31",
          reportEvidenceRef: null,
          createdAt: "2026-01-01T00:00:00Z",
          updatedAt: "2026-01-01T00:00:00Z",
        },
      ],
      financialFacts: [
        {
          ...runtime.data.financialFacts[0],
          id: "f2",
          companyId: "c1",
          periodId: "p1",
          definitionId: "kpi_def_revenue",
          valueNumeric: "1050000000",
          currency: "PLN",
        },
      ],
    } as typeof runtime.data;

    const flagged = (await runtime.invoke("list_flagged_fact_provenance", {})) as Array<{
      factId: string;
      metricKey: string;
      valueNumeric: string;
      fiscalYear: number;
    }>;
    expect(flagged.map((p) => p.factId)).toEqual(["f2"]);
    expect(flagged[0]).toMatchObject({ metricKey: "revenue", valueNumeric: "1050000000", fiscalYear: 2026 });

    // Scoping to another company excludes it (the Coverage panel's read).
    const scoped = (await runtime.invoke("list_flagged_fact_provenance", {
      input: { companyId: "other" },
    })) as unknown[];
    expect(scoped).toEqual([]);

    const summary = (await runtime.invoke("run_structured_extraction", {
      input: { companyId: "c1", reportDocumentId: "d1", fiscalYear: 2026, periodType: "FY", periodEnd: "2026-03-31" },
    })) as { acceptance: string };
    expect(summary.acceptance).toBe("accepted");
  });

  it("evaluate_framework mints a fresh snapshot id per run (mirrors backend unique ids — a repeated id duplicates React keys in the history list)", async () => {
    const runtime = createMockRuntime("minimal");
    const base = {
      frameworkId: "fw1",
      frameworkVersion: 1,
      companyId: "c1",
      periodId: null,
      passCount: 1,
      partialCount: 0,
      failCount: 0,
      unavailableCount: 0,
      engineVersion: "v1",
      createdAt: "2026-06-01T00:00:00Z",
      results: [],
    };
    runtime.data.frameworkEvaluations = [
      { ...base, id: "evaluation_seeded" },
    ] as typeof runtime.data.frameworkEvaluations;

    const first = (await runtime.invoke("evaluate_framework", {
      input: { companyId: "c1", frameworkId: "fw1" },
    })) as { id: string };
    const second = (await runtime.invoke("evaluate_framework", {
      input: { companyId: "c1", frameworkId: "fw1" },
    })) as { id: string };

    const ids = runtime.data.frameworkEvaluations.map((e) => e.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(first.id).not.toBe("evaluation_seeded");
    expect(second.id).not.toBe(first.id);
    // The minted snapshot lands in the store (list_framework_evaluations sees it).
    expect(ids).toContain(first.id);
    expect(ids).toContain(second.id);
  });
});

describe("mock runtime — re-render safety (new collection reference per mutation)", () => {
  // A mutating handler MUST reassign its store collection (`d.x = [...]` / `.map`),
  // never mutate an entity in place — an in-place mutation keeps the same array
  // reference, so React bails on re-render and clickable journeys silently break.
  // This gate fails if any listed mutation leaves its store field reference-equal.
  // (See docs/testing.md → "mock runtime conventions".)
  const cases: { command: string; field: keyof ReturnType<typeof createMockRuntime>["data"]; args: (data: ReturnType<typeof createMockRuntime>["data"]) => Record<string, unknown> }[] = [
    { command: "create_watchlist", field: "watchlists", args: () => ({ input: { name: "New", description: null } }) },
    { command: "rename_watchlist", field: "watchlists", args: (d) => ({ input: { id: d.watchlists[0].id, name: "Renamed", description: null } }) },
    { command: "delete_watchlist", field: "watchlists", args: (d) => ({ watchlistId: d.watchlists[0].id }) },
    { command: "update_feed_item_state", field: "feedItems", args: (d) => ({ input: { id: d.feedItems[0].id, read: true, saved: false } }) },
    { command: "create_notebook_entry", field: "notebookEntries", args: (d) => ({ input: { companyId: d.companies[0].id, title: "N", body: "b", bodyFormat: "markdown", tags: [], kind: "note", claimStatus: null, eventDate: null, followUpAfter: null, followUpDate: null, origins: [] } }) },
    { command: "update_notebook_entry", field: "notebookEntries", args: (d) => ({ input: { id: d.notebookEntries[0].id, title: "Edited", body: "b", tags: [], kind: "note", claimStatus: null, eventDate: null, followUpAfter: null, followUpDate: null } }) },
    { command: "delete_notebook_entry", field: "notebookEntries", args: (d) => ({ id: d.notebookEntries[0].id }) },
    { command: "confirm_company_signal", field: "signals", args: (d) => ({ input: { id: d.signals[0].id } }) },
    { command: "reject_company_signal", field: "signals", args: (d) => ({ input: { id: d.signals[0].id } }) },
    { command: "create_research_question", field: "researchQuestions", args: (d) => ({ input: { scopeType: "company", scopeId: d.companies[0].id, title: "Q", body: "" } }) },
    { command: "update_research_question", field: "researchQuestions", args: (d) => ({ input: { id: d.researchQuestions[0].id, title: "Edited" } }) },
    { command: "delete_research_question", field: "researchQuestions", args: (d) => ({ id: d.researchQuestions[0].id }) },
    { command: "update_research_reminder", field: "researchReminders", args: (d) => ({ input: { id: d.researchReminders[0].id, status: "completed" } }) },
    { command: "update_management_claim", field: "managementClaims", args: (d) => ({ input: { id: d.managementClaims[0].id, statement: "Edited" } }) },
    { command: "set_claim_verdict", field: "managementClaims", args: (d) => ({ input: { claimId: d.managementClaims[0].id, status: "delivered" } }) },
    { command: "update_financial_fact", field: "financialFacts", args: (d) => ({ input: { id: d.financialFacts[0].id, valueNumeric: "999" } }) },
    { command: "update_kpi_relevance", field: "kpiRelevance", args: (d) => ({ input: { id: d.kpiRelevance[0].id, status: "muted" } }) },
    { command: "update_quality_framework", field: "qualityFrameworks", args: (d) => ({ input: { id: d.qualityFrameworks[0].id, name: "Edited", description: null } }) },
    { command: "set_source_adapter_enabled", field: "sourceAdapters", args: (d) => ({ input: { adapterId: d.sourceAdapters.find((s) => s.userConfigurable)?.id, enabled: false } }) },
    { command: "mark_report_processed", field: "reportPreparations", args: (d) => ({ input: { companyId: d.reportPreparations[0].companyId, eventKey: d.reportPreparations[0].eventKey } }) },
  ];

  for (const { command, field, args } of cases) {
    it(`${command} reassigns d.${String(field)} (new reference)`, async () => {
      const runtime = createMockRuntime("rich");
      const before = runtime.data[field];
      await runtime.invoke(command, args(runtime.data));
      expect(runtime.data[field], `${command} mutated d.${String(field)} in place`).not.toBe(before);
    });
  }
});

describe("mock runtime — round-trip mutations", () => {
  it("create → list reflects the new watchlist", async () => {
    const runtime = createMockRuntime("minimal");
    const beforeLen = ((await runtime.invoke("list_watchlists", {})) as unknown[]).length;
    const created = (await runtime.invoke("create_watchlist", {
      input: { name: "New list", description: null },
    })) as { id: string; name: string };
    expect(created.name).toBe("New list");
    const after = (await runtime.invoke("list_watchlists", {})) as { id: string }[];
    expect(after).toHaveLength(beforeLen + 1);
    expect(after.some((w) => w.id === created.id)).toBe(true);
  });

  it("update_feed_item_state flips unread/saved", async () => {
    const runtime = createMockRuntime("minimal");
    const items = (await runtime.invoke("list_feed_items", {})) as { id: string }[];
    const target = items[0];
    const updated = (await runtime.invoke("update_feed_item_state", {
      input: { id: target.id, read: true, saved: true },
    })) as { id: string; unread: boolean; saved: boolean };
    expect(updated.id).toBe(target.id);
    expect(updated.unread).toBe(false);
    expect(updated.saved).toBe(true);
  });

  it("delete_research_question removes it", async () => {
    const runtime = createMockRuntime("minimal");
    const before = (await runtime.invoke("list_research_questions", {})) as { id: string }[];
    const target = before[0];
    await runtime.invoke("delete_research_question", { id: target.id });
    const after = (await runtime.invoke("list_research_questions", {})) as { id: string }[];
    expect(after.some((q) => q.id === target.id)).toBe(false);
  });

  it("update_framework_criterion falls back to existing kind/guidance when omitted (F9 resolver)", async () => {
    const runtime = createMockRuntime("minimal");
    const created = (await runtime.invoke("create_framework_criterion", {
      input: {
        frameworkId: "framework_sample_moat",
        label: "Wide moat",
        expression: "",
        weight: null,
        partialBand: null,
        kind: "qualitative",
        assessmentGuidance: "Assess durable competitive advantage.",
      },
    })) as { id: string; kind: string; assessmentGuidance: string | null };
    expect(created.kind).toBe("qualitative");
    // Update ONLY the label — kind and guidance are omitted, so the shared
    // resolver must fall back to the existing qualitative row rather than reset
    // the criterion to quantitative with an empty expression.
    const updated = (await runtime.invoke("update_framework_criterion", {
      input: { id: created.id, label: "Wide durable moat" },
    })) as { id: string; label: string; kind: string; expression: string; assessmentGuidance: string | null };
    expect(updated.label).toBe("Wide durable moat");
    expect(updated.kind).toBe("qualitative");
    expect(updated.expression).toBe("");
    expect(updated.assessmentGuidance).toBe("Assess durable competitive advantage.");
  });

  it("reset restores a fresh store and clears mutations", async () => {
    const runtime = createMockRuntime("minimal");
    await runtime.invoke("create_watchlist", { input: { name: "Temp", description: null } });
    runtime.reset();
    const watchlists = (await runtime.invoke("list_watchlists", {})) as unknown[];
    expect(watchlists).toHaveLength(1);
  });

  it("reset clears BOTH mcp tokens — scenarios must not inherit credentials", async () => {
    // ADR 0099: the acquisition token is context state like the primary; a
    // reset that leaked it would make token-status tests order-dependent.
    const runtime = createMockRuntime("minimal");
    await runtime.invoke("regenerate_mcp_token", {});
    await runtime.invoke("regenerate_kpi_acquisition_token", {});
    runtime.reset();
    const primary = (await runtime.invoke("mcp_token_status", {})) as {
      configured: boolean;
    };
    const acquisition = (await runtime.invoke(
      "kpi_acquisition_token_status",
      {},
    )) as { configured: boolean };
    expect(primary.configured).toBe(false);
    expect(acquisition.configured).toBe(false);
  });

  it("reset to a different scenario swaps the dataset", async () => {
    const runtime = createMockRuntime("minimal");
    runtime.reset("rich");
    const companies = (await runtime.invoke("list_companies", {})) as unknown[];
    expect(companies).toHaveLength(28);
    expect(runtime.scenario).toBe("rich");
  });

  it("each runtime is isolated (no shared store)", async () => {
    const a = createMockRuntime("minimal");
    const b = createMockRuntime("minimal");
    await a.invoke("delete_company", { companyId: buildScenario("minimal").companies[0].id });
    const bCompanies = (await b.invoke("list_companies", {})) as unknown[];
    expect(bCompanies).toHaveLength(4);
  });
});

// Deliverable A (Radicle 5be14c9, epic 0db7a7a) — the minimal invocation-
// settlement seam Q2's controlled-async `reject(id, error)` delegates to.
describe("mock runtime — failure-injection seam (Radicle 5be14c9)", () => {
  it("failNext rejects the NEXT invocation of the selected command with the CommandError envelope (ADR 0070)", async () => {
    const runtime = createMockRuntime("minimal");
    runtime.failNext("list_companies", { code: "internal", message: "sample seam failure" });
    await expect(runtime.invoke("list_companies", {})).rejects.toMatchObject({
      code: "internal",
      message: "sample seam failure",
    });
  });

  it("failNext is one-shot: the invocation after the rejected one runs the real handler", async () => {
    const runtime = createMockRuntime("minimal");
    runtime.failNext("list_companies", { code: "internal", message: "sample seam failure" });
    await expect(runtime.invoke("list_companies", {})).rejects.toBeTruthy();
    const companies = await runtime.invoke("list_companies", {});
    expect(Array.isArray(companies)).toBe(true);
    expect((companies as unknown[]).length).toBeGreaterThan(0);
  });

  it("failNext only rejects the SELECTED command, not others", async () => {
    const runtime = createMockRuntime("minimal");
    runtime.failNext("list_companies", { code: "internal", message: "sample seam failure" });
    const watchlists = await runtime.invoke("list_watchlists", {});
    expect(Array.isArray(watchlists)).toBe(true);
  });

  it("reset clears every queued failure so no promise leaks across tests", async () => {
    const runtime = createMockRuntime("minimal");
    runtime.failNext("list_companies", { code: "internal", message: "sample seam failure" });
    runtime.reset();
    const companies = await runtime.invoke("list_companies", {});
    expect(Array.isArray(companies)).toBe(true);
  });
});

// Epic #40 S1 (ADR 0091) — persistent chaos rules beside the one-shot queue.
describe("mock runtime — persistent chaos rules (epic #40 S1)", () => {
  it("chaos(command, error) fails the command EVERY time, not once", async () => {
    const runtime = createMockRuntime("minimal");
    runtime.chaos("list_companies", { code: "internal", message: "chaos rule" });
    await expect(runtime.invoke("list_companies", {})).rejects.toMatchObject({
      code: "internal",
      message: "chaos rule",
    });
    await expect(runtime.invoke("list_companies", {})).rejects.toMatchObject({
      code: "internal",
      message: "chaos rule",
    });
  });

  it("chaos only fails the SELECTED command, not others", async () => {
    const runtime = createMockRuntime("minimal");
    runtime.chaos("list_companies", { code: "internal", message: "chaos rule" });
    const watchlists = await runtime.invoke("list_watchlists", {});
    expect(Array.isArray(watchlists)).toBe(true);
  });

  it("a one-shot failNext wins over an active chaos rule and is consumed; the chaos rule keeps failing after", async () => {
    const runtime = createMockRuntime("minimal");
    runtime.chaos("list_companies", { code: "internal", message: "chaos rule" });
    runtime.failNext("list_companies", { code: "conflict", message: "one-shot first" });
    await expect(runtime.invoke("list_companies", {})).rejects.toMatchObject({
      code: "conflict",
      message: "one-shot first",
    });
    await expect(runtime.invoke("list_companies", {})).rejects.toMatchObject({
      code: "internal",
      message: "chaos rule",
    });
  });

  it("clearChaos() restores normal behavior without touching the store", async () => {
    const runtime = createMockRuntime("minimal");
    runtime.chaos("list_companies", { code: "internal", message: "chaos rule" });
    await expect(runtime.invoke("list_companies", {})).rejects.toBeTruthy();
    runtime.clearChaos();
    const companies = await runtime.invoke("list_companies", {});
    expect(Array.isArray(companies)).toBe(true);
    expect((companies as unknown[]).length).toBeGreaterThan(0);
  });

  it("reset() clears chaos rules too — no failure leaks across tests", async () => {
    const runtime = createMockRuntime("minimal");
    runtime.chaos("list_companies", { code: "internal", message: "chaos rule" });
    runtime.reset();
    const companies = await runtime.invoke("list_companies", {});
    expect(Array.isArray(companies)).toBe(true);
    expect((companies as unknown[]).length).toBeGreaterThan(0);
  });
});

// Q2 controlled-async wired end-to-end on a real MockRuntime (Radicle a9992e2).
describe("mock runtime — controlled async (Radicle a9992e2)", () => {
  it("holds a real read command, then releases it with the real handler result", async () => {
    const runtime = createMockRuntime("minimal");
    const id = runtime.controls.hold({ command: "list_companies" });
    const promise = runtime.invoke("list_companies", {});
    expect(runtime.controls.pending()).toHaveLength(1);
    runtime.controls.release(id);
    const companies = await promise;
    expect(Array.isArray(companies)).toBe(true);
    expect((companies as unknown[]).length).toBeGreaterThan(0);
  });

  it("reject(id, CommandError) on a held command settles via the failNext seam (ADR 0070 envelope)", async () => {
    const runtime = createMockRuntime("minimal");
    const id = runtime.controls.hold({ command: "create_watchlist" });
    const promise = runtime.invoke("create_watchlist", { input: { name: "Held", description: null } });
    runtime.controls.reject(id, { code: "conflict", message: "held rejection" });
    await expect(promise).rejects.toMatchObject({ code: "conflict", message: "held rejection" });
  });

  it("reset() clears held controls too — no promise leaks across a scenario reset", async () => {
    const runtime = createMockRuntime("minimal");
    runtime.controls.hold({ command: "list_companies" });
    const promise = runtime.invoke("list_companies", {});
    runtime.reset();
    await expect(promise).rejects.toThrow(/reset/i);
    expect(runtime.controls.pending()).toEqual([]);
  });

  it("two runtimes' controlled-async state is isolated — holding on one never affects the other", async () => {
    const a = createMockRuntime("minimal");
    const b = createMockRuntime("minimal");
    a.controls.hold({ command: "list_companies" });
    void a.invoke("list_companies", {});
    expect(a.controls.pending()).toHaveLength(1);
    expect(b.controls.pending()).toEqual([]);
    const bCompanies = await b.invoke("list_companies", {});
    expect(Array.isArray(bCompanies)).toBe(true);
  });
});

// get_company_view mock fixes (sol-review finding 7, F3a S1a). These bugs need
// domain data (management claims, company events, feed items, analyst
// recommendations) the shared fidelity corpus can't seed — mock_fidelity.rs
// (out of this slice's ownership) has no write commands for those tables — so
// they're pinned directly against the mock runtime here instead. Rust's own
// counters/price/recommendations behavior is pinned by company_view_tests.rs.
// The price-delta zero-division guard (also fixed in this pass) has no case
// here: the mock's own `get_price_context` handler always returns an empty
// `history: []` regardless of seeding (runtime.ts, "get_price_context"), so
// that branch of `get_company_view`'s price section is unreachable through
// `createMockRuntime` today — the guard mirrors Rust's `compute_price`
// semantics defensively for whenever that handler grows real history.
describe("get_company_view mock — sol-review finding 7 fixes", () => {
  it("throws the Rust-shaped error for an unknown company instead of a blank identity", async () => {
    const runtime = createMockRuntime("minimal");
    await expect(runtime.invoke("get_company_view", { companyId: "company_does_not_exist" })).rejects.toThrow(
      "no tracked company for id company_does_not_exist",
    );
  });

  it("counts open management claims and upcoming scheduled events", async () => {
    const runtime = createMockRuntime("minimal");
    const spec = COMPANY_SPECS[0];
    const cid = companyId(spec);
    expect(runtime.data.companies.some((c) => c.id === cid), "fixture company present").toBe(true);
    runtime.data.managementClaims = [makeManagementClaim(spec)]; // status: "pending"
    runtime.data.events = [makeEvent(spec)]; // eventDate 2026-06-20, status: "scheduled" — within SAMPLE_NOW's 30d window

    const view = (await runtime.invoke("get_company_view", { companyId: cid })) as CompanyView;
    expect(view.counters?.claims.open).toBe(1);
    expect(view.counters?.events.upcoming).toBe(1);
  });

  it("feed strip is the newest 6 official-report/public-media items, publishedAt DESC then id DESC", async () => {
    const runtime = createMockRuntime("minimal");
    const spec = COMPANY_SPECS[0];
    const cid = companyId(spec);
    const company = runtime.data.companies.find((c) => c.id === cid);
    if (!company) throw new Error("fixture company missing");

    const item = (id: string, publishedAt: string, type: string) => ({
      id,
      company: company.qualifiedTicker,
      type,
      source: "GPW ESPI/EBI",
      time: "Today 09:12",
      title: id,
      unread: false,
      saved: false,
      sourceUrl: `https://example.test/${id}`,
      language: "pl",
      publishedAt,
      fetchedAt: publishedAt,
      attribution: "GPW",
      summary: id,
      bodyText: id,
      attachments: [],
      presentationKind: "filing" as const,
    });
    runtime.data.feedItems = [
      item("f1", "2026-05-01T00:00:00Z", "Official report"),
      item("f2", "2026-05-02T00:00:00Z", "Public media"),
      item("f3", "2026-05-03T00:00:00Z", "Official report"),
      item("f4", "2026-05-04T00:00:00Z", "Public media"),
      item("f5", "2026-05-05T00:00:00Z", "Official report"),
      item("f6", "2026-05-06T00:00:00Z", "Public media"),
      item("f7", "2026-05-07T00:00:00Z", "Official report"),
      // Excluded item_type — never counted toward the cap.
      item("fx", "2026-05-08T00:00:00Z", "News"),
    ];

    const view = (await runtime.invoke("get_company_view", { companyId: cid })) as CompanyView;
    expect(view.feed.map((f) => f.feedItemId)).toEqual(["f7", "f6", "f5", "f4", "f3", "f2"]);
  });

  it("recommendations are capped at 3, newest first, matching Rust's RECOMMENDATIONS_LIMIT", async () => {
    const runtime = createMockRuntime("minimal");
    const spec = COMPANY_SPECS[0];
    const cid = companyId(spec);
    const entry = (firm: string, publishedAt: string) => ({
      firm,
      analyst: null,
      rating: "trzymaj",
      ratingPrev: null,
      direction: "initiate",
      targetPrice: null,
      targetCurrency: null,
      targetPrev: null,
      priceAtIssue: null,
      publishedAt,
      reportUrl: null,
      sourceUrl: "https://example.test/rec",
    });
    runtime.data.analystRecommendationsByCompany = {
      ...runtime.data.analystRecommendationsByCompany,
      [cid]: {
        companyId: cid,
        entries: [
          entry("A", "2026-06-01T00:00:00"),
          entry("B", "2026-05-01T00:00:00"),
          entry("C", "2026-04-01T00:00:00"),
          entry("D", "2026-03-01T00:00:00"),
          entry("E", "2026-02-01T00:00:00"),
        ],
        latestTarget: null,
        lastRefreshedAt: "2026-06-01T00:00:00",
      },
    } as typeof runtime.data.analystRecommendationsByCompany;

    const view = (await runtime.invoke("get_company_view", { companyId: cid })) as CompanyView;
    expect(view.recommendations).toHaveLength(3);
    expect(view.recommendations.map((r) => r.firm)).toEqual(["A", "B", "C"]);
  });
});
