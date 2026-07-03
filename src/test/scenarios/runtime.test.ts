import { describe, expect, it } from "vitest";

import { createMockRuntime, knownCommands, READ_COMMANDS } from "./runtime";
import { buildScenario } from "./scenarios";

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

    const flagged = (await runtime.invoke("list_flagged_fact_provenance", {})) as Array<{ factId: string }>;
    expect(flagged.map((p) => p.factId)).toEqual(["f2"]);

    const summary = (await runtime.invoke("run_structured_extraction", {
      input: { companyId: "c1", reportDocumentId: "d1", fiscalYear: 2026, periodType: "FY", periodEnd: "2026-03-31" },
    })) as { acceptance: string };
    expect(summary.acceptance).toBe("accepted");
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

  it("reset restores a fresh store and clears mutations", async () => {
    const runtime = createMockRuntime("minimal");
    await runtime.invoke("create_watchlist", { input: { name: "Temp", description: null } });
    runtime.reset();
    const watchlists = (await runtime.invoke("list_watchlists", {})) as unknown[];
    expect(watchlists).toHaveLength(1);
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
