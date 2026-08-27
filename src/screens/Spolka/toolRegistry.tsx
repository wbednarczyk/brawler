import { useEffect, useRef, useState, type ReactElement } from "react";
import { ArrowLeft, X } from "lucide-react";
import type { Company, FeedItem } from "../../api/types";
import { listClaimsToVerify } from "../../api/managementClaims";
import { listCompanyEvents } from "../../api/events";
import { useCommandQuery } from "../../shared/state/useCommandQuery";
import { useLocale } from "../../shared/locale";
import { Button, DenseRow, EmptyState, ErrorText, SectionHeader, Skeleton } from "../../ui";
import { CompanyBasicInfoPanel } from "../../shared/components/CompanyBasicInfoPanel";
import { CompanyClaimsPanel } from "../../shared/components/CompanyClaimsPanel";
import { CompanyCoveragePanel } from "../../shared/components/CompanyCoveragePanel";
import { CompanyReportDocumentsPanel } from "../../shared/components/CompanyReportDocumentsPanel";
import { QualityPanel } from "../../shared/components/QualityPanel";
import { ReportDiffPanel } from "../Companies/ReportDiffPanel";
import { ResearchScreen } from "../Research/ResearchScreen";
import { CockpitCompanyFeedPanel } from "../Cockpit/CockpitCompanyFeedPanel";
import {
  CockpitAnalystRecommendationsPanel,
  CockpitCompanyNotebookPanel,
  CockpitDecisionJournalPanel,
  CockpitFundamentalsPanel,
  CockpitRedFlagsPanel,
  CockpitShortPositionsPanel,
} from "../Cockpit/companyPanels";
import type { Tool } from "./route";

// Maps every `Tool` variant to the SAME hosted component the cockpit's
// `renderPinned`/`renderLinked` use (F3a S2, ADR 0107) — reused verbatim via
// `../Cockpit/companyPanels`, never re-implemented.

export type ToolRenderContext = {
  companyId: string;
  company: Company;
  feedItems: FeedItem[];
  /** Today's `openCompanyClaims` global highlight (F2 S3) — falls back to it
   * only when the tool itself carries no `claimId`. */
  rootHighlightClaimId: string | null;
  onOpenTool: (tool: Tool) => void;
  onOpenDocument: (documentRef: string) => void;
  onOpenFeedItem: (feedItemId: string) => void;
  onCloseTool: () => void;
};

const TOOL_TITLES: Record<Tool["t"], string> = {
  tezy: "Claims",
  feedItem: "Feed item",
  dokumenty: "Documents",
  feed: "Feed",
  notatnik: "Notebook",
  dziennik: "Decision journal",
  jakosc: "Quality",
  diff: "Report diff",
  research: "Research",
  akcjonariat: "Ownership",
  sygnaly: "Signals",
  fundamenty: "Fundamentals",
  pokrycie: "Coverage",
  rekomendacje: "Recommendations",
  wydarzenia: "Events",
};

export function renderTool(tool: Tool, ctx: ToolRenderContext): ReactElement {
  return <ToolFrame tool={tool} ctx={ctx} />;
}

function ToolFrame({ tool, ctx }: { tool: Tool; ctx: ToolRenderContext }) {
  const { text } = useLocale();
  return (
    <div role="group" aria-label={text("Workshop tool")} data-tool={tool.t} className="spolka-tool">
      <div className="spolka-tool-header">
        {/* A leading way back to the untouched core (owner dogfooding v0.74,
            item 5) — same destination as the ✕, which stays for the
            close-without-looking-back gesture. */}
        <Button variant="ghost" onClick={ctx.onCloseTool}>
          <ArrowLeft size={14} aria-hidden="true" />
          {text("Overview")}
        </Button>
        <SectionHeader
          level="h2"
          eyebrow={text("Workshop")}
          title={text(TOOL_TITLES[tool.t])}
          actions={
            <Button variant="icon" aria-label={text("Close tool")} onClick={ctx.onCloseTool}>
              <X size={16} aria-hidden="true" />
            </Button>
          }
        />
      </div>
      <div className="spolka-tool-body">{renderToolBody(tool, ctx)}</div>
    </div>
  );
}

function renderToolBody(tool: Tool, ctx: ToolRenderContext): ReactElement {
  switch (tool.t) {
    case "tezy":
      return (
        <TezyTool
          companyId={ctx.companyId}
          claimId={tool.claimId}
          rootHighlightClaimId={ctx.rootHighlightClaimId}
        />
      );
    case "feedItem":
      // The selected item's detail leads, the list stays reachable below it
      // (owner dogfooding v0.74, item 7) — opened from the Inbox "Otwórz
      // spółkę", the item could sit anywhere in a 30+-row feed.
      return (
        <CockpitCompanyFeedPanel
          company={ctx.company}
          feedItems={ctx.feedItems}
          initialSelectedFeedItemId={tool.feedItemId}
          leadWithDetail
        />
      );
    case "dokumenty":
      // KPI provenance ticket navigation (sol-review finding 8, ADR 0104 dec.
      // 7): the target document scrolls into view + flashes once loaded.
      return <CompanyReportDocumentsPanel companyId={ctx.companyId} highlightDocumentRef={tool.documentId} />;
    case "feed":
      return <CockpitCompanyFeedPanel company={ctx.company} feedItems={ctx.feedItems} />;
    case "notatnik":
      return <CockpitCompanyNotebookPanel company={ctx.company} />;
    case "dziennik":
      return <CockpitDecisionJournalPanel company={ctx.company} />;
    case "jakosc":
      return <QualityPanel companyId={ctx.companyId} />;
    case "diff":
      return <ReportDiffPanel companyId={ctx.companyId} />;
    case "research":
      // ResearchScreen has no company-scope prop yet (context-driven, repoctx
      // confirmed) — hosts the global screen, as the contract allows.
      return <ResearchScreen />;
    case "akcjonariat":
      return <AkcjonariatTool companyId={ctx.companyId} company={ctx.company} />;
    case "sygnaly":
      return <CockpitRedFlagsPanel company={ctx.company} onOpenEvidence={ctx.onOpenFeedItem} />;
    case "fundamenty":
      return (
        <CockpitFundamentalsPanel
          companyId={ctx.companyId}
          qualifiedTicker={ctx.company.qualifiedTicker}
          revision={0}
          onOpenRecommendations={() => ctx.onOpenTool({ t: "rekomendacje" })}
        />
      );
    case "pokrycie":
      return (
        <CompanyCoveragePanel
          companyId={ctx.companyId}
          onOpenDocuments={() => ctx.onOpenTool({ t: "dokumenty" })}
        />
      );
    case "rekomendacje":
      return <CockpitAnalystRecommendationsPanel company={ctx.company} />;
    case "wydarzenia":
      return <WydarzeniaTool companyId={ctx.companyId} />;
  }
}

// `{t:"tezy"}` with no explicit `claimId`/root highlight: highlights the FIRST
// claim in the review queue (overdue before due — plan §8, J2 red case) so
// "Open claims" from the workshop bar always lands on something actionable.
function TezyTool({
  companyId,
  claimId,
  rootHighlightClaimId,
}: {
  companyId: string;
  claimId?: string;
  rootHighlightClaimId: string | null;
}) {
  const explicit = claimId ?? rootHighlightClaimId ?? null;
  const [queueHighlightId, setQueueHighlightId] = useState<string | null>(null);

  useEffect(() => {
    if (explicit) return undefined;
    let cancelled = false;
    listClaimsToVerify(companyId)
      .then((queue) => {
        if (cancelled) return;
        const first = queue.overdue[0] ?? queue.due[0];
        setQueueHighlightId(first ? first.claim.id : null);
      })
      .catch(() => {
        if (!cancelled) setQueueHighlightId(null);
      });
    return () => {
      cancelled = true;
    };
  }, [companyId, explicit]);

  return <CompanyClaimsPanel companyId={companyId} highlightClaimId={explicit ?? queueHighlightId} />;
}

// `basicInfo` + the full short-positions section stacked (plan § Parzystość
// akcji): the shorts counter drill always scrolls the short-positions section
// into view.
function AkcjonariatTool({ companyId, company }: { companyId: string; company: Company }) {
  const shortsRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // jsdom (the vitest/browser test harness) does not implement
    // `scrollIntoView` — guarded so tests don't need a polyfill.
    shortsRef.current?.scrollIntoView?.({ block: "start" });
  }, []);
  return (
    <div className="spolka-tool-akcjonariat">
      <CompanyBasicInfoPanel companyId={companyId} />
      <div ref={shortsRef} data-section="shorts">
        <CockpitShortPositionsPanel company={company} />
      </div>
    </div>
  );
}

// `{t:"wydarzenia"}`: the company's upcoming events, chronological — a plain
// list (NOT the week grid), so the Events screen's cut-Friday defect doesn't
// carry over (plan § Scope decision).
function WydarzeniaTool({ companyId }: { companyId: string }) {
  const { text } = useLocale();
  const query = useCommandQuery(["spolka-events", companyId], () =>
    listCompanyEvents({
      mode: "upcoming",
      companyId,
      watchlistId: null,
      eventType: null,
      status: null,
      dateFrom: null,
      dateTo: null,
    }),
  );

  if (query.status === "loading") {
    return <Skeleton variant="list-row" count={3} />;
  }
  if (query.status === "error") {
    return <ErrorText>{text("Couldn't load events.")}</ErrorText>;
  }

  const events = [...query.data].sort((a, b) => a.eventDate.localeCompare(b.eventDate));
  if (events.length === 0) {
    return <EmptyState>{text("No upcoming events.")}</EmptyState>;
  }

  return (
    <ul className="spolka-tool-events">
      {events.map((event) => (
        <li key={event.id}>
          <DenseRow>
            <span className="num-tabular">{event.eventDate}</span> {event.title}
          </DenseRow>
        </li>
      ))}
    </ul>
  );
}
