import { Save, X } from "lucide-react";
import type { TranscriptJob } from "../../api/types";
import { Button } from "../../shared/components/Button";
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
  return (
    <form
      className="transcript-note-draft"
      onSubmit={(event) => createTranscriptNotebookEntry(job, event)}
      aria-label="Transcript note draft"
    >
      <div className="event-composer-header">
        <div>
          <h2>Notebook note draft</h2>
          <p>Edit the note before saving it to the company notebook.</p>
        </div>
        <Button className="compact-button" onClick={discardTranscriptNoteDraft}>
          <X size={15} />
          Discard
        </Button>
      </div>
      <div className="event-composer-grid">
        <label className="event-composer-title">
          Title
          <input
            aria-label="Transcript note title"
            value={transcriptNoteForm.title}
            onChange={(event) => updateTranscriptNoteForm("title", event.target.value)}
          />
        </label>
        <label>
          Kind
          <select
            aria-label="Transcript note kind"
            value={transcriptNoteForm.kind}
            onChange={(event) => updateTranscriptNoteForm("kind", event.target.value)}
          >
            <option value="manual">Manual</option>
            <option value="observation">Observation</option>
            <option value="claim">Claim</option>
            <option value="question">Question</option>
            <option value="follow_up">Follow-up</option>
          </select>
        </label>
        <label>
          Status
          <select
            aria-label="Transcript note status"
            value={transcriptNoteForm.claimStatus}
            onChange={(event) => updateTranscriptNoteForm("claimStatus", event.target.value)}
          >
            <option value="">Not set</option>
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
            aria-label="Transcript note tags"
            value={transcriptNoteForm.tags}
            onChange={(event) => updateTranscriptNoteForm("tags", event.target.value)}
          />
        </label>
        <NotebookDateField
          ariaLabel="Transcript note event date"
          label="Event date"
          value={transcriptNoteForm.eventDate}
          onChange={(value) => updateTranscriptNoteForm("eventDate", value)}
        />
        <NotebookQuarterField
          ariaLabel="Transcript note follow-up quarter"
          label="Follow-up quarter"
          value={transcriptNoteForm.followUpAfter}
          onChange={(value) => updateTranscriptNoteForm("followUpAfter", value)}
        />
        <NotebookDateField
          ariaLabel="Transcript note follow-up date"
          label="Follow-up date"
          value={transcriptNoteForm.followUpDate}
          onChange={(value) => updateTranscriptNoteForm("followUpDate", value)}
        />
      </div>
      <label className="notebook-body-field">
        Body
        <textarea
          aria-label="Transcript note body"
          value={transcriptNoteForm.body}
          onChange={(event) => updateTranscriptNoteForm("body", event.target.value)}
        />
      </label>
      <div className="event-composer-actions">
        <Button className="compact-button" disabled={transcriptNoteSaveInFlight === job.id} type="submit" variant="primary">
          <Save size={15} />
          {transcriptNoteSaveInFlight === job.id ? "Saving" : "Save"}
        </Button>
      </div>
    </form>
  );
}
