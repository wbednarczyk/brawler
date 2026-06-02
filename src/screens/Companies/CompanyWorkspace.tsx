import {
  BookOpenText,
  CheckCircle2,
  ExternalLink,
  FileText,
  Inbox,
  Mail,
  MailOpen,
  Plus,
  Save,
  Video,
  X,
} from "lucide-react";
import type { Company } from "../../api/types";
import { Button } from "../../shared/components/Button";
import { EmptyState } from "../../shared/components/EmptyState";
import { StatusPill } from "../../shared/components/StatusPill";
import type { CompaniesScreenProps } from "./CompaniesScreen";

type CompanyWorkspaceProps = Pick<
  CompaniesScreenProps,
  | "membershipsByCompany"
  | "selectedCompanyFeedStats"
  | "companyWorkspaceTab"
  | "selectedCompanyFeedItems"
  | "selectedCompanyFeedItem"
  | "selectedCompanyNotebookEntries"
  | "isNotebookComposerOpen"
  | "notebookForm"
  | "selectedNotebookEntryId"
  | "selectedNotebookEntry"
  | "notebookEditMode"
  | "notebookEditForm"
  | "isNotebookEditDirty"
  | "notebookError"
  | "selectedCompanyClaimEntries"
  | "selectedClaimEntry"
  | "claimStatusDraft"
  | "setCompanyWorkspaceTab"
  | "toggleCompanyFeedItem"
  | "selectCompanyFeedItemFromKeyboard"
  | "updateFeedItemState"
  | "inspectCompanyFeedItem"
  | "openFeedItemNoteDraft"
  | "openCompanyInboxFilter"
  | "setNotebookComposerOpen"
  | "updateNotebookForm"
  | "createNotebookEntry"
  | "setSelectedNotebookEntryId"
  | "saveNotebookEntry"
  | "cancelNotebookEdit"
  | "setNotebookEditMode"
  | "updateNotebookEditForm"
  | "toggleClaimEntry"
  | "setClaimStatusDraft"
  | "saveClaimStatus"
  | "NotebookDateField"
  | "NotebookQuarterField"
  | "MarkdownNoteBody"
  | "renderNotebookOrigins"
  | "formatTimestamp"
  | "feedItemSummary"
> & {
  selectedCompany: Company;
};

export function CompanyWorkspace({
  selectedCompany,
  membershipsByCompany,
  selectedCompanyFeedStats,
  companyWorkspaceTab,
  selectedCompanyFeedItems,
  selectedCompanyFeedItem,
  selectedCompanyNotebookEntries,
  isNotebookComposerOpen,
  notebookForm,
  selectedNotebookEntryId,
  selectedNotebookEntry,
  notebookEditMode,
  notebookEditForm,
  isNotebookEditDirty,
  notebookError,
  selectedCompanyClaimEntries,
  selectedClaimEntry,
  claimStatusDraft,
  setCompanyWorkspaceTab,
  toggleCompanyFeedItem,
  selectCompanyFeedItemFromKeyboard,
  updateFeedItemState,
  inspectCompanyFeedItem,
  openFeedItemNoteDraft,
  openCompanyInboxFilter,
  setNotebookComposerOpen,
  updateNotebookForm,
  createNotebookEntry,
  setSelectedNotebookEntryId,
  saveNotebookEntry,
  cancelNotebookEdit,
  setNotebookEditMode,
  updateNotebookEditForm,
  toggleClaimEntry,
  setClaimStatusDraft,
  saveClaimStatus,
  NotebookDateField,
  NotebookQuarterField,
  MarkdownNoteBody,
  renderNotebookOrigins,
  formatTimestamp,
  feedItemSummary,
}: CompanyWorkspaceProps) {
  return (
    <section className="company-workspace" aria-label="Company workspace">
      <div className="company-workspace-header">
        <div>
          <span className="eyebrow">Company workspace</span>
          <h2>{selectedCompany.qualifiedTicker}</h2>
          <p>{selectedCompany.displayName}</p>
        </div>
        <div className="company-workspace-meta" aria-label="Selected company metadata">
          <span>{selectedCompany.exchange}</span>
          <span>{selectedCompany.isin ?? "No ISIN"}</span>
          <span>{selectedCompanyFeedStats.total} feed</span>
          <span>{selectedCompanyFeedStats.unread} unread</span>
          <span>{selectedCompanyFeedStats.saved} saved</span>
          {(membershipsByCompany[selectedCompany.id] ?? []).map((membership) => (
            <span key={membership.watchlistId}>{membership.watchlistName}</span>
          ))}
        </div>
      </div>
    
      <div className="segmented-control company-tabs" aria-label="Company workspace tabs">
        {(["Feed", "Notebook", "Claims", "Transcripts", "Metadata"] as const).map(
          (tab) => {
            const TabIcon =
              tab === "Feed"
                ? Inbox
                : tab === "Notebook"
                  ? BookOpenText
                  : tab === "Claims"
                    ? CheckCircle2
                    : tab === "Transcripts"
                      ? Video
                      : FileText;
    
            return (
              <button
                className={companyWorkspaceTab === tab ? "segment-active" : undefined}
                key={tab}
                onClick={() => setCompanyWorkspaceTab(tab)}
                type="button"
              >
                <TabIcon size={14} />
                {tab}
              </button>
            );
          },
        )}
      </div>
    
      {companyWorkspaceTab === "Feed" ? (
        <div
          className="company-tab-panel"
          aria-label="Company feed"
          data-company-feed-list="true"
        >
          {selectedCompanyFeedItems.map((item) => (
            <div className="company-feed-row-block" key={item.id}>
              <article
                aria-label={`Open company feed item: ${item.title}`}
                className={[
                  "company-feed-row",
                  item.unread ? "unread" : "",
                  selectedCompanyFeedItem?.id === item.id
                    ? "company-feed-row-selected"
                    : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
                data-company-feed-item-id={item.id}
                data-company-feed-row="true"
                onClick={() => toggleCompanyFeedItem(item)}
                onKeyDown={(event) => selectCompanyFeedItemFromKeyboard(event, item)}
                role="button"
                tabIndex={0}
                title="Open company feed item details"
              >
                <div className="feed-row-main">
                  <div className="feed-meta">
                    <span>{item.type}</span>
                    <span>{item.source}</span>
                    <span>{formatTimestamp(item.time, "Unknown")}</span>
                  </div>
                  <h3>{item.title}</h3>
                  <p>{feedItemSummary(item)}</p>
                </div>
                {item.saved ? <span className="saved-pill">Saved</span> : null}
                {item.unread ? <span className="unread-dot" title="Unread" /> : null}
              </article>
    
              {selectedCompanyFeedItem?.id === item.id ? (
                <aside className="company-feed-detail" aria-label="Company feed item details">
                  <div>
                    <span className="eyebrow">Selected item</span>
                    <h3>{selectedCompanyFeedItem.title}</h3>
                    <section className="feed-body-section" aria-label="Feed summary">
                      <div className="feed-body-heading">
                        <span>Summary</span>
                      </div>
                      <p className="feed-detail-body">{feedItemSummary(selectedCompanyFeedItem)}</p>
                    </section>
                    <details className="feed-body-section feed-body-disclosure" aria-label="Official report body">
                      <summary className="feed-body-heading">
                        <span>Official report body</span>
                        <strong>{selectedCompanyFeedItem.bodyText ? "Stored" : "Not stored"}</strong>
                      </summary>
                      {selectedCompanyFeedItem.bodyText ? (
                        <p className="feed-detail-body">{selectedCompanyFeedItem.bodyText}</p>
                      ) : (
                        <p className="feed-detail-empty">
                          No official report body is stored for this item yet. Refresh sources and
                          check Sources for detail warnings if this remains empty.
                        </p>
                      )}
                    </details>
                  </div>
                  <div className="detail-actions" aria-label="Company feed item actions">
                    <Button
                      className="compact-button"
                      onClick={() =>
                        updateFeedItemState(selectedCompanyFeedItem, (feedItem) => ({
                          ...feedItem,
                          unread: !feedItem.unread,
                        }))
                      }
                    >
                      {selectedCompanyFeedItem.unread ? (
                        <MailOpen size={15} />
                      ) : (
                        <Mail size={15} />
                      )}
                      {selectedCompanyFeedItem.unread ? "Mark read" : "Mark unread"}
                    </Button>
                    <Button
                      className="compact-button"
                      onClick={() =>
                        updateFeedItemState(selectedCompanyFeedItem, (feedItem) => ({
                          ...feedItem,
                          saved: !feedItem.saved,
                        }))
                      }
                    >
                      <Save size={15} />
                      {selectedCompanyFeedItem.saved ? "Unsave" : "Save"}
                    </Button>
                    <Button
                      className="compact-button"
                      onClick={() => inspectCompanyFeedItem(selectedCompanyFeedItem)}
                    >
                      <Inbox size={15} />
                      Open in Inbox
                    </Button>
                    <Button
                      className="compact-button"
                      onClick={() => openFeedItemNoteDraft(selectedCompanyFeedItem)}
                    >
                      <BookOpenText size={15} />
                      Note
                    </Button>
                    <a
                      className="secondary-button compact-button"
                      href={selectedCompanyFeedItem.sourceUrl}
                      rel="noreferrer"
                      target="_blank"
                    >
                      <ExternalLink size={15} />
                      Open source
                    </a>
                  </div>
                  <dl className="metadata-grid">
                    <div>
                      <dt>Source</dt>
                      <dd>{selectedCompanyFeedItem.source}</dd>
                    </div>
                    <div>
                      <dt>Type</dt>
                      <dd>{selectedCompanyFeedItem.type}</dd>
                    </div>
                    <div>
                      <dt>Published</dt>
                      <dd>{formatTimestamp(selectedCompanyFeedItem.publishedAt, "Unknown")}</dd>
                    </div>
                    <div>
                      <dt>Fetched</dt>
                      <dd>{formatTimestamp(selectedCompanyFeedItem.fetchedAt, "Unknown")}</dd>
                    </div>
                    <div>
                      <dt>Attribution</dt>
                      <dd>{selectedCompanyFeedItem.attribution}</dd>
                    </div>
                    <div>
                      <dt>Language</dt>
                      <dd>{selectedCompanyFeedItem.language}</dd>
                    </div>
                  </dl>
                  {selectedCompanyFeedItem.attachments.length > 0 ? (
                    <div className="feed-attachment-list" aria-label="Company feed attachments">
                      {selectedCompanyFeedItem.attachments.map((attachment) => (
                        <a
                          className="feed-attachment-link"
                          href={attachment.url}
                          key={attachment.id}
                          rel="noreferrer"
                          target="_blank"
                        >
                          <ExternalLink size={14} />
                          {attachment.label}
                        </a>
                      ))}
                    </div>
                  ) : null}
                </aside>
              ) : null}
            </div>
          ))}
          {selectedCompanyFeedItems.length === 0 ? (
            <EmptyState className="company-feed-empty" wrapText={false}>
              <div>
                <strong>No stored feed items for {selectedCompany.qualifiedTicker} yet.</strong>
                <p>
                  This company is tracked locally, but no sample or ingested items are attached to
                  it yet.
                </p>
              </div>
              <Button
                className="compact-button"
                onClick={() => openCompanyInboxFilter(selectedCompany)}
            >
              <Inbox size={15} />
              Open filtered Inbox
            </Button>
            </EmptyState>
          ) : null}
        </div>
      ) : null}
    
      {companyWorkspaceTab === "Notebook" ? (
        <div className="company-tab-panel notebook-panel" aria-label="Company notebook">
          <div className="notebook-toolbar">
            <div>
              <h3>Notebook</h3>
              <p>
                {selectedCompanyNotebookEntries.length} note
                {selectedCompanyNotebookEntries.length === 1 ? "" : "s"} for{" "}
                {selectedCompany.qualifiedTicker}
              </p>
            </div>
            <Button
              className="compact-button"
              onClick={() => setNotebookComposerOpen((current) => !current)}
              variant="primary"
            >
              {isNotebookComposerOpen ? <X size={15} /> : <Plus size={15} />}
              {isNotebookComposerOpen ? "Hide form" : "New note"}
            </Button>
          </div>
    
          {isNotebookComposerOpen ? (
            <form className="notebook-form" onSubmit={createNotebookEntry}>
              <div className="notebook-form-grid">
                <label>
                  Title
                  <input
                    aria-label="Notebook note title"
                    value={notebookForm.title}
                    onChange={(event) => updateNotebookForm("title", event.target.value)}
                  />
                </label>
                <label>
                  Kind
                  <select
                    aria-label="Notebook note kind"
                    value={notebookForm.kind}
                    onChange={(event) => updateNotebookForm("kind", event.target.value)}
                  >
                    <option value="manual">Manual</option>
                    <option value="observation">Observation</option>
                    <option value="claim">Claim</option>
                    <option value="question">Question</option>
                    <option value="follow_up">Follow-up</option>
                  </select>
                </label>
                <label>
                  Tags
                  <input
                    aria-label="Notebook note tags"
                    placeholder="comma, separated"
                    value={notebookForm.tags}
                    onChange={(event) => updateNotebookForm("tags", event.target.value)}
                  />
                </label>
                <label>
                  Claim status
                  <select
                    aria-label="Notebook claim status"
                    value={notebookForm.claimStatus}
                    onChange={(event) => updateNotebookForm("claimStatus", event.target.value)}
                  >
                    <option value="">None</option>
                    <option value="open">Open</option>
                    <option value="delivered">Delivered</option>
                    <option value="partially_delivered">Partially delivered</option>
                    <option value="missed">Missed</option>
                    <option value="unknown">Unknown</option>
                    <option value="not_applicable">Not applicable</option>
                  </select>
                </label>
                <NotebookDateField
                  ariaLabel="Notebook event date"
                  label="Event date"
                  value={notebookForm.eventDate}
                  onChange={(value) => updateNotebookForm("eventDate", value)}
                />
                <NotebookQuarterField
                  ariaLabel="Notebook follow-up quarter"
                  label="Follow-up quarter"
                  value={notebookForm.followUpAfter}
                  onChange={(value) => updateNotebookForm("followUpAfter", value)}
                />
                <NotebookDateField
                  ariaLabel="Notebook follow-up date"
                  label="Follow-up date"
                  value={notebookForm.followUpDate}
                  onChange={(value) => updateNotebookForm("followUpDate", value)}
                />
                <Button
                  className="compact-button notebook-submit-button"
                  disabled={!notebookForm.title.trim() || !notebookForm.body.trim()}
                  type="submit"
                  variant="primary"
                >
                  <Save size={15} />
                  Save
                </Button>
              </div>
              <label className="notebook-body-field">
                Body
                <textarea
                  aria-label="Notebook note body"
                  value={notebookForm.body}
                  onChange={(event) => updateNotebookForm("body", event.target.value)}
                />
              </label>
            </form>
          ) : null}
    
          <div className="notebook-workspace">
            <div className="notebook-list" aria-label="Notebook entries">
              {selectedCompanyNotebookEntries.map((entry) => (
                <button
                  aria-label={`Select notebook entry: ${entry.title}`}
                  className={[
                    "notebook-row",
                    selectedNotebookEntry?.id === entry.id ? "notebook-row-selected" : "",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                  key={entry.id}
                  onClick={() => setSelectedNotebookEntryId(entry.id)}
                  type="button"
                >
                  <div>
                    <div className="notebook-row-top">
                      <h3>{entry.title}</h3>
                      <span>{entry.kind.replace("_", " ")}</span>
                    </div>
                  </div>
                  <div className="notebook-row-meta">
                    {entry.claimStatus ? <span>{entry.claimStatus.replace("_", " ")}</span> : null}
                    {entry.followUpAfter ? <span>{entry.followUpAfter}</span> : null}
                    {entry.tags.slice(0, 2).map((tag) => (
                      <span key={tag}>{tag}</span>
                    ))}
                  </div>
                </button>
              ))}
              {selectedCompanyNotebookEntries.length === 0 ? (
                <EmptyState>No notebook entries for {selectedCompany.qualifiedTicker} yet.</EmptyState>
              ) : null}
            </div>
            <form
              className="notebook-detail"
              aria-label="Notebook entry detail"
              onSubmit={saveNotebookEntry}
            >
              {selectedNotebookEntry ? (
                notebookEditMode ? (
                  <>
                    <div className="notebook-entry-header">
                      <label>
                        Title
                        <input
                          aria-label="Selected notebook title"
                          value={notebookEditForm.title}
                          onChange={(event) =>
                            updateNotebookEditForm("title", event.target.value)
                          }
                        />
                      </label>
                      <div className="notebook-detail-actions">
                        <Button
                          className="compact-button"
                          onClick={cancelNotebookEdit}
                        >
                          <X size={15} />
                          Cancel
                        </Button>
                        <Button
                          className="compact-button"
                          disabled={
                            !isNotebookEditDirty ||
                            !notebookEditForm.title.trim() ||
                            !notebookEditForm.body.trim()
                          }
                          type="submit"
                          variant="primary"
                        >
                          <Save size={15} />
                          Save
                        </Button>
                      </div>
                    </div>
                    <textarea
                      aria-label="Selected notebook body"
                      value={notebookEditForm.body}
                      onChange={(event) => updateNotebookEditForm("body", event.target.value)}
                    />
                    <div className="notebook-detail-grid">
                      <label>
                        Kind
                        <select
                          aria-label="Selected notebook kind"
                          value={notebookEditForm.kind}
                          onChange={(event) => updateNotebookEditForm("kind", event.target.value)}
                        >
                          <option value="manual">Manual</option>
                          <option value="observation">Observation</option>
                          <option value="claim">Claim</option>
                          <option value="question">Question</option>
                          <option value="follow_up">Follow-up</option>
                        </select>
                      </label>
                      <label>
                        Claim status
                        <select
                          aria-label="Selected notebook claim status"
                          value={notebookEditForm.claimStatus}
                          onChange={(event) =>
                            updateNotebookEditForm("claimStatus", event.target.value)
                          }
                        >
                          <option value="">None</option>
                          <option value="open">Open</option>
                          <option value="delivered">Delivered</option>
                          <option value="partially_delivered">Partially delivered</option>
                          <option value="missed">Missed</option>
                          <option value="unknown">Unknown</option>
                          <option value="not_applicable">Not applicable</option>
                        </select>
                      </label>
                      <label>
                        Tags
                        <input
                          aria-label="Selected notebook tags"
                          value={notebookEditForm.tags}
                          onChange={(event) => updateNotebookEditForm("tags", event.target.value)}
                        />
                      </label>
                      <NotebookDateField
                        ariaLabel="Selected notebook event date"
                        label="Event date"
                        value={notebookEditForm.eventDate}
                        onChange={(value) => updateNotebookEditForm("eventDate", value)}
                      />
                      <NotebookQuarterField
                        ariaLabel="Selected notebook follow-up quarter"
                        label="Follow-up quarter"
                        value={notebookEditForm.followUpAfter}
                        onChange={(value) => updateNotebookEditForm("followUpAfter", value)}
                      />
                      <NotebookDateField
                        ariaLabel="Selected notebook follow-up date"
                        label="Follow-up date"
                        value={notebookEditForm.followUpDate}
                        onChange={(value) => updateNotebookEditForm("followUpDate", value)}
                      />
                    </div>
                  </>
                ) : (
                  <>
                    <div className="notebook-entry-header">
                      <div>
                        <span className="eyebrow">
                          {selectedNotebookEntry.kind.replace("_", " ")}
                        </span>
                        <h3>{selectedNotebookEntry.title}</h3>
                      </div>
                      <Button
                        className="compact-button"
                        onClick={() => setNotebookEditMode(true)}
                      >
                        <BookOpenText size={15} />
                        Edit
                      </Button>
                    </div>
                    <MarkdownNoteBody
                      ariaLabel="Selected notebook body"
                      body={selectedNotebookEntry.body}
                    />
                  </>
                )
              ) : (
                  <EmptyState>Select a note to inspect it.</EmptyState>
              )}
              {selectedNotebookEntry ? (
                <>
                  <div
                    className="source-chip-list"
                    aria-label={`Tags for ${selectedNotebookEntry.title}`}
                  >
                    {selectedNotebookEntry.tags.map((tag) => (
                      <StatusPill key={tag}>{tag}</StatusPill>
                    ))}
                    {selectedNotebookEntry.tags.length === 0 ? (
                      <span className="membership-empty">No tags</span>
                    ) : null}
                  </div>
                  <dl className="metadata-grid notebook-entry-meta">
                    <div>
                      <dt>Status</dt>
                      <dd>{selectedNotebookEntry.claimStatus ?? "Not set"}</dd>
                    </div>
                    <div>
                      <dt>Event</dt>
                      <dd>{selectedNotebookEntry.eventDate ?? "Not set"}</dd>
                    </div>
                    <div>
                      <dt>Follow-up quarter</dt>
                      <dd>{selectedNotebookEntry.followUpAfter ?? "Not set"}</dd>
                    </div>
                    <div>
                      <dt>Follow-up date</dt>
                      <dd>{selectedNotebookEntry.followUpDate ?? "Not set"}</dd>
                    </div>
                    <div>
                      <dt>Origin</dt>
                      <dd>{renderNotebookOrigins(selectedNotebookEntry.origins, selectedNotebookEntry.companyId)}</dd>
                    </div>
                  </dl>
                </>
              ) : null}
            </form>
          </div>
          {notebookError ? (
            <p className="error-text">Notebook command failed: {notebookError}</p>
          ) : null}
        </div>
      ) : null}
    
      {companyWorkspaceTab === "Claims" ? (
        <div className="company-tab-panel claims-panel" aria-label="Company claims">
          <div className="notebook-toolbar">
            <div>
              <h3>Claims</h3>
              <p>
                {selectedCompanyClaimEntries.length} follow-up item
                {selectedCompanyClaimEntries.length === 1 ? "" : "s"} for{" "}
                {selectedCompany.qualifiedTicker}
              </p>
            </div>
          </div>
          <div className="claims-list">
            {selectedCompanyClaimEntries.map((entry) => (
              <div className="claim-row-block" key={entry.id}>
                <button
                  aria-label={`Open claim: ${entry.title}`}
                  className={[
                    "notebook-row",
                    selectedClaimEntry?.id === entry.id ? "notebook-row-selected" : "",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                  onClick={() => toggleClaimEntry(entry)}
                  type="button"
                >
                  <div>
                    <div className="notebook-row-top">
                      <h3>{entry.title}</h3>
                      <span>{entry.claimStatus?.replace("_", " ") ?? "open"}</span>
                    </div>
                  </div>
                  <div className="notebook-row-meta">
                    {entry.followUpAfter ? <span>{entry.followUpAfter}</span> : null}
                    {entry.followUpDate ? <span>{entry.followUpDate}</span> : null}
                    {entry.tags.slice(0, 2).map((tag) => (
                      <span key={tag}>{tag}</span>
                    ))}
                  </div>
                </button>
    
                {selectedClaimEntry?.id === entry.id ? (
                  <div className="claim-detail" aria-label="Claim detail">
                    <div className="notebook-entry-header">
                      <div>
                        <span className="eyebrow">
                          {entry.kind.replace("_", " ")}
                        </span>
                        <h3>{entry.title}</h3>
                      </div>
                      <div className="claim-status-control">
                        <label>
                          Status
                          <select
                            aria-label="Claim status"
                            value={claimStatusDraft}
                            onChange={(event) => setClaimStatusDraft(event.target.value)}
                          >
                            <option value="open">Open</option>
                            <option value="delivered">Delivered</option>
                            <option value="partially_delivered">Partially delivered</option>
                            <option value="missed">Missed</option>
                            <option value="unknown">Unknown</option>
                            <option value="not_applicable">Not applicable</option>
                          </select>
                        </label>
                        <Button
                          className="compact-button"
                          disabled={(entry.claimStatus ?? "open") === claimStatusDraft}
                          onClick={() => saveClaimStatus(entry)}
                          variant="primary"
                        >
                          <Save size={15} />
                          Save
                        </Button>
                      </div>
                    </div>
                    <MarkdownNoteBody body={entry.body} />
                    <dl className="metadata-grid notebook-entry-meta">
                      <div>
                        <dt>Event</dt>
                        <dd>{entry.eventDate ?? "Not set"}</dd>
                      </div>
                      <div>
                        <dt>Follow-up quarter</dt>
                        <dd>{entry.followUpAfter ?? "Not set"}</dd>
                      </div>
                      <div>
                        <dt>Follow-up date</dt>
                        <dd>{entry.followUpDate ?? "Not set"}</dd>
                      </div>
                      <div>
                        <dt>Origin</dt>
                        <dd>{renderNotebookOrigins(entry.origins, entry.companyId)}</dd>
                      </div>
                    </dl>
                  </div>
                ) : null}
              </div>
            ))}
            {selectedCompanyClaimEntries.length === 0 ? (
              <EmptyState>No claim notes for {selectedCompany.qualifiedTicker} yet.</EmptyState>
            ) : null}
          </div>
          {notebookError ? (
            <p className="error-text">Notebook command failed: {notebookError}</p>
          ) : null}
        </div>
      ) : null}
    
      {companyWorkspaceTab === "Transcripts" ? (
        <EmptyState className="company-tab-panel">
          YouTube transcript workflows start in Milestone 7.
        </EmptyState>
      ) : null}
    
      {companyWorkspaceTab === "Metadata" ? (
        <dl className="company-tab-panel metadata-grid" aria-label="Company metadata">
          <div>
            <dt>Qualified ticker</dt>
            <dd>{selectedCompany.qualifiedTicker}</dd>
          </div>
          <div>
            <dt>Exchange</dt>
            <dd>{selectedCompany.exchange}</dd>
          </div>
          <div>
            <dt>Ticker</dt>
            <dd>{selectedCompany.ticker}</dd>
          </div>
          <div>
            <dt>ISIN</dt>
            <dd>{selectedCompany.isin ?? "Not set"}</dd>
          </div>
          <div>
            <dt>CIK</dt>
            <dd>{selectedCompany.cik ?? "Not set"}</dd>
          </div>
          <div>
            <dt>LEI</dt>
            <dd>{selectedCompany.lei ?? "Not set"}</dd>
          </div>
        </dl>
      ) : null}
    </section>
  );
}
