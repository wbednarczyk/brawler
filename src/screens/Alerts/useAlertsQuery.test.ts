import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useAlertsQuery, type AlertsQueryResult } from "./useAlertsQuery";
import {
  createAlertRule,
  deleteAlertRule,
  listAlertRules,
  setAlertRuleEnabled,
  updateAlertRule,
} from "../../api/attention";
import { listCompanies } from "../../api/companies";
import { listWatchlists } from "../../api/watchlists";
import type { Company } from "../../api/types";
import type { AttentionController } from "../../app/useAttentionController";
import { COMPANY_SPECS, makeAlertRule, makeAttentionEvent, makeCompany } from "../../test/scenarios/entities";

// `listAlertRules` stays mocked even though `useAlertsQuery` no longer
// imports it — the spy-zero assertion below needs it.
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
const setAlertRuleEnabledMock = vi.mocked(setAlertRuleEnabled);
const deleteAlertRuleMock = vi.mocked(deleteAlertRule);
const updateAlertRuleMock = vi.mocked(updateAlertRule);

function makeAttention(
  rules: AttentionController["rules"],
  overrides: Partial<AttentionController> = {},
): AttentionController {
  return {
    events: [],
    rules,
    rulesById: new Map(rules.map((rule) => [rule.id, rule])),
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

  // (ADR 0097 dec. 6): rules have exactly one owner — the
  // shared AttentionController. A second `listAlertRules` fetch here would
  // let this screen's copy drift from Today's/the badge's.
  it("never calls listAlertRules itself — rules come from the attention controller", async () => {
    listCompaniesMock.mockResolvedValue([]);
    listWatchlistsMock.mockResolvedValue([]);

    const attention = makeAttention([rule]);
    const { result } = renderHook(() => useAlertsQuery(attention));

    await waitFor(() => expect(result.current.status).toBe("success"));
    expect(result.current.rules).toEqual([rule]);
    expect(listAlertRulesMock).not.toHaveBeenCalled();
  });

  const newRuleInput = {
    triggerType: "signal_category" as const,
    signalCategory: "profit_warning",
    priceMin: null,
    priceMax: null,
    scopeType: "company" as const,
    scopeRef: "company-1",
  };
  const priceUpdateInput = { id: rule.id, priceMin: 1, priceMax: 2 };

  it.each([
    [
      "createRule",
      () => createAlertRuleMock.mockResolvedValue(rule),
      (alerts: AlertsQueryResult) => alerts.createRule(newRuleInput),
    ],
    [
      "setRuleEnabled",
      () => setAlertRuleEnabledMock.mockResolvedValue(rule),
      (alerts: AlertsQueryResult) => alerts.setRuleEnabled(rule.id, false),
    ],
    [
      "removeRule",
      () => deleteAlertRuleMock.mockResolvedValue(undefined),
      (alerts: AlertsQueryResult) => alerts.removeRule(rule.id),
    ],
    [
      "updateRulePrice",
      () => updateAlertRuleMock.mockResolvedValue(rule),
      (alerts: AlertsQueryResult) => alerts.updateRulePrice(priceUpdateInput),
    ],
  ] as const)(
    "%s persists then calls attention.refresh once; the rendered list converges to the controller's next rules",
    async (_name, setupMock, runAction) => {
      listCompaniesMock.mockResolvedValue([]);
      listWatchlistsMock.mockResolvedValue([]);
      setupMock();
      const refresh = vi.fn();

      const { result, rerender } = renderHook(
        ({ attention }: { attention: AttentionController }) => useAlertsQuery(attention),
        { initialProps: { attention: makeAttention([], { refresh }) } },
      );
      await waitFor(() => expect(result.current.status).toBe("success"));

      await act(async () => {
        await runAction(result.current);
      });

      expect(refresh).toHaveBeenCalledTimes(1);

      // Convergence: the hook reads `attention.rules`
      // directly — once the controller's next value lands, the rendered
      // list reflects it with no second, local refetch.
      rerender({ attention: makeAttention([rule], { refresh }) });
      expect(result.current.rules).toEqual([rule]);
    },
  );

  // (c) + (d): the ADR 0097 false-quiet ban — the "All quiet" empty state
  // reads off `status`, so `status` itself must never leave "loading" before
  // the controller has hydrated, even once the unrelated library read (here:
  // companies/watchlists) has already resolved.
  it("stays 'loading' — never 'success' — while the controller is unhydrated, even after the library resolves", async () => {
    let resolveCompanies!: () => void;
    listCompaniesMock.mockImplementation(
      () => new Promise((resolve) => (resolveCompanies = () => resolve([]))),
    );
    listWatchlistsMock.mockResolvedValue([]);

    const attention = makeAttention([], { hydrated: false, loading: false });
    const { result } = renderHook(() => useAlertsQuery(attention));

    await act(async () => {
      resolveCompanies();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.status).toBe("loading");
  });

  it("resolves an overlapping refetch race to the NEWER request's data (the useCommandQuery seam)", async () => {
    const deferred: Array<(companies: Company[]) => void> = [];
    listCompaniesMock.mockImplementation(
      () => new Promise((resolve) => deferred.push(resolve)),
    );
    listWatchlistsMock.mockResolvedValue([]);

    const attention = makeAttention([]);
    const { result } = renderHook(() => useAlertsQuery(attention));

    await waitFor(() => expect(listCompaniesMock).toHaveBeenCalledTimes(1));

    act(() => {
      result.current.refetch();
    });
    await waitFor(() => expect(listCompaniesMock).toHaveBeenCalledTimes(2));

    const olderCompany = makeCompany(COMPANY_SPECS.find((spec) => spec.key === "pzu")!);
    const newerCompany = makeCompany(COMPANY_SPECS.find((spec) => spec.key === "cdr")!);

    // The NEWER request resolves first; the OLDER settles late and must be
    // discarded — this reddens if the per-run seq gate is removed.
    await act(async () => {
      deferred[1]([newerCompany]);
      await Promise.resolve();
      deferred[0]([olderCompany]);
      await Promise.resolve();
    });

    expect(result.current.companies).toEqual([newerCompany]);
  });

  it("reports a partial status when the attention read fails but the library read succeeds", async () => {
    listCompaniesMock.mockResolvedValue([]);
    listWatchlistsMock.mockResolvedValue([]);

    const firedEvent = makeAttentionEvent("event-1", rule.id, "company-1");
    const attention = makeAttention([rule], { error: "boom", events: [firedEvent] });
    const { result } = renderHook(() => useAlertsQuery(attention));

    await waitFor(() => expect(result.current.status).toBe("partial"));
    expect(result.current.sectionErrors).toEqual({ rules: "unavailable", events: "unavailable" });
    // Rules come straight off the controller, unaffected by the library's
    // own status; the controller's last-known-good events still pass through
    // (ADR 0097 dec. 6).
    expect(result.current.rules).toEqual([rule]);
    expect(result.current.events).toEqual([firedEvent]);
  });

  it("a failed first attention read is the error state with Retry, never an endless skeleton", async () => {
    listCompaniesMock.mockResolvedValue([]);
    listWatchlistsMock.mockResolvedValue([]);
    const attention = makeAttention([], { hydrated: false, loading: false, error: "attention read failed" });
    const { result } = renderHook(() => useAlertsQuery(attention));

    await waitFor(() => expect(result.current.status).not.toBe("loading"));
    expect(result.current.status).toBe("partial");
    expect(result.current.sectionErrors).toEqual({ rules: "unavailable", events: "unavailable" });
  });

  it("a failed company/list read flags the scope names only — the controller's rules stay rendered", async () => {
    listCompaniesMock.mockRejectedValue(new Error("companies unavailable"));
    listWatchlistsMock.mockResolvedValue([]);
    const attention = makeAttention([rule]);
    const { result } = renderHook(() => useAlertsQuery(attention));

    await waitFor(() => expect(result.current.status).toBe("partial"));
    expect(result.current.sectionErrors).toEqual({ scope: "unavailable" });
    expect(result.current.rules).toEqual([rule]);
  });
});
