import {
  ArrowLeft,
  BookOpenText,
  Building2,
  ExternalLink,
  FileText,
  Mail,
  MailOpen,
  Save,
} from "lucide-react";
import { ActionRow, Button, ErrorText } from "../../ui";
import { CompanyContextSection } from "../../shared/components/feedDetail/CompanyContextSection";
import { FeedDetailContent } from "../../shared/components/feedDetail/FeedDetailContent";
import { useLocale } from "../../shared/locale";
import type { CompanySignal, FeedItem } from "../../api/types";
import type { InboxScreenProps } from "./inboxTypes";

type InboxDetailPaneProps = Pick<
  InboxScreenProps,
  | "selectedFeedItem"
  | "selectedFeedCompany"
  | "healthError"
  | "databaseError"
  | "updateSelectedFeedItem"
  | "openCompanyWorkspaceFromFeedItem"
  | "openFeedItemNoteDraft"
  | "confirmCompanySignal"
  | "rejectCompanySignal"
  | "feedItemSummary"
  | "formatTimestamp"
> & {
  selectedFeedSignals: CompanySignal[] | undefined;
  // At the S width tier (ADR 0076 D6) the detail is a full-pane overlay over the
  // list, raised when `detailOpen` is set (presentation-only — the list keeps its
  // selection). The back control calls `onBack` to lower the overlay.
  detailOpen: boolean;
  onBack: () => void;
};

export function InboxDetailPane({
  selectedFeedItem,
  selectedFeedCompany,
  selectedFeedSignals,
  healthError,
  databaseError,
  updateSelectedFeedItem,
  openCompanyWorkspaceFromFeedItem,
  openFeedItemNoteDraft,
  confirmCompanySignal,
  rejectCompanySignal,
  feedItemSummary,
  formatTimestamp,
  detailOpen,
  onBack,
}: InboxDetailPaneProps) {
  const { text } = useLocale();
  const signals = selectedFeedSignals ?? [];

  return (
    <aside
      className="detail-pane"
      aria-label={text("Feed item details")}
      data-detail-open={detailOpen ? "true" : undefined}
    >
      {selectedFeedItem ? (
        <Button
          aria-label={text("Back to list")}
          className="feed-detail-back compact-button"
          onClick={onBack}
          variant="minimal"
        >
          <ArrowLeft size={15} />
          {text("Back to list")}
        </Button>
      ) : null}
      {selectedFeedItem ? (
        <>
          <FeedDetailContent
            item={selectedFeedItem}
            signals={signals}
            actions={buildDetailActions({
              item: selectedFeedItem,
              company: selectedFeedCompany,
              text,
              updateSelectedFeedItem,
              openCompanyWorkspaceFromFeedItem,
              openFeedItemNoteDraft,
            })}
            feedItemSummary={feedItemSummary}
            formatTimestamp={formatTimestamp}
            onConfirmSignal={confirmCompanySignal}
            onRejectSignal={rejectCompanySignal}
          />
          {selectedFeedCompany ? (
            <>
              <div className="detail-context-divider" />
              <CompanyContextSection
                companyId={selectedFeedCompany.id}
                // Same navigation the report primary action already uses (S4)
                // — the provenance thread lands on the company's dashboard,
                // where the report-documents panel is part of the default set.
                onOpenReportDocuments={() => openCompanyWorkspaceFromFeedItem(selectedFeedItem)}
              />
            </>
          ) : null}
        </>
      ) : (
        <>
          <div className="detail-icon">
            <FileText size={24} />
          </div>
          <h2>{text("No item selected")}</h2>
          <p>{text("Select a feed item to inspect source details and origin links.")}</p>
        </>
      )}
      {healthError ? <ErrorText>{text("Health command failed")}: {healthError}</ErrorText> : null}
      {databaseError ? <ErrorText>{text("Workspace refresh failed")}: {databaseError}</ErrorText> : null}
    </aside>
  );
}

type BuildDetailActionsArgs = {
  item: FeedItem;
  company: InboxDetailPaneProps["selectedFeedCompany"];
  text: (value: string) => string;
  updateSelectedFeedItem: InboxDetailPaneProps["updateSelectedFeedItem"];
  openCompanyWorkspaceFromFeedItem: InboxDetailPaneProps["openCompanyWorkspaceFromFeedItem"];
  openFeedItemNoteDraft: InboxDetailPaneProps["openFeedItemNoteDraft"];
};

// The primary/secondary action buttons for the detail's actions slot.
//
// The per-kind primary/secondary pair follows the experience contract (§6)
// and the approved mockups exactly: media/filing → open the source, marked
// primary; report → read the report (no reportDocumentId reaches Inbox yet,
// so this opens the company workspace where the real KPI-reading flow —
// Extract data on a stored document — lives), open source stays secondary.
//
// Deviation from the mockups (documented, not silent): the mockups show only
// that primary/secondary pair, but the pre-redesign detail also let a user
// save/bookmark an item and draft a notebook note from it — real, reachable
// capabilities the mockups didn't re-litigate. Dropping them for media/
// filing/report would be a silent regression ("a capability is not done
// until a user can reach it" — CLAUDE.md), so Save/Note (+ Open company for
// media/filing, where it's not already the primary action) stay available
// after the mockup's pair on every kind, never marked primary. redFlag/
// unknown keeps the exact pre-redesign action row.
function buildDetailActions({
  item,
  company,
  text,
  updateSelectedFeedItem,
  openCompanyWorkspaceFromFeedItem,
  openFeedItemNoteDraft,
}: BuildDetailActionsArgs) {
  const toggleRead = () =>
    updateSelectedFeedItem((current) => ({ ...current, unread: !current.unread }));
  const markReadButton = (
    <Button className="compact-button" onClick={toggleRead}>
      {item.unread ? <MailOpen size={15} /> : <Mail size={15} />}
      {item.unread ? text("Mark read") : text("Mark unread")}
    </Button>
  );
  const saveButton = (
    <Button
      className="compact-button"
      onClick={() => updateSelectedFeedItem((current) => ({ ...current, saved: !current.saved }))}
    >
      <Save size={15} />
      {item.saved ? text("Unsave") : text("Save")}
    </Button>
  );
  const openCompanyButton = company ? (
    <Button className="compact-button" onClick={() => openCompanyWorkspaceFromFeedItem(item)}>
      <Building2 size={15} />
      {text("Open company")}
    </Button>
  ) : null;
  const noteButton = company ? (
    <Button className="compact-button" onClick={() => openFeedItemNoteDraft(item)}>
      <BookOpenText size={15} />
      {text("Note")}
    </Button>
  ) : null;
  const openSourceLink = (primary: boolean, label: string) => (
    <a
      className={primary ? "primary-button compact-button" : "secondary-button compact-button"}
      data-ux-primary-action={primary ? "true" : undefined}
      href={item.sourceUrl}
      rel="noreferrer"
      target="_blank"
      // Contract §7: `Otwórz…` marks the item read on its way out.
      onClick={() => updateSelectedFeedItem((current) => ({ ...current, unread: false }))}
    >
      <ExternalLink size={15} />
      {label}
    </a>
  );

  if (item.presentationKind === "media") {
    return (
      <ActionRow className="detail-actions" ariaLabel={text("Feed item actions")}>
        {openSourceLink(true, text("Open source"))}
        {markReadButton}
        {saveButton}
        {openCompanyButton}
        {noteButton}
      </ActionRow>
    );
  }

  if (item.presentationKind === "filing") {
    return (
      <ActionRow className="detail-actions" ariaLabel={text("Feed item actions")}>
        {openSourceLink(true, text("Open notice"))}
        {markReadButton}
        {saveButton}
        {openCompanyButton}
        {noteButton}
      </ActionRow>
    );
  }

  if (item.presentationKind === "report") {
    return (
      <ActionRow className="detail-actions" ariaLabel={text("Feed item actions")}>
        <Button
          className="compact-button"
          data-ux-primary-action="true"
          onClick={() => openCompanyWorkspaceFromFeedItem(item)}
          variant="primary"
        >
          {text("Read the report")}
        </Button>
        {openSourceLink(false, text("Open source"))}
        {markReadButton}
        {saveButton}
        {noteButton}
      </ActionRow>
    );
  }

  // redFlag / unknown: the pre-redesign action row, unchanged.
  return (
    <ActionRow className="detail-actions" ariaLabel={text("Feed item actions")}>
      {markReadButton}
      {saveButton}
      {openCompanyButton}
      {noteButton}
    </ActionRow>
  );
}
