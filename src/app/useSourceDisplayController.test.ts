import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import { createElement } from "react";

import { useSourceDisplayController } from "./useSourceDisplayController";
import { LocaleContext, makeTextTranslator, makeTranslator, type LocaleCode } from "../shared/locale";
import type { SourceAdapter, UserSettings } from "../api/types";

// sol R1: the scheduler/next-refresh sentences must never splice a raw
// English interval into an otherwise-Polish sentence (the bug: string
// concatenation around `formatPollInterval`, English-only). Each sentence is
// now one text() template filled in after translation — asserted in both
// locales here, including the "day" plural form that triggered the finding.

function wrapper(locale: LocaleCode) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return createElement(
      LocaleContext.Provider,
      { value: { locale, t: makeTranslator(locale), text: makeTextTranslator(locale) } },
      children,
    );
  };
}

function makeAdapter(overrides: Partial<SourceAdapter> = {}): SourceAdapter {
  return {
    id: "bankier-company-komunikaty",
    displayName: "Bankier Company Komunikaty",
    sourceType: "official_report",
    fetchMode: "public_json",
    visibility: "required",
    role: "primary",
    userConfigurable: true,
    healthStatus: "healthy",
    enabled: true,
    defaultPollIntervalSeconds: 900,
    sourceUrl: "https://example.test",
    rateLimitPolicy: "",
    policyNote: "",
    lastAttemptAt: null,
    lastTrigger: null,
    lastSuccessAt: null,
    lastErrorAt: null,
    lastError: null,
    lastItemsFetched: null,
    lastItemsCreated: null,
    lastItemsMatched: null,
    lastItemsUnmatched: null,
    lastDetailItemsAttempted: null,
    lastDetailItemsStored: null,
    lastDetailItemsFailed: null,
    lastDetailWarning: null,
    markets: ["GPW"],
    ...overrides,
  };
}

const settings = { pollIntervalSeconds: 900 } as UserSettings;

function setup(
  locale: LocaleCode,
  sourceRefreshFailureCount = 0,
  nextRefresh: { nextRegistryRefreshAt?: number | null; nextSourceRefreshAtByAdapterId?: Record<string, number> } = {},
) {
  return renderHook(
    () =>
      useSourceDisplayController({
        nextRegistryRefreshAt: nextRefresh.nextRegistryRefreshAt ?? null,
        nextSourceRefreshAtByAdapterId: nextRefresh.nextSourceRefreshAtByAdapterId ?? {},
        refreshCompanyRegistryEntries: vi.fn(),
        setActiveSection: vi.fn(),
        setCompanyRegistryListExpanded: vi.fn(),
        setSelectedSourceAdapterId: vi.fn(),
        settings,
        sourceAdapters: [],
        sourceRefreshFailureCount,
      }),
    { wrapper: wrapper(locale) },
  );
}

describe("useSourceDisplayController — locale-aware scheduler sentences (sol R1)", () => {
  it.each(["en", "pl"] as const)("formatSourceScheduler: no fragment concatenation, one sentence (%s)", (locale) => {
    const { result } = setup(locale);
    const sentence = result.current.formatSourceScheduler(makeAdapter());
    expect(sentence).toBe(
      locale === "pl" ? "Automatycznie co 15 min" : "Automatically every 15 min",
    );
  });

  it.each(["en", "pl"] as const)("formatSourceScheduler backoff sentence names the retry, no English leaking into PL (%s)", (locale) => {
    const { result } = setup(locale, 2);
    const sentence = result.current.formatSourceScheduler(makeAdapter());
    expect(sentence).toBe(
      locale === "pl"
        ? "Automatycznie co 15 min · ponowna próba za 30 min"
        : "Automatically every 15 min · retry in 30 min",
    );
  });

  it.each(["en", "pl"] as const)("a whole-day interval uses the locale's plural word, never the raw English \"day\" (%s)", (locale) => {
    const { result } = setup(locale);
    const sentence = result.current.formatSourceScheduler(
      makeAdapter({ sourceType: "company_registry", defaultPollIntervalSeconds: 86400 }),
    );
    expect(sentence).toBe(locale === "pl" ? "Automatycznie co 1 dzień" : "Automatically every 1 day");
    if (locale === "pl") {
      expect(sentence).not.toMatch(/\bday\b/);
    }
  });

  it.each(["en", "pl"] as const)("multiple days pluralize correctly (%s)", (locale) => {
    const { result } = setup(locale);
    const sentence = result.current.formatSourceScheduler(
      makeAdapter({ sourceType: "company_registry", defaultPollIntervalSeconds: 86400 * 3 }),
    );
    expect(sentence).toBe(locale === "pl" ? "Automatycznie co 3 dni" : "Automatically every 3 days");
  });

  it.each(["en", "pl"] as const)("formatNextRefresh: one templated sentence, no bare fragments (%s)", (locale) => {
    const { result } = setup(locale, 0, { nextSourceRefreshAtByAdapterId: { "bankier-company-komunikaty": Date.now() + 15 * 60_000 } });
    const sentence = result.current.formatNextRefresh(makeAdapter());
    expect(sentence).toMatch(locale === "pl" ? /^za 1[45] min/ : /^next in 1[45] min/);
  });
});
