import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useAlertsQuery } from "./useAlertsQuery";
import { createAlertRule, listAlertRules } from "../../api/attention";
import { listCompanies } from "../../api/companies";
import { listWatchlists } from "../../api/watchlists";
import type { AttentionController } from "../../app/useAttentionController";
import { makeAlertRule, makeAttentionEvent } from "../../test/scenarios/entities";

vi.mock("../../api/attention", () => ({
  createAlertRule: vi.fn(),
  deleteAlertRule: vi.fn(),
  listAlertRules: vi.fn(),
  setAlertRuleEnabled: vi.fn(),
  updateAlertRule: vi.fn(),
}));
vi.mock("../../api/companies", () => ({ listCompanies: vi.fn() }));
vi.mock("../../api/watchlists", () => ({ listWatchlists: vi.fn() }));

const listAlertRulesMock = vi.mocked(listAlertRules);
const listCompaniesMock = vi.mocked(listCompanies);
const listWatchlistsMock = vi.mocked(listWatchlists);
const createAlertRuleMock = vi.mocked(createAlertRule);

function fakeAttention(overrides: Partial<AttentionController> = {}): AttentionController {
  return {
    events: [],
    rulesById: new Map(),
    loading: false,
    hydrated: true,
    error: null,
    unseenCount: 0,
    refresh: vi.fn(),
    markSeen: vi.fn(() => Promise.resolve()),
    markManySeen: vi.fn(() => Promise.resolve()),
    dismiss: vi.fn(() => Promise.resolve()),
    ...overrides,
  };
}

const rule = makeAlertRule("rule-1", "signal_category", "company-1");

describe("useAlertsQuery", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("refetches the rules list after createRule resolves (spy count 2)", async () => {
    listAlertRulesMock.mockResolvedValue([rule]);
    listCompaniesMock.mockResolvedValue([]);
    listWatchlistsMock.mockResolvedValue([]);
    createAlertRuleMock.mockResolvedValue(rule);

    const attention = fakeAttention();
    const { result } = renderHook(() => useAlertsQuery(attention));

    await waitFor(() => expect(result.current.rules).toEqual([rule]));
    expect(listAlertRulesMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      await result.current.createRule({
        triggerType: "signal_category",
        signalCategory: "profit_warning",
        priceMin: null,
        priceMax: null,
        scopeType: "company",
        scopeRef: "company-1",
      });
    });

    await waitFor(() => expect(listAlertRulesMock).toHaveBeenCalledTimes(2));
  });

  it("resolves an overlapping refetch race to the NEWER request's data (the useCommandQuery seam)", async () => {
    const deferred: Array<(rules: ReturnType<typeof makeAlertRule>[]) => void> = [];
    listAlertRulesMock.mockImplementation(
      () => new Promise((resolve) => deferred.push(resolve)),
    );
    listCompaniesMock.mockResolvedValue([]);
    listWatchlistsMock.mockResolvedValue([]);

    const attention = fakeAttention();
    const { result } = renderHook(() => useAlertsQuery(attention));

    await waitFor(() => expect(listAlertRulesMock).toHaveBeenCalledTimes(1));

    act(() => {
      result.current.refetch();
    });
    await waitFor(() => expect(listAlertRulesMock).toHaveBeenCalledTimes(2));

    const olderRule = makeAlertRule("older", "signal_category", "company-1");
    const newerRule = makeAlertRule("newer", "signal_category", "company-1");

    // The NEWER request resolves first; the OLDER settles late and must be
    // discarded — this reddens if the per-run seq gate is removed.
    await act(async () => {
      deferred[1]([newerRule]);
      await Promise.resolve();
      deferred[0]([olderRule]);
      await Promise.resolve();
    });

    expect(result.current.rules).toEqual([newerRule]);
  });

  it("reports a partial status when the attention read fails but the rules read succeeds", async () => {
    listAlertRulesMock.mockResolvedValue([rule]);
    listCompaniesMock.mockResolvedValue([]);
    listWatchlistsMock.mockResolvedValue([]);

    const firedEvent = makeAttentionEvent("event-1", rule.id, "company-1");
    const attention = fakeAttention({ error: "boom", events: [firedEvent] });
    const { result } = renderHook(() => useAlertsQuery(attention));

    await waitFor(() => expect(result.current.status).toBe("partial"));
    expect(result.current.sectionErrors).toEqual({ events: "unavailable" });
    // Rules loaded fine; events section is degraded but the controller's
    // last-known-good events still pass through (ADR 0097 dec. 6).
    expect(result.current.rules).toEqual([rule]);
    expect(result.current.events).toEqual([firedEvent]);
  });
});
