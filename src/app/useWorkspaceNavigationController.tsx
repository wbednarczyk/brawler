import type { Dispatch, KeyboardEvent, SetStateAction } from "react";
import type { Company, FeedItem } from "../api/types";
import type { Section } from "./navigation";
import type { SpolkaTransition } from "./useSpolkaScreenWiring";

type WorkspaceNavigationControllerInput = {
  // Cross-navigation must scope the Inbox through this single reset-then-scope
  // helper — individual filter setters are deliberately not passed in, so a
  // partial reset cannot strand a stale filter that hides the feed (c80dabe).
  scopeInboxToCompany: (company: string) => void;
  selectedCompanyFeedItemId: string | null;
  selectedCompanyId: string | null;
  /** Cross-nav to the Inbox (`openCompanyInboxFilter`) only — every
   * company-workspace entry point goes through `navigate` instead (sol R1
   * finding 3), never this directly. */
  setActiveSection: Dispatch<SetStateAction<Section>>;
  setSelectedCompanyFeedItemId: Dispatch<SetStateAction<string | null>>;
  /** Arrow-key row highlight only (`focusCompanyWorkspace`) — does not enter
   * the Spółka screen, so it stays outside the guarded `navigate` seam. */
  setSelectedCompanyId: Dispatch<SetStateAction<string | null>>;
  setSelectedFeedItemId: Dispatch<SetStateAction<string | null>>;
  /** Commits companyId + section + optional tool + optional claim highlight
   * as ONE guarded transition (the ToolHost seam, F3a S2, sol R1 finding 3)
   * — used to raise the claims tool for `openCompanyClaims`. */
  navigate: (transition: SpolkaTransition) => void;
};

export function useWorkspaceNavigationController({
  scopeInboxToCompany,
  selectedCompanyFeedItemId,
  selectedCompanyId,
  setActiveSection,
  setSelectedCompanyFeedItemId,
  setSelectedCompanyId,
  setSelectedFeedItemId,
  navigate,
}: WorkspaceNavigationControllerInput) {
  // Opening a company lands the Spółka screen (F3a S1, ADR 0107) — the ONE
  // company deep-dive destination (ADR 0108).
  function openCompanyWorkspace(company: Company) {
    navigate({ companyId: company.id, section: "Spolka" });
  }

  // F3a S3 (ADR 0107 decision 2 mapping "Claims/highlightClaimId→{t:'tezy',
  // claimId}"): the seam raises the claims TOOL itself, atomically with the
  // company + section (sol R1 finding 3) — `highlightClaimId` alone (the
  // pre-F3a shape, when the curated dashboard opened claims pinned by
  // default) no longer surfaces the highlight anywhere, since a fresh Spółka
  // screen opens on the core, not a tool.
  function openCompanyClaims(companyId: string, claimId: string) {
    navigate({ companyId, section: "Spolka", tool: { t: "tezy", claimId }, highlightClaimId: claimId });
  }

  // Arrow-key navigation in the company library only moves the highlighted row;
  // it must not yank the whole app into the Spółka screen on every keypress.
  function focusCompanyWorkspace(companyId: string) {
    setSelectedCompanyId(companyId);
  }

  function openCompanyWorkspaceFromKeyboard(event: KeyboardEvent<HTMLElement>, company: Company) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      openCompanyWorkspace(company);
    }

    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();

      const rows = Array.from(
        event.currentTarget
          .closest("[data-company-list='true']")
          ?.querySelectorAll<HTMLElement>("[data-company-row='true']") ?? [],
      );
      const currentIndex = rows.indexOf(event.currentTarget);
      const direction = event.key === "ArrowDown" ? 1 : -1;
      const nextIndex = Math.min(Math.max(currentIndex + direction, 0), rows.length - 1);
      const nextRow = rows[nextIndex];
      // The focusable row control (data-company-row) is the primary <button>; the
      // company id lives on its container (data-company-id).
      const nextCompanyId = nextRow?.closest<HTMLElement>("[data-company-id]")?.dataset.companyId;

      if (nextRow && nextCompanyId) {
        nextRow.focus();
        if (selectedCompanyId) {
          focusCompanyWorkspace(nextCompanyId);
        }
      }
    }
  }

  function openCompanyInboxFilter(company: Company) {
    setSelectedFeedItemId(null);
    scopeInboxToCompany(company.qualifiedTicker);
    setActiveSection("Inbox");
  }

  function toggleCompanyFeedItem(item: FeedItem) {
    setSelectedCompanyFeedItemId((current) => (current === item.id ? null : item.id));
  }

  function selectCompanyFeedItemFromKeyboard(event: KeyboardEvent<HTMLElement>, item: FeedItem) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      toggleCompanyFeedItem(item);
    }

    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();

      const rows = Array.from(
        event.currentTarget
          .closest("[data-company-feed-list='true']")
          ?.querySelectorAll<HTMLElement>("[data-company-feed-row='true']") ?? [],
      );
      const currentIndex = rows.indexOf(event.currentTarget);
      const direction = event.key === "ArrowDown" ? 1 : -1;
      const nextIndex = Math.min(Math.max(currentIndex + direction, 0), rows.length - 1);
      const nextRow = rows[nextIndex];
      const nextFeedItemId = nextRow?.dataset.companyFeedItemId;

      if (nextRow && nextFeedItemId) {
        nextRow.focus();
        if (selectedCompanyFeedItemId) {
          setSelectedCompanyFeedItemId(nextFeedItemId);
        }
      }
    }
  }

  return {
    focusCompanyWorkspace,
    openCompanyClaims,
    openCompanyInboxFilter,
    openCompanyWorkspace,
    openCompanyWorkspaceFromKeyboard,
    selectCompanyFeedItemFromKeyboard,
    toggleCompanyFeedItem,
  };
}
