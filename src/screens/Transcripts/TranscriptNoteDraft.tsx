import { Save, X } from "lucide-react";
import type { TranscriptJob } from "../../api/types";
import { Button } from "../../ui";
import { useLocale } from "../../shared/locale";
import type { TranscriptsScreenProps } from "./transcriptTypes";

type TranscriptNoteDraftProps = Pick<
  TranscriptsScreenProps,
  | "transcriptNoteForm"
  | "transcriptNoteSaveInFlight"
  | "NotebookDateField"
  | "NotebookQuarterField"
  | "createTranscriptNotebookEntry"
  | "discardTranscriptNoteDraft"
  | "updateTranscriptNoteForm"
> & {
  job: TranscriptJob;
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
}: TranscriptNoteDraftProps) {
  const { text } = useLocale();

  return (
    <form
      className="transcript-note-draft"
      onSubmit={(event) => createTranscriptNotebookEntry(job, event)}
      aria-label={text("Transcript note draft")}
    >
      <div className="event-composer-header">
        <div>
          <h2>{text("Notebook note draft")}</h2>
          <p>{text("Edit the note before saving it to the company notebook.")}</p>
        </div>
        <Button className="compact-button" onClick={discardTranscriptNoteDraft}>
          <X size={15} />
          {text("Discard")}
        </Button>
      </div>
      <div className="event-composer-grid">
        <label className="event-composer-title">
          {text("Title")}
          <input
            aria-label={text("Transcript note title")}
            value={transcriptNoteForm.title}
            onChange={(event) => updateTranscriptNoteForm("title", event.target.value)}
          />
        </label>
        <label>
          {text("Kind")}
          <select
            aria-label={text("Transcript note kind")}
            value={transcriptNoteForm.kind}
            onChange={(event) => updateTranscriptNoteForm("kind", event.target.value)}
          >
            <option value="manual">{text("Manual")}</option>
            <option value="observation">{text("Observation")}</option>
            <option value="claim">{text("Claim")}</option>
            <option value="question">{text("Question")}</option>
            <option value="follow_up">{text("Follow-up")}</option>
          </select>
        </label>
        <label>
          {text("Status")}
          <select
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
          </select>
        </label>
        <label>
          {text("Tags")}
          <input
            aria-label={text("Transcript note tags")}
            value={transcriptNoteForm.tags}
            onChange={(event) => updateTranscriptNoteForm("tags", event.target.value)}
          />
        </label>
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
      <label className="notebook-body-field">
        {text("Body")}
        <textarea
          aria-label={text("Transcript note body")}
          value={transcriptNoteForm.body}
          onChange={(event) => updateTranscriptNoteForm("body", event.target.value)}
        />
      </label>
      <div className="event-composer-actions">
        <Button className="compact-button" disabled={transcriptNoteSaveInFlight === job.id} type="submit" variant="primary">
          <Save size={15} />
          {transcriptNoteSaveInFlight === job.id ? text("Saving") : text("Save")}
        </Button>
      </div>
    </form>
  );
}
