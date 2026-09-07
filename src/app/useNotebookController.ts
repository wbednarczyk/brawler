import type { Company, FeedItem } from "../api/types";
import type { NotebookDraft, NotebookToolIntent } from "../screens/Spolka/route";
import { notebookTagFromFeedValue } from "./notebookForms";

// The dead ESPI/EBI filing-notice literal (F1 #413) — a bare official-report
// NOTICE (`presentationKind: "filing"`) always carries it, but an
// attachment-bearing `report` item can ALSO carry it verbatim when its own
// summary hasn't been parsed yet (`report_documents.rs:212`); dogfooding #9
// caught it leaking through the Company feed row because the old guard only
// checked `presentationKind === "filing"`.
const DEAD_FILING_SUMMARY_LITERAL = "Komunikat ESPI/EBI";

// Shared root for every render site (Inbox, Spółka, Company feed): suppress
// the dead literal — by kind (filing, always) and by exact match (any kind
// that happens to carry it verbatim) — once here rather than forking a guard
// into each caller. A meaningful summary is never touched.
export function feedItemSummary(item: FeedItem) {
  if (item.presentationKind === "filing") {
    return "";
  }
  const summary = item.summary.trim();
  if (summary === DEAD_FILING_SUMMARY_LITERAL) {
    return "";
  }
  return summary || item.title;
}

// The origin-attributed draft a feed item seeds into the `notatnik` composer
// (F4c S2). Extracted so both the cross-company entry point below
// (`openFeedItemNoteDraft`, which still needs to LOCATE the item's company)
// and a same-company caller that already knows its company (the Spółka feed
// tool, sol fix1 item 3) build the identical draft shape — never two forks
// of this origin-attribution logic.
export function buildFeedItemNoteDraft(item: FeedItem): NotebookDraft {
  return {
    form: {
      title: item.title,
      body: item.bodyText || feedItemSummary(item),
      tags: ["feed", notebookTagFromFeedValue(item.type), notebookTagFromFeedValue(item.source)]
        .filter(Boolean)
        .join(", "),
      kind: "observation",
      claimStatus: "",
      eventDate: "",
      followUpAfter: "",
      followUpDate: "",
    },
    origins: [
      {
        sourceType: "feed_item",
        sourceId: item.id,
        sourceUrl: item.sourceUrl,
        label: `${item.source}: ${item.title}`,
      },
    ],
  };
}

type NotebookControllerInput = {
  companies: Company[];
  // The ONE landing point for the Spółka `notatnik` tool (F4c S2, ADR 0108
  // amendment, sol re-review): company + section + tool as ONE guarded
  // transition (`useSpolkaNavigate`), never `spolkaTool.openTool` (which only
  // commits tool state and neither selects the company nor activates Spółka).
  navigateToCompanyNotebook: (companyId: string, intent: NotebookToolIntent) => void;
};

// Screen-only note-taking (composer/edit forms, filters, the Notebooks-global
// list) retired with the screen (F4c S2) — the per-company panel owns that
// state now (`useCompanyNotebookPanel`). This controller keeps only the two
// things every render site still needs: the shared summary root and the
// feed-item-to-draft entry point.
export function useNotebookController({ companies, navigateToCompanyNotebook }: NotebookControllerInput) {
  function findCompanyForFeedItem(item: FeedItem) {
    return companies.find((company) => company.qualifiedTicker === item.company) ?? null;
  }

  function openFeedItemNoteDraft(item: FeedItem) {
    const company = findCompanyForFeedItem(item);

    if (!company) {
      return;
    }

    navigateToCompanyNotebook(company.id, { draft: buildFeedItemNoteDraft(item) });
  }

  return {
    feedItemSummary,
    openFeedItemNoteDraft,
  };
}
