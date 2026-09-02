import type { Company, FeedItem } from "../api/types";
import type { NotebookToolIntent } from "../screens/Spolka/route";
import { notebookTagFromFeedValue } from "./notebookForms";

// Shared root for every render site (Inbox, Spółka, Company feed): a
// "filing" is a bare official-report notice with no attachments, so its
// stored summary is the dead "Komunikat ESPI/EBI" literal — suppress it here
// once rather than forking a guard into each caller.
export function feedItemSummary(item: FeedItem) {
  if (item.presentationKind === "filing") {
    return "";
  }
  return item.summary.trim() || item.title;
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

    navigateToCompanyNotebook(company.id, {
      draft: {
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
      },
    });
  }

  return {
    feedItemSummary,
    openFeedItemNoteDraft,
  };
}
