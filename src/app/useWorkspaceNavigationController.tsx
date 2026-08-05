import type { Dispatch, KeyboardEvent, SetStateAction } from "react";
import { ExternalLink, Inbox } from "lucide-react";
import type { Company, FeedItem, NotebookOrigin } from "../api/types";
import type { Section } from "./navigation";

type WorkspaceNavigationControllerInput = {
  companiesById: Record<string, Company>;
  feedState: FeedItem[];
  // Cross-navigation must scope the Inbox through this single reset-then-scope
  // helper — individual filter setters are deliberately not passed in, so a
  // partial reset cannot strand a stale filter that hides the feed (c80dabe).
  scopeInboxToCompany: (company: string) => void;
  selectedCompanyFeedItemId: string | null;
  selectedCompanyId: string | null;
  setActiveSection: Dispatch<SetStateAction<Section>>;
  setSelectedCompanyFeedItemId: Dispatch<SetStateAction<string | null>>;
  setSelectedCompanyId: Dispatch<SetStateAction<string | null>>;
  setSelectedFeedItemId: Dispatch<SetStateAction<string | null>>;
  setCockpitInitialCompanyId: Dispatch<SetStateAction<string | null>>;
};

export function useWorkspaceNavigationController({
  companiesById,
  feedState,
  scopeInboxToCompany,
  selectedCompanyFeedItemId,
  selectedCompanyId,
  setActiveSection,
  setSelectedCompanyFeedItemId,
  setSelectedCompanyId,
  setSelectedFeedItemId,
  setCockpitInitialCompanyId,
}: WorkspaceNavigationControllerInput) {
  // Opening a company lands the curated dashboard scoped to it (ADR 0057): the
  // cockpit is the single company deep-dive. The library selection is kept in
  // sync so the row stays highlighted.
  function openCompanyWorkspace(company: Company) {
    setSelectedCompanyId(company.id);
    setCockpitInitialCompanyId(company.id);
    setActiveSection("Cockpit");
  }

  // Arrow-key navigation in the company library only moves the highlighted row;
  // it must not yank the whole app into the cockpit on every keypress.
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

  function openOriginFeedItem(origin: NotebookOrigin, companyId: string) {
    const company = companiesById[companyId];
    const originFeedItem = origin.sourceId
      ? feedState.find((item) => item.id === origin.sourceId)
      : null;

    scopeInboxToCompany(company?.qualifiedTicker ?? originFeedItem?.company ?? "all");

    if (origin.sourceId) {
      setSelectedFeedItemId(origin.sourceId);
    }

    setActiveSection("Inbox");
  }

  function renderNotebookOrigins(origins: NotebookOrigin[], companyId: string) {
    if (origins.length === 0) {
      return <span className="membership-empty">None</span>;
    }

    return (
      <div className="origin-link-list">
        {origins.map((origin) => {
          const label = origin.label ?? origin.sourceType.replace("_", " ");
          const canOpenFeedItem = origin.sourceType === "feed_item" && Boolean(origin.sourceId);
          const hasOriginActions = canOpenFeedItem || Boolean(origin.sourceUrl);

          return (
            <div className="origin-link" key={origin.id}>
              <span>{label}</span>
              {hasOriginActions ? (
                <div className="origin-actions">
                  {canOpenFeedItem ? (
                    <button
                      aria-label={`Open origin feed item: ${label}`}
                      className="secondary-button compact-button"
                      onClick={() => openOriginFeedItem(origin, companyId)}
                      type="button"
                    >
                      <Inbox size={14} />
                      Feed item
                    </button>
                  ) : null}
                  {origin.sourceUrl ? (
                    <a
                      aria-label={`Open origin source: ${label}`}
                      className="secondary-button compact-button"
                      href={origin.sourceUrl}
                      rel="noreferrer"
                      target="_blank"
                    >
                      <ExternalLink size={14} />
                      Source
                    </a>
                  ) : null}
                </div>
              ) : null}
            </div>
          );
        })}
      </div>
    );
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
    openCompanyInboxFilter,
    openCompanyWorkspace,
    openCompanyWorkspaceFromKeyboard,
    renderNotebookOrigins,
    selectCompanyFeedItemFromKeyboard,
    toggleCompanyFeedItem,
  };
}
