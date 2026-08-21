import type { ReactElement } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { LocaleContext, makeTextTranslator, makeTranslator } from "../../../shared/locale";
import { makeAttentionEvent } from "../../../test/scenarios/entities";
import type { TodayItem } from "../../../api/generated/TodayItem";
import type { TodayClaim } from "../../../api/generated/TodayClaim";
import { AttentionRow } from "./AttentionRow";
import { AutopilotRunRow } from "./AutopilotRunRow";
import { CalendarRow } from "./CalendarRow";
import { ClaimRow } from "./ClaimRow";
import { DayHeader } from "./DayHeader";
import { DeltaHeader } from "./DeltaHeader";
import { FilingRow } from "./FilingRow";
import { MediaClusterRow } from "./MediaClusterRow";
import { NonArrivalRow } from "./NonArrivalRow";

function withPolish(ui: ReactElement) {
  return (
    <LocaleContext.Provider value={{ locale: "pl", t: makeTranslator("pl"), text: makeTextTranslator("pl") }}>
      {ui}
    </LocaleContext.Provider>
  );
}

describe("FilingRow", () => {
  const base: Extract<TodayItem, { kind: "filing" }> = {
    kind: "filing",
    feedItemId: "feed_1",
    companyId: "company_1",
    qualifiedTicker: "GPW:BFT",
    title: "Wyniki finansowe PSr /2026",
    publishedAt: "2026-08-20T17:19:00Z",
    read: false,
    presentationKind: "report",
  };

  it("uses \"Przeczytaj raport\" for the report variant and lands on the exact onOpen call", async () => {
    const onOpen = vi.fn();
    render(withPolish(<FilingRow item={base} onOpen={onOpen} />));
    const action = screen.getByRole("button", { name: "Przeczytaj raport" });
    await userEvent.click(action);
    expect(onOpen).toHaveBeenCalledTimes(1);
  });

  it("uses \"Otwórz komunikat\" for a bare filing notice", () => {
    render(withPolish(<FilingRow item={{ ...base, presentationKind: "filing" }} onOpen={vi.fn()} />));
    expect(screen.getByRole("button", { name: "Otwórz komunikat" })).toBeInTheDocument();
  });
});

describe("MediaClusterRow", () => {
  const base: Extract<TodayItem, { kind: "mediaCluster" }> = {
    kind: "mediaCluster",
    companyId: "company_2",
    qualifiedTicker: "GPW:KGH",
    day: "2026-08-20",
    count: 5,
    earliestPublishedAt: "2026-08-20T12:54:00Z",
    latestPublishedAt: "2026-08-20T14:40:00Z",
    topTitles: ["Sierra Gorda, kredyt 1 mld USD"],
    feedItemIds: ["f1", "f2", "f3", "f4", "f5"],
  };

  it("shows \"Otwórz w Inbox\" and the ×N chip for a real cluster", () => {
    render(withPolish(<MediaClusterRow item={base} onOpen={vi.fn()} />));
    expect(screen.getByRole("button", { name: "Otwórz w Inbox" })).toBeInTheDocument();
    expect(screen.getByText("MEDIA ×5")).toBeInTheDocument();
  });

  it("shows \"Otwórz artykuł\" for a single-article cluster", () => {
    render(withPolish(<MediaClusterRow item={{ ...base, count: 1, feedItemIds: ["f1"] }} onOpen={vi.fn()} />));
    expect(screen.getByRole("button", { name: "Otwórz artykuł" })).toBeInTheDocument();
  });
});

describe("NonArrivalRow", () => {
  it("shows the amber chip and the \"Odśwież źródła\" action", async () => {
    const onOpen = vi.fn();
    render(
      withPolish(
        <NonArrivalRow
          item={{
            kind: "nonArrival",
            eventKey: "ev1",
            companyId: "company_3",
            qualifiedTicker: "GPW:DNP",
            eventDate: "2026-08-20",
            title: "Raport H1 zapowiedziany wczoraj",
          }}
          onOpen={onOpen}
        />,
      ),
    );
    // Source text is normal-case; the mockup's caps look is a CSS
    // text-transform on `.dayq-chip` (i18n/screen-reader hygiene).
    expect(screen.getByText("Nie wpłynął")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Odśwież źródła" }));
    expect(onOpen).toHaveBeenCalledTimes(1);
  });
});

describe("CalendarRow", () => {
  it("renders no action button (Wiersze.dc.html: bez akcji — czeka)", () => {
    render(
      withPolish(
        <CalendarRow
          item={{
            kind: "calendar",
            eventKey: "cal1",
            eventDate: "2026-08-21",
            eventType: "periodic_report",
            title: "Publikacja raportu skonsolidowanego",
            companyId: "company_4",
            qualifiedTicker: "GPW:BDX",
          }}
        />,
      ),
    );
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});

describe("AutopilotRunRow", () => {
  it("resolves the caller-supplied ticker (the DTO carries none)", () => {
    render(
      withPolish(
        <AutopilotRunRow
          item={{
            kind: "autopilotRun",
            run: {
              id: "run1",
              companyId: "company_5",
              reportDocumentId: "doc1",
              trigger: "detection",
              mode: "assist",
              sweepId: null,
              status: "succeeded",
              stage: "done",
              summaryText: null,
              kpiDeltaJson: null,
              reportDiffRef: null,
              crossRefsJson: null,
              producedFactIds: [],
              notificationState: "unread",
              lastError: null,
              createdAt: "2026-08-21T05:00:00Z",
              updatedAt: "2026-08-21T05:00:00Z",
              severity: "routine",
              reportDocumentTitle: "Raport roczny 2025",
            },
          }}
          qualifiedTicker="GPW:XYZ"
          onOpen={vi.fn()}
        />,
      ),
    );
    expect(screen.getByText("Raport roczny 2025")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Przeczytaj raport" })).toBeInTheDocument();
  });
});

describe("AttentionRow", () => {
  it("carries the agent (accent/violet) chip tone and the caller's action label", async () => {
    const onOpen = vi.fn();
    const event = makeAttentionEvent("attn1", "rule1", "company_6", "urgent");
    render(withPolish(<AttentionRow event={event} qualifiedTicker="GPW:CDR" actionLabel="Otwórz szczegóły" onOpen={onOpen} />));
    const chip = screen.getByText("Sygnał");
    expect(chip.closest(".ui-status-chip")).toHaveClass("ui-status-chip-accent");
    await userEvent.click(screen.getByRole("button", { name: "Otwórz szczegóły" }));
    expect(onOpen).toHaveBeenCalledTimes(1);
  });
});

describe("ClaimRow", () => {
  it("shows \"Otwórz tezę\" and the period chip", async () => {
    const onOpen = vi.fn();
    const entry: TodayClaim = {
      claim: {
        id: "claim1",
        companyId: "company_7",
        statement: "Koszty marketingowe wzrosną o ok. 50% r/r",
        body: "",
        bodyFormat: "markdown",
        madeAt: null,
        sourcePeriodId: null,
        dueFiscalYear: 2026,
        duePeriodType: "FY",
        status: "pending",
        sourceEvidenceType: "manual",
        sourceEvidenceId: null,
        targetMetricKey: null,
        targetComparator: null,
        targetValueNumeric: null,
        targetUnit: null,
        verifyingFactId: null,
        revisesClaimId: null,
        createdAt: "2026-08-01T00:00:00Z",
        updatedAt: "2026-08-01T00:00:00Z",
      },
      qualifiedTicker: "GPW:XTB",
      bucket: "due",
    };
    render(withPolish(<ClaimRow entry={entry} onOpen={onOpen} />));
    expect(screen.getByText("Teza · FY 2026")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Otwórz tezę" }));
    expect(onOpen).toHaveBeenCalledTimes(1);
  });
});

describe("DayHeader", () => {
  it("renders counters with an unseen clause when expanded", () => {
    render(withPolish(<DayHeader day="2026-08-20" relativeDay="yesterday" total={7} unseen={4} collapsed={false} />));
    expect(screen.getByText("Wczoraj")).toBeInTheDocument();
    expect(screen.getByText("7 pozycji · 4 nieprzejrzane")).toBeInTheDocument();
  });

  it("renders the collapsed variant with the Otwórz dzień recovery link", async () => {
    const onExpand = vi.fn();
    render(
      withPolish(
        <DayHeader day="2026-08-19" relativeDay="earlier" total={6} unseen={0} collapsed onExpand={onExpand} />,
      ),
    );
    await userEvent.click(screen.getByRole("button", { name: "Otwórz dzień" }));
    expect(onExpand).toHaveBeenCalledTimes(1);
  });
});

describe("DeltaHeader", () => {
  it("marks the primary action with data-ux-primary-action and omits it on a clean morning", () => {
    const { rerender } = render(
      <DeltaHeader eyebrow="DZIŚ · 07:25" headline="Since your last visit: 1 report" primaryAction={{ label: "Read report", onClick: vi.fn() }} />,
    );
    expect(screen.getByRole("button", { name: "Read report" })).toHaveAttribute(
      "data-ux-primary-action",
      "true",
    );

    rerender(<DeltaHeader eyebrow="DZIŚ · 07:25" headline="Nothing new since your last visit" primaryAction={null} />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});
