import type { Company, FeedItem } from "../api/types";
import type { NotebookDraft, NotebookToolIntent } from "../screens/Spolka/route";
import { notebookTagFromFeedValue } from "./notebookForms";

// The backend's placeholder summary for an unparsed official notice; any
// kind may carry it verbatim (`report_documents.rs`), so it is suppressed by
// exact match here, the one root every render site shares.
const DEAD_FILING_SUMMARY_LITERAL = "Komunikat ESPI/EBI";

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
