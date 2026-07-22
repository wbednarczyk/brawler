import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Mock } from "vitest";
import type { Company } from "../api/types";

// ADR 0081 Q2 (Radicle a9992e2) — component-layer proof of the
// `useResearchController` request-version seam ("last-intent-wins",
// useResearchController.ts:148-172: a response is applied only while
// `requestVersionRef.current === requestVersion`). Q8 (66621c9) moved this from a
// skipped Playwright case to here: the per-company `list_research_evidence` fetch
// is a controller concern with no cheaper authoritative surface than the hook
// itself (the standalone Research screen is retired as a nav destination, epic
// c793ca1), so a controlled-async renderHook test is the authoritative layer.

vi.mock("../api/research", () => ({
  listResearchEvidence: vi.fn(),
  listResearchQuestions: vi.fn(() => Promise.resolve([])),
  listResearchReminders: vi.fn(() => Promise.resolve([])),
  listResearchBriefs: vi.fn(() => Promise.resolve([])),
  listResearchDigests: vi.fn(() => Promise.resolve([])),
  listEvidenceLinks: vi.fn(() => Promise.resolve([])),
  markResearchScopeReviewed: vi.fn(() => Promise.resolve()),
  createResearchQuestion: vi.fn(() => Promise.resolve()),
  updateResearchQuestion: vi.fn(() => Promise.resolve()),
  deleteResearchQuestion: vi.fn(() => Promise.resolve()),
  createResearchReminder: vi.fn(() => Promise.resolve()),
  updateResearchReminder: vi.fn(() => Promise.resolve()),
  deleteResearchReminder: vi.fn(() => Promise.resolve()),
  createEvidenceLink: vi.fn(() => Promise.resolve()),
  deleteEvidenceLink: vi.fn(() => Promise.resolve()),
}));

import * as researchApi from "../api/research";
import { useResearchController } from "./useResearchController";

function company(id: string, ticker: string): Company {
  return {
    id,
    exchange: "GPW",
    ticker,
    qualifiedTicker: `GPW:${ticker}`,
    displayName: `${ticker} S.A.`,
    isin: null,
    cik: null,
    lei: null,
  } as Company;
}

const COMPANY_A = company("company_gpw_cdr", "CDR");
const COMPANY_B = company("company_gpw_pkn", "PKN");

type Deferred = { resolve: (value: unknown) => void };

describe("useResearchController request-version seam (last-intent-wins)", () => {
  let deferreds: Deferred[];

  beforeEach(() => {
    deferreds = [];
    (researchApi.listResearchEvidence as Mock).mockImplementation(
      () => new Promise<unknown>((resolve) => deferreds.push({ resolve })),
    );
  });

  it("suppresses a stale evidence response so it cannot replace the newer state", async () => {
    const { result } = renderHook(() =>
      useResearchController({
        activeSection: "Research",
        companies: [COMPANY_A, COMPANY_B],
        watchlists: [],
        watchlistMemberships: [],
        text: (value) => value,
        runUndoableDelete: () => {},
      }),
    );

    // On mount the controller auto-selects the first company (A) and fires its
    // evidence fetch (requestVersion 1) — held, not yet delivered.
    await waitFor(() => expect(deferreds).toHaveLength(1));

    // Switch to company B — a fresh fetch (requestVersion 2), also held.
    await act(async () => {
      result.current.setSelectedResearchCompanyId(COMPANY_B.id);
    });
    await waitFor(() => expect(deferreds).toHaveLength(2));

    // Deliver the NEWER response (B) first — the UI adopts B's timeline.
    const timelineB = { marker: "B" };
    await act(async () => {
      deferreds[1].resolve(timelineB);
    });
    await waitFor(() => expect(result.current.researchTimeline).toBe(timelineB));

    // Deliver the OLDER response (A) second — it is now stale. The requestVersion
    // guard must suppress it: the timeline stays B and never reverts to A.
    const timelineA = { marker: "A" };
    await act(async () => {
      deferreds[0].resolve(timelineA);
    });
    // Give any (incorrect) state update a chance to flush before asserting.
    await Promise.resolve();
    expect(result.current.researchTimeline).toBe(timelineB);
    expect(result.current.researchTimeline).not.toBe(timelineA);
  });
});
