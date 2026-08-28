import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import type { Company } from "../../../api/types";
import type { DecisionEntry } from "../../../api/decisionJournal";
import { createDecisionEntry, listDecisionEntries } from "../../../api/decisionJournal";
import { createEvidenceLink, listCompanyTimeline, listEvidenceLinks } from "../../../api/research";
import { useDecisionJournalPanel } from "./useDecisionJournalPanel";

vi.mock("../../../api/decisionJournal", () => ({
  createDecisionEntry: vi.fn(),
  listDecisionEntries: vi.fn(),
}));

vi.mock("../../../api/research", () => ({
  createEvidenceLink: vi.fn(),
  listCompanyTimeline: vi.fn(),
  listEvidenceLinks: vi.fn(),
}));

const createDecisionEntryMock = vi.mocked(createDecisionEntry);
const listDecisionEntriesMock = vi.mocked(listDecisionEntries);
const createEvidenceLinkMock = vi.mocked(createEvidenceLink);
const listCompanyTimelineMock = vi.mocked(listCompanyTimeline);
const listEvidenceLinksMock = vi.mocked(listEvidenceLinks);

const company = { id: "c1", qualifiedTicker: "GPW:CDR", displayName: "CD PROJEKT S.A." } as Company;

function entry(overrides: Partial<DecisionEntry>): DecisionEntry {
  return {
    id: "d1",
    companyId: "c1",
    kind: "buy",
    rationaleMd: "Entering a position.",
    decidedAt: "2026-06-01",
    supersededByEntryId: null,
    createdAt: "2026-06-01T00:00:00Z",
    ...overrides,
  };
}

describe("useDecisionJournalPanel (ADR 0071, J3)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listDecisionEntriesMock.mockResolvedValue([]);
    listCompanyTimelineMock.mockResolvedValue({
      items: [],
      summary: {
        total: 0,
        changedSinceReview: 0,
        lastReviewedAt: null,
        memberCompanyCount: 0,
        companiesWithChangedEvidence: 0,
        companySummaries: [],
      },
    });
    listEvidenceLinksMock.mockResolvedValue([]);
  });

  it("creates an entry and shows it in the company list", async () => {
    const created = entry({ id: "d1", rationaleMd: "New thesis." });
    createDecisionEntryMock.mockResolvedValue(created);
    listDecisionEntriesMock.mockResolvedValueOnce([]).mockResolvedValue([created]);

    const { result } = renderHook(() => useDecisionJournalPanel(company));
    await waitFor(() => expect(listDecisionEntriesMock).toHaveBeenCalledTimes(1));

    act(() => result.current.updateForm("rationaleMd", "New thesis."));
    act(() => result.current.createEntry());

    await waitFor(() => expect(createDecisionEntryMock).toHaveBeenCalledTimes(1));
    expect(createDecisionEntryMock).toHaveBeenCalledWith(
      expect.objectContaining({ companyId: "c1", kind: "buy", rationaleMd: "New thesis." }),
    );
    await waitFor(() => expect(result.current.entries).toEqual([created]));
  });

  it("supersede creates a follow-up entry linked to the superseded one", async () => {
    const original = entry({ id: "d1", kind: "keep_watching" });
    createDecisionEntryMock.mockResolvedValue(entry({ id: "d2", supersededByEntryId: "d1" }));
    listDecisionEntriesMock.mockResolvedValue([original]);

    const { result } = renderHook(() => useDecisionJournalPanel(company));
    await waitFor(() => expect(result.current.entries).toEqual([original]));

    act(() => result.current.startSupersede(original));
    expect(result.current.supersedingEntry).toEqual(original);
    // The follow-up seeds the superseded entry's kind.
    expect(result.current.form.kind).toBe("keep_watching");

    act(() => result.current.updateForm("rationaleMd", "Revised: opening a small position."));
    act(() => result.current.createEntry());

    await waitFor(() => expect(createDecisionEntryMock).toHaveBeenCalledTimes(1));
    expect(createDecisionEntryMock).toHaveBeenCalledWith(
      expect.objectContaining({ supersededByEntryId: "d1" }),
    );
  });

  it("attaches evidence to the selected entry with fromType decision_entry", async () => {
    const existing = entry({ id: "d1" });
    listDecisionEntriesMock.mockResolvedValue([existing]);
    listCompanyTimelineMock.mockResolvedValue({
      items: [
        {
          id: "e1",
          evidenceType: "feed_item",
          sourceDomain: "feed",
          sourceId: "feed_1",
          companyId: "c1",
          occurredAt: "2026-05-30T00:00:00Z",
          title: "Q1 report",
          summary: null,
          sourceUrl: null,
          attribution: null,
          trustCategory: "official_report",
          reviewState: { changedSinceCompanyReview: false, changedSinceWatchlistReview: false },
        },
      ],
      summary: {
        total: 1,
        changedSinceReview: 0,
        lastReviewedAt: null,
        memberCompanyCount: 0,
        companiesWithChangedEvidence: 0,
        companySummaries: [],
      },
    });
    createEvidenceLinkMock.mockResolvedValue({
      id: "link_1",
      fromType: "decision_entry",
      fromId: "d1",
      toType: "feed_item",
      toId: "feed_1",
      relationType: "cites",
      createdAt: "2026-06-01T00:00:00Z",
    });

    const { result } = renderHook(() => useDecisionJournalPanel(company));
    await waitFor(() => expect(result.current.entries).toEqual([existing]));

    act(() => result.current.setSelectedEntryId("d1"));
    await waitFor(() => expect(result.current.evidenceCandidates).toHaveLength(1));

    act(() => result.current.linkEvidence(result.current.evidenceCandidates[0]));

    await waitFor(() => expect(createEvidenceLinkMock).toHaveBeenCalledTimes(1));
    expect(createEvidenceLinkMock).toHaveBeenCalledWith({
      fromType: "decision_entry",
      fromId: "d1",
      toType: "feed_item",
      toId: "feed_1",
      relationType: "cites",
    });
  });
});
