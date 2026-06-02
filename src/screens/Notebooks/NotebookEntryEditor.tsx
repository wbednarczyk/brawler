import { BookOpenText, Save, X } from "lucide-react";
import type { NotebookEntry } from "../../api/types";
import { Button } from "../../shared/components/Button";
import { StatusPill } from "../../shared/components/StatusPill";
import type { NotebooksScreenProps } from "./notebookTypes";

type NotebookEntryEditorProps = Pick<
  NotebooksScreenProps,
  | "isNotebookScreenEditMode"
  | "isNotebookScreenEditDirty"
  | "notebookScreenEditForm"
  | "saveNotebookScreenEntry"
  | "cancelNotebookScreenEdit"
  | "setNotebookScreenEditMode"
  | "updateNotebookScreenEditForm"
  | "NotebookDateField"
  | "NotebookQuarterField"
  | "MarkdownNoteBody"
  | "renderNotebookOrigins"
> & {
  selectedNotebookScreenEntry: NotebookEntry;
};

export function NotebookEntryEditor({
  selectedNotebookScreenEntry,
  isNotebookScreenEditMode,
  isNotebookScreenEditDirty,
  notebookScreenEditForm,
  saveNotebookScreenEntry,
  cancelNotebookScreenEdit,
  setNotebookScreenEditMode,
  updateNotebookScreenEditForm,
  NotebookDateField,
  NotebookQuarterField,
  MarkdownNoteBody,
  renderNotebookOrigins,
}: NotebookEntryEditorProps) {
  return (
    <form
      className="notebook-detail notebooks-inline-detail"
      aria-label="Notebook screen entry detail"
      onSubmit={saveNotebookScreenEntry}
    >
      {isNotebookScreenEditMode ? (
        <>
          <div className="notebook-entry-header">
            <label>
              Title
              <input
                aria-label="Notebook screen selected title"
                value={notebookScreenEditForm.title}
                onChange={(event) =>
                  updateNotebookScreenEditForm("title", event.target.value)
                }
              />
            </label>
            <div className="notebook-detail-actions">
              <Button
                className="compact-button"
                onClick={cancelNotebookScreenEdit}
              >
                <X size={15} />
                Cancel
              </Button>
              <Button
                className="compact-button"
                disabled={
                  !isNotebookScreenEditDirty ||
                  !notebookScreenEditForm.title.trim() ||
                  !notebookScreenEditForm.body.trim()
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
            aria-label="Notebook screen selected body"
            value={notebookScreenEditForm.body}
            onChange={(event) =>
              updateNotebookScreenEditForm("body", event.target.value)
            }
          />
          <div className="notebook-detail-grid">
            <label>
              Kind
              <select
                aria-label="Notebook screen selected kind"
                value={notebookScreenEditForm.kind}
                onChange={(event) =>
                  updateNotebookScreenEditForm("kind", event.target.value)
                }
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
                aria-label="Notebook screen selected claim status"
                value={notebookScreenEditForm.claimStatus}
                onChange={(event) =>
                  updateNotebookScreenEditForm("claimStatus", event.target.value)
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
                aria-label="Notebook screen selected tags"
                value={notebookScreenEditForm.tags}
                onChange={(event) =>
                  updateNotebookScreenEditForm("tags", event.target.value)
                }
              />
            </label>
            <NotebookDateField
              ariaLabel="Notebook screen selected event date"
              label="Event date"
              value={notebookScreenEditForm.eventDate}
              onChange={(value) => updateNotebookScreenEditForm("eventDate", value)}
            />
            <NotebookQuarterField
              ariaLabel="Notebook screen selected follow-up quarter"
              label="Follow-up quarter"
              value={notebookScreenEditForm.followUpAfter}
              onChange={(value) => updateNotebookScreenEditForm("followUpAfter", value)}
            />
            <NotebookDateField
              ariaLabel="Notebook screen selected follow-up date"
              label="Follow-up date"
              value={notebookScreenEditForm.followUpDate}
              onChange={(value) => updateNotebookScreenEditForm("followUpDate", value)}
            />
          </div>
        </>
      ) : (
        <>
          <div className="notebook-entry-header">
            <div>
              <span className="eyebrow">
                {selectedNotebookScreenEntry.kind.replace("_", " ")}
              </span>
              <h3>{selectedNotebookScreenEntry.title}</h3>
            </div>
            <Button
              className="compact-button"
              onClick={() => setNotebookScreenEditMode(true)}
            >
              <BookOpenText size={15} />
              Edit
            </Button>
          </div>
          <MarkdownNoteBody
            ariaLabel="Notebook screen selected body"
            body={selectedNotebookScreenEntry.body}
          />
        </>
      )}
      <div
        className="source-chip-list"
        aria-label={`Tags for ${selectedNotebookScreenEntry.title}`}
      >
        {selectedNotebookScreenEntry.tags.map((tag) => (
          <StatusPill key={tag}>{tag}</StatusPill>
        ))}
        {selectedNotebookScreenEntry.tags.length === 0 ? (
          <span className="membership-empty">No tags</span>
        ) : null}
      </div>
      <dl className="metadata-grid notebook-entry-meta">
        <div>
          <dt>Status</dt>
          <dd>{selectedNotebookScreenEntry.claimStatus ?? "Not set"}</dd>
        </div>
        <div>
          <dt>Follow-up quarter</dt>
          <dd>{selectedNotebookScreenEntry.followUpAfter ?? "Not set"}</dd>
        </div>
        <div>
          <dt>Follow-up date</dt>
          <dd>{selectedNotebookScreenEntry.followUpDate ?? "Not set"}</dd>
        </div>
        <div>
          <dt>Origin</dt>
          <dd>
            {renderNotebookOrigins(
              selectedNotebookScreenEntry.origins,
              selectedNotebookScreenEntry.companyId,
            )}
          </dd>
        </div>
      </dl>
    </form>
  );
}
