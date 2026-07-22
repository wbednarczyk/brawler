import { useState } from "react";
import { ArrowLeft, BookOpenText, Maximize2, Plus, Save, X } from "lucide-react";
import type { Company, NotebookEntry } from "../../api/types";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import {
  ActionRow,
  Button,
  ChipList,
  EmptyState,
  ErrorText,
  FocusOverlay,
  InfoGrid,
  SectionHeader,
  SelectField,
  StatusPill,
  TextareaField,
  TextField,
} from "../../ui";
import type {
  MarkdownNoteBodyProps,
  NotebookDateLikeFieldProps,
  NotebookForm,
} from "../../shared/types/notebook";

// The company-scoped notebook composer + list/detail editor (with the
// distraction-free Focus writer). Extracted from the tabbed CompanyWorkspace so
// both the workspace and the cockpit `companyNotebook` dashboard panel (ADR 0057)
// render the same surface from explicit props.
export type CompanyNotebookSectionProps = {
  company: Company;
  notebookEntries: NotebookEntry[];
  isComposerOpen: boolean;
  notebookForm: NotebookForm;
  selectedNotebookEntry: NotebookEntry | null;
  notebookEditMode: boolean;
  notebookEditForm: NotebookForm;
  isNotebookEditDirty: boolean;
  notebookError: string | null;
  setComposerOpen: React.Dispatch<React.SetStateAction<boolean>>;
  updateNotebookForm: (field: keyof NotebookForm, value: string) => void;
  createNotebookEntry: (event: React.FormEvent<HTMLFormElement>) => void;
  setSelectedNotebookEntryId: React.Dispatch<React.SetStateAction<string | null>>;
  saveNotebookEntry: (event: React.FormEvent<HTMLFormElement>) => void;
  cancelNotebookEdit: () => void;
  setNotebookEditMode: (value: boolean) => void;
  updateNotebookEditForm: (field: keyof NotebookForm, value: string) => void;
  NotebookDateField: React.ComponentType<NotebookDateLikeFieldProps>;
  NotebookQuarterField: React.ComponentType<NotebookDateLikeFieldProps>;
  MarkdownNoteBody: React.ComponentType<MarkdownNoteBodyProps>;
  renderNotebookOrigins: (origins: NotebookEntry["origins"], companyId: string) => React.ReactNode;
};

export function CompanyNotebookSection({
  company,
  notebookEntries,
  isComposerOpen,
  notebookForm,
  selectedNotebookEntry,
  notebookEditMode,
  notebookEditForm,
  isNotebookEditDirty,
  notebookError,
  setComposerOpen,
  updateNotebookForm,
  createNotebookEntry,
  setSelectedNotebookEntryId,
  saveNotebookEntry,
  cancelNotebookEdit,
  setNotebookEditMode,
  updateNotebookEditForm,
  NotebookDateField,
  NotebookQuarterField,
  MarkdownNoteBody,
  renderNotebookOrigins,
}: CompanyNotebookSectionProps) {
  const { text } = useLocale();
  // Distraction-free full-screen note authoring (Focus writer, ADR 0054).
  const [writerOpen, setWriterOpen] = useState(false);

  return (
    <div className="company-tab-panel notebook-panel" aria-label={text("Company notebook")}>
      <SectionHeader
        level="h3"
        paneLead
        title={text("Notebook")}
        description={
          <>
            {notebookEntries.length} {text(notebookEntries.length === 1 ? "note" : "notes")} {text("for")}{" "}
            <TickerLabel value={company.qualifiedTicker} />
          </>
        }
        actions={
          <Button
            className="compact-button"
            // ADR 0081 Q4: the Notebook panel's contracted primary action —
            // capture a note. J2's former primary action (the KPI-extraction
            // launcher) went with the AI layer (ADR 0084), and note capture is
            // the journey's surviving single-primary decision surface.
            data-ux-primary-action="true"
            onClick={() => setComposerOpen((current) => !current)}
            variant="primary"
          >
            {isComposerOpen ? <X size={15} /> : <Plus size={15} />}
            {isComposerOpen ? text("Hide form") : text("New note")}
          </Button>
        }
      />

      {isComposerOpen ? (
        <form className="notebook-form" onSubmit={createNotebookEntry}>
          <div className="notebook-form-grid">
            <TextField
              label={text("Title")}
              aria-label={text("Notebook note title")}
              value={notebookForm.title}
              onChange={(event) => updateNotebookForm("title", event.target.value)}
            />
            <SelectField
              label={text("Kind")}
              aria-label={text("Notebook note kind")}
              value={notebookForm.kind}
              onChange={(event) => updateNotebookForm("kind", event.target.value)}
            >
              <option value="manual">{text("Manual")}</option>
              <option value="observation">{text("Observation")}</option>
              <option value="claim">{text("Claim")}</option>
              <option value="question">{text("Question")}</option>
              <option value="follow_up">{text("Follow-up")}</option>
            </SelectField>
            <TextField
              label={text("Tags")}
              aria-label={text("Notebook note tags")}
              placeholder={text("comma, separated")}
              value={notebookForm.tags}
              onChange={(event) => updateNotebookForm("tags", event.target.value)}
            />
            <SelectField
              label={text("Claim status")}
              aria-label={text("Notebook claim status")}
              value={notebookForm.claimStatus}
              onChange={(event) => updateNotebookForm("claimStatus", event.target.value)}
            >
              <option value="">{text("None")}</option>
              <option value="open">{text("Status open")}</option>
              <option value="delivered">{text("Delivered")}</option>
              <option value="partially_delivered">{text("Partially delivered")}</option>
              <option value="missed">{text("Missed")}</option>
              <option value="unknown">{text("Unknown")}</option>
              <option value="not_applicable">{text("Not applicable")}</option>
            </SelectField>
            <NotebookDateField
              ariaLabel={text("Notebook event date")}
              label={text("Event date")}
              value={notebookForm.eventDate}
              onChange={(value) => updateNotebookForm("eventDate", value)}
            />
            <NotebookQuarterField
              ariaLabel={text("Notebook follow-up quarter")}
              label={text("Follow-up quarter")}
              value={notebookForm.followUpAfter}
              onChange={(value) => updateNotebookForm("followUpAfter", value)}
            />
            <NotebookDateField
              ariaLabel={text("Notebook follow-up date")}
              label={text("Follow-up date")}
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
              {text("Save")}
            </Button>
          </div>
          <TextareaField
            className="notebook-body-field"
            label={text("Body")}
            aria-label={text("Notebook note body")}
            value={notebookForm.body}
            onChange={(event) => updateNotebookForm("body", event.target.value)}
          />
        </form>
      ) : null}

      <div className="notebook-workspace" {...(selectedNotebookEntry ? { "data-detail-open": "" } : {})}>
        <div className="notebook-list" aria-label={text("Notebook entries")}>
          {notebookEntries.map((entry) => (
            <button
              aria-label={`${text("Select notebook entry")}: ${entry.title}`}
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
          {notebookEntries.length === 0 ? (
            <EmptyState>{text("No notebook entries for")} <TickerLabel value={company.qualifiedTicker} /> {text("yet.")}</EmptyState>
          ) : null}
        </div>
        <form
          className="notebook-detail"
          aria-label={text("Notebook entry detail")}
          onSubmit={saveNotebookEntry}
        >
          {selectedNotebookEntry ? (
            <Button
              className="notebook-back-to-list"
              type="button"
              variant="minimal"
              onClick={() => setSelectedNotebookEntryId(null)}
              aria-label={text("Back to note list")}
            >
              <ArrowLeft size={15} />
              {text("Back")}
            </Button>
          ) : null}
          {selectedNotebookEntry ? (
            notebookEditMode ? (
              <>
                <div className="notebook-entry-header">
                  <TextField
                    label={text("Title")}
                    aria-label={text("Selected notebook title")}
                    value={notebookEditForm.title}
                    onChange={(event) => updateNotebookEditForm("title", event.target.value)}
                  />
                  <ActionRow className="notebook-detail-actions">
                    <Button
                      className="compact-button"
                      type="button"
                      onClick={() => setWriterOpen(true)}
                      title={text("Write full-screen, distraction-free")}
                    >
                      <Maximize2 size={15} />
                      {text("Focus")}
                    </Button>
                    <Button className="compact-button" onClick={cancelNotebookEdit}>
                      <X size={15} />
                      {text("Cancel")}
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
                      {text("Save")}
                    </Button>
                  </ActionRow>
                </div>
                <TextareaField
                  aria-label={text("Selected notebook body")}
                  value={notebookEditForm.body}
                  onChange={(event) => updateNotebookEditForm("body", event.target.value)}
                />
                <FocusOverlay
                  open={writerOpen}
                  onClose={() => setWriterOpen(false)}
                  eyebrow={<TickerLabel value={company.qualifiedTicker} />}
                  title={notebookEditForm.title.trim() ? notebookEditForm.title : text("Untitled note")}
                  ariaLabel={text("Focus writer")}
                  actions={
                    <Button
                      form="notebook-writer-form"
                      type="submit"
                      variant="primary"
                      disabled={
                        !isNotebookEditDirty ||
                        !notebookEditForm.title.trim() ||
                        !notebookEditForm.body.trim()
                      }
                    >
                      <Save size={15} />
                      {text("Save")}
                    </Button>
                  }
                >
                  <form
                    id="notebook-writer-form"
                    className="notebook-writer"
                    onSubmit={(event) => {
                      saveNotebookEntry(event);
                      setWriterOpen(false);
                    }}
                  >
                    <TextField
                      aria-label={text("Note title")}
                      value={notebookEditForm.title}
                      onChange={(event) => updateNotebookEditForm("title", event.target.value)}
                    />
                    <TextareaField
                      textareaClassName="notebook-writer-body"
                      aria-label={text("Note body")}
                      value={notebookEditForm.body}
                      onChange={(event) => updateNotebookEditForm("body", event.target.value)}
                    />
                  </form>
                </FocusOverlay>
                <div className="notebook-detail-grid">
                  <SelectField
                    label={text("Kind")}
                    aria-label={text("Selected notebook kind")}
                    value={notebookEditForm.kind}
                    onChange={(event) => updateNotebookEditForm("kind", event.target.value)}
                  >
                    <option value="manual">{text("Manual")}</option>
                    <option value="observation">{text("Observation")}</option>
                    <option value="claim">{text("Claim")}</option>
                    <option value="question">{text("Question")}</option>
                    <option value="follow_up">{text("Follow-up")}</option>
                  </SelectField>
                  <SelectField
                    label={text("Claim status")}
                    aria-label={text("Selected notebook claim status")}
                    value={notebookEditForm.claimStatus}
                    onChange={(event) => updateNotebookEditForm("claimStatus", event.target.value)}
                  >
                    <option value="">{text("None")}</option>
                    <option value="open">{text("Status open")}</option>
                    <option value="delivered">{text("Delivered")}</option>
                    <option value="partially_delivered">{text("Partially delivered")}</option>
                    <option value="missed">{text("Missed")}</option>
                    <option value="unknown">{text("Unknown")}</option>
                    <option value="not_applicable">{text("Not applicable")}</option>
                  </SelectField>
                  <TextField
                    label={text("Tags")}
                    aria-label={text("Selected notebook tags")}
                    value={notebookEditForm.tags}
                    onChange={(event) => updateNotebookEditForm("tags", event.target.value)}
                  />
                  <NotebookDateField
                    ariaLabel={text("Selected notebook event date")}
                    label={text("Event date")}
                    value={notebookEditForm.eventDate}
                    onChange={(value) => updateNotebookEditForm("eventDate", value)}
                  />
                  <NotebookQuarterField
                    ariaLabel={text("Selected notebook follow-up quarter")}
                    label={text("Follow-up quarter")}
                    value={notebookEditForm.followUpAfter}
                    onChange={(value) => updateNotebookEditForm("followUpAfter", value)}
                  />
                  <NotebookDateField
                    ariaLabel={text("Selected notebook follow-up date")}
                    label={text("Follow-up date")}
                    value={notebookEditForm.followUpDate}
                    onChange={(value) => updateNotebookEditForm("followUpDate", value)}
                  />
                </div>
              </>
            ) : (
              <>
                <div className="notebook-entry-header">
                  <div>
                    <span className="eyebrow">{selectedNotebookEntry.kind.replace("_", " ")}</span>
                    <h3>{selectedNotebookEntry.title}</h3>
                  </div>
                  <Button className="compact-button" onClick={() => setNotebookEditMode(true)}>
                    <BookOpenText size={15} />
                    {text("Edit")}
                  </Button>
                </div>
                <MarkdownNoteBody
                  ariaLabel={text("Selected notebook body")}
                  body={selectedNotebookEntry.body}
                />
              </>
            )
          ) : (
            <EmptyState>{text("Select a note to inspect it.")}</EmptyState>
          )}
          {selectedNotebookEntry ? (
            <>
              <ChipList ariaLabel={`${text("Tags for")} ${selectedNotebookEntry.title}`}>
                {selectedNotebookEntry.tags.map((tag) => (
                  <StatusPill key={tag}>{tag}</StatusPill>
                ))}
                {selectedNotebookEntry.tags.length === 0 ? (
                  <span className="membership-empty">{text("No tags")}</span>
                ) : null}
              </ChipList>
              <InfoGrid
                className="metadata-grid notebook-entry-meta"
                items={[
                  { label: text("Status"), value: selectedNotebookEntry.claimStatus ?? text("Not set") },
                  { label: text("Event"), value: selectedNotebookEntry.eventDate ?? text("Not set") },
                  { label: text("Follow-up quarter"), value: selectedNotebookEntry.followUpAfter ?? text("Not set") },
                  { label: text("Follow-up date"), value: selectedNotebookEntry.followUpDate ?? text("Not set") },
                  {
                    label: text("Origin"),
                    value: renderNotebookOrigins(selectedNotebookEntry.origins, selectedNotebookEntry.companyId),
                  },
                ]}
              />
            </>
          ) : null}
        </form>
      </div>
      {notebookError ? (
        <ErrorText>{text("Notebook command failed")}: {notebookError}</ErrorText>
      ) : null}
    </div>
  );
}
