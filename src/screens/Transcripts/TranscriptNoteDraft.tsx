import type { TranscriptJob } from "../../api/types";
import { ActionButton, ActionRow, SectionHeader, SelectField, TextField, TextareaField } from "../../ui";
import { useLocale } from "../../shared/locale";
import type { TranscriptPrimary } from "./transcriptPrimary";
import type { TranscriptsScreenProps } from "./transcriptTypes";

type TranscriptNoteDraftProps = Pick<
  TranscriptsScreenProps,
  | "transcriptNoteForm"
  | "transcriptNoteSaveInFlight"
  | "NotebookDateField"
  | "NotebookQuarterField"
  | "createTranscriptNotebookEntry"
  | "updateTranscriptNoteForm"
> & {
  job: TranscriptJob;
  discardTranscriptNoteDraft: () => void;
  primary: TranscriptPrimary;
};

export function TranscriptNoteDraft({
  job,
  transcriptNoteForm,
  transcriptNoteSaveInFlight,
  NotebookDateField,
  NotebookQuarterField,
  createTranscriptNotebookEntry,
  discardTranscriptNoteDraft,
  updateTranscriptNoteForm,
  primary,
}: TranscriptNoteDraftProps) {
  const { text } = useLocale();

  return (
    <form
      className="transcript-note-draft"
      onSubmit={(event) => createTranscriptNotebookEntry(job, event)}
      aria-label={text("Notebook note draft")}
    >
      <SectionHeader
        title={text("Notebook note draft")}
        description={text("Edit the note before saving it to the company notebook.")}
        actions={
          <ActionButton kind="control" onClick={discardTranscriptNoteDraft} type="button" variant="ghost">
            {text("Discard")}
          </ActionButton>
        }
      />
      <div className="transcript-note-draft-grid">
        <TextField
          label={text("Title")}
          aria-label={text("Transcript note title")}
          value={transcriptNoteForm.title}
          onChange={(event) => updateTranscriptNoteForm("title", event.target.value)}
        />
        <SelectField
          label={text("Kind")}
          aria-label={text("Transcript note kind")}
          value={transcriptNoteForm.kind}
          onChange={(event) => updateTranscriptNoteForm("kind", event.target.value)}
        >
          <option value="manual">{text("Manual")}</option>
          <option value="observation">{text("Observation")}</option>
          <option value="claim">{text("Claim")}</option>
          <option value="question">{text("Question")}</option>
          <option value="follow_up">{text("Follow-up")}</option>
        </SelectField>
        <SelectField
          label={text("Status")}
          aria-label={text("Transcript note status")}
          value={transcriptNoteForm.claimStatus}
          onChange={(event) => updateTranscriptNoteForm("claimStatus", event.target.value)}
        >
          <option value="">{text("Not set")}</option>
          <option value="open">{text("Status open")}</option>
          <option value="delivered">{text("Delivered")}</option>
          <option value="partially_delivered">{text("Partially delivered")}</option>
          <option value="missed">{text("Missed")}</option>
          <option value="unknown">{text("Unknown")}</option>
          <option value="not_applicable">{text("Not applicable")}</option>
        </SelectField>
        <TextField
          label={text("Tags")}
          aria-label={text("Transcript note tags")}
          value={transcriptNoteForm.tags}
          onChange={(event) => updateTranscriptNoteForm("tags", event.target.value)}
        />
        <NotebookDateField
          ariaLabel={text("Transcript note event date")}
          label={text("Event date")}
          value={transcriptNoteForm.eventDate}
          onChange={(value) => updateTranscriptNoteForm("eventDate", value)}
        />
        <NotebookQuarterField
          ariaLabel={text("Transcript note follow-up quarter")}
          label={text("Follow-up quarter")}
          value={transcriptNoteForm.followUpAfter}
          onChange={(value) => updateTranscriptNoteForm("followUpAfter", value)}
        />
        <NotebookDateField
          ariaLabel={text("Transcript note follow-up date")}
          label={text("Follow-up date")}
          value={transcriptNoteForm.followUpDate}
          onChange={(value) => updateTranscriptNoteForm("followUpDate", value)}
        />
      </div>
      <TextareaField
        className="notebook-body-field"
        label={text("Body")}
        aria-label={text("Transcript note body")}
        value={transcriptNoteForm.body}
        onChange={(event) => updateTranscriptNoteForm("body", event.target.value)}
      />
      <ActionRow className="transcript-note-draft-actions">
        <ActionButton
          data-ux-primary-action={primary === "saveNote" ? "true" : undefined}
          disabled={transcriptNoteSaveInFlight === job.id}
          type="submit"
          variant={primary === "saveNote" ? "primary" : "secondary"}
          verb="save"
        >
          {transcriptNoteSaveInFlight === job.id ? text("Saving") : text("Save note")}
        </ActionButton>
      </ActionRow>
    </form>
  );
}
