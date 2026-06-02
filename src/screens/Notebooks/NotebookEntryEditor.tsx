import { BookOpenText, Save, X } from "lucide-react";
import type { NotebookEntry } from "../../api/types";
import { Button } from "../../shared/components/Button";
import { StatusPill } from "../../shared/components/StatusPill";
import { useLocale } from "../../shared/locale";
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
  const { text } = useLocale();

  return (
    <form
      className="notebook-detail notebooks-inline-detail"
      aria-label={text("Notebook screen entry detail")}
      onSubmit={saveNotebookScreenEntry}
    >
      {isNotebookScreenEditMode ? (
        <>
          <div className="notebook-entry-header">
            <label>
              {text("Title")}
              <input
                aria-label={text("Notebook screen selected title")}
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
                {text("Cancel")}
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
                {text("Save")}
              </Button>
            </div>
          </div>
          <textarea
            aria-label={text("Notebook screen selected body")}
            value={notebookScreenEditForm.body}
            onChange={(event) =>
              updateNotebookScreenEditForm("body", event.target.value)
            }
          />
          <div className="notebook-detail-grid">
            <label>
              {text("Kind")}
              <select
                aria-label={text("Notebook screen selected kind")}
                value={notebookScreenEditForm.kind}
                onChange={(event) =>
                  updateNotebookScreenEditForm("kind", event.target.value)
                }
              >
                <option value="manual">{text("Manual")}</option>
                <option value="observation">{text("Observation")}</option>
                <option value="claim">{text("Claim")}</option>
                <option value="question">{text("Question")}</option>
                <option value="follow_up">{text("Follow-up")}</option>
              </select>
            </label>
            <label>
              {text("Claim status")}
              <select
                aria-label={text("Notebook screen selected claim status")}
                value={notebookScreenEditForm.claimStatus}
                onChange={(event) =>
                  updateNotebookScreenEditForm("claimStatus", event.target.value)
                }
              >
                <option value="">{text("None")}</option>
                <option value="open">{text("Status open")}</option>
                <option value="delivered">{text("Delivered")}</option>
                <option value="partially_delivered">{text("Partially delivered")}</option>
                <option value="missed">{text("Missed")}</option>
                <option value="unknown">{text("Unknown")}</option>
                <option value="not_applicable">{text("Not applicable")}</option>
              </select>
            </label>
            <label>
              {text("Tags")}
              <input
                aria-label={text("Notebook screen selected tags")}
                value={notebookScreenEditForm.tags}
                onChange={(event) =>
                  updateNotebookScreenEditForm("tags", event.target.value)
                }
              />
            </label>
            <NotebookDateField
              ariaLabel={text("Notebook screen selected event date")}
              label={text("Event date")}
              value={notebookScreenEditForm.eventDate}
              onChange={(value) => updateNotebookScreenEditForm("eventDate", value)}
            />
            <NotebookQuarterField
              ariaLabel={text("Notebook screen selected follow-up quarter")}
              label={text("Follow-up quarter")}
              value={notebookScreenEditForm.followUpAfter}
              onChange={(value) => updateNotebookScreenEditForm("followUpAfter", value)}
            />
            <NotebookDateField
              ariaLabel={text("Notebook screen selected follow-up date")}
              label={text("Follow-up date")}
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
                {text(selectedNotebookScreenEntry.kind.replace("_", " "))}
              </span>
              <h3>{selectedNotebookScreenEntry.title}</h3>
            </div>
            <Button
              className="compact-button"
              onClick={() => setNotebookScreenEditMode(true)}
            >
              <BookOpenText size={15} />
              {text("Edit")}
            </Button>
          </div>
          <MarkdownNoteBody
            ariaLabel={text("Notebook screen selected body")}
            body={selectedNotebookScreenEntry.body}
          />
        </>
      )}
      <div
        className="source-chip-list"
        aria-label={`${text("Tags for")} ${selectedNotebookScreenEntry.title}`}
      >
        {selectedNotebookScreenEntry.tags.map((tag) => (
          <StatusPill key={tag}>{tag}</StatusPill>
        ))}
        {selectedNotebookScreenEntry.tags.length === 0 ? (
          <span className="membership-empty">{text("No tags")}</span>
        ) : null}
      </div>
      <dl className="metadata-grid notebook-entry-meta">
        <div>
          <dt>{text("Status")}</dt>
          <dd>{selectedNotebookScreenEntry.claimStatus ?? text("Not set")}</dd>
        </div>
        <div>
          <dt>{text("Follow-up quarter")}</dt>
          <dd>{selectedNotebookScreenEntry.followUpAfter ?? text("Not set")}</dd>
        </div>
        <div>
          <dt>{text("Follow-up date")}</dt>
          <dd>{selectedNotebookScreenEntry.followUpDate ?? text("Not set")}</dd>
        </div>
        <div>
          <dt>{text("Origin")}</dt>
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
