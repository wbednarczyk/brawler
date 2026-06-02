import { BookOpenText, Building2, ExternalLink, FileText, Mail, MailOpen, Save } from "lucide-react";
import { Button } from "../../shared/components/Button";
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
  | "feedItemSummary"
  | "formatTimestamp"
>;

export function InboxDetailPane({
  selectedFeedItem,
  selectedFeedCompany,
  healthError,
  databaseError,
  updateSelectedFeedItem,
  openCompanyWorkspaceFromFeedItem,
  openFeedItemNoteDraft,
  feedItemSummary,
  formatTimestamp,
}: InboxDetailPaneProps) {
  return (
    <aside className="detail-pane" aria-label="Feed item details">
      <div className="detail-icon">
        <FileText size={24} />
      </div>
      {selectedFeedItem ? (
        <>
          <h2>{selectedFeedItem.title}</h2>
          <section className="feed-body-section" aria-label="Feed summary">
            <div className="feed-body-heading">
              <span>Summary</span>
            </div>
            <p className="feed-detail-body">{feedItemSummary(selectedFeedItem)}</p>
          </section>
          <details className="feed-body-section feed-body-disclosure" aria-label="Official report body">
            <summary className="feed-body-heading">
              <span>Official report body</span>
              <strong>{selectedFeedItem.bodyText ? "Stored" : "Not stored"}</strong>
            </summary>
            {selectedFeedItem.bodyText ? (
              <p className="feed-detail-body">{selectedFeedItem.bodyText}</p>
            ) : (
              <p className="feed-detail-empty">
                No official report body is stored for this item yet. Refresh sources and check Sources for detail
                warnings if this remains empty.
              </p>
            )}
          </details>
          <div className="detail-actions" aria-label="Feed item actions">
            <Button
              className="compact-button"
              onClick={() =>
                updateSelectedFeedItem((item) => ({
                  ...item,
                  unread: !item.unread,
                }))
              }
            >
              {selectedFeedItem.unread ? <MailOpen size={15} /> : <Mail size={15} />}
              {selectedFeedItem.unread ? "Mark read" : "Mark unread"}
            </Button>
            <Button
              className="compact-button"
              onClick={() =>
                updateSelectedFeedItem((item) => ({
                  ...item,
                  saved: !item.saved,
                }))
              }
            >
              <Save size={15} />
              {selectedFeedItem.saved ? "Unsave" : "Save"}
            </Button>
            {selectedFeedCompany ? (
              <Button
                className="compact-button"
                onClick={() => openCompanyWorkspaceFromFeedItem(selectedFeedItem)}
              >
                <Building2 size={15} />
                Open company
              </Button>
            ) : null}
            {selectedFeedCompany ? (
              <Button
                className="compact-button"
                onClick={() => openFeedItemNoteDraft(selectedFeedItem)}
              >
                <BookOpenText size={15} />
                Note
              </Button>
            ) : null}
            <a className="secondary-button compact-button" href={selectedFeedItem.sourceUrl} rel="noreferrer" target="_blank">
              <ExternalLink size={15} />
              Open source
            </a>
          </div>
          <dl>
            <div>
              <dt>Company</dt>
              <dd>{selectedFeedItem.company}</dd>
            </div>
            <div>
              <dt>Source</dt>
              <dd>{selectedFeedItem.source}</dd>
            </div>
            <div>
              <dt>Source URL</dt>
              <dd>
                <a href={selectedFeedItem.sourceUrl} rel="noreferrer" target="_blank">
                  {selectedFeedItem.sourceUrl}
                </a>
              </dd>
            </div>
            <div>
              <dt>Published</dt>
              <dd>{formatTimestamp(selectedFeedItem.publishedAt, "Unknown")}</dd>
            </div>
            <div>
              <dt>Fetched</dt>
              <dd>{formatTimestamp(selectedFeedItem.fetchedAt, "Unknown")}</dd>
            </div>
            <div>
              <dt>Attribution</dt>
              <dd>{selectedFeedItem.attribution}</dd>
            </div>
          </dl>
          {selectedFeedItem.attachments.length > 0 ? (
            <div className="feed-attachment-list" aria-label="Feed attachments">
              {selectedFeedItem.attachments.map((attachment) => (
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
        </>
      ) : (
        <>
          <h2>No item selected</h2>
          <p>Select a feed item to inspect source details and origin links.</p>
        </>
      )}
      {healthError ? <p className="error-text">Health command failed: {healthError}</p> : null}
      {databaseError ? <p className="error-text">Database command failed: {databaseError}</p> : null}
    </aside>
  );
}
