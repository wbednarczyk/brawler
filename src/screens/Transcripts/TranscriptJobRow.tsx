import { Plus, RefreshCw, Save, Trash2 } from "lucide-react";
import type { TranscriptJob } from "../../api/types";
import { Button } from "../../shared/components/Button";
import { StatusPill } from "../../shared/components/StatusPill";
import { formatTranscriptStatus } from "./transcriptHelpers";
import { TranscriptNoteDraft } from "./TranscriptNoteDraft";
import { TranscriptSegmentReview } from "./TranscriptSegmentReview";
import type { TranscriptsScreenProps } from "./transcriptTypes";

type TranscriptJobRowProps = Pick<
  TranscriptsScreenProps,
  | "companies"
  | "settings"
  | "geminiCredentialStatus"
  | "transcriptJobRunInFlight"
  | "selectedTranscriptJobId"
  | "transcriptSegmentsByJobId"
  | "transcriptSegmentsErrorByJobId"
  | "transcriptSegmentSearchByJobId"
  | "selectedTranscriptSegmentIdsByJobId"
  | "transcriptNoteDraftJobId"
  | "transcriptNoteForm"
  | "transcriptNoteErrorByJobId"
  | "transcriptNoteSaveInFlight"
  | "transcriptLinkQueryByJobId"
  | "transcriptLinkErrorByJobId"
  | "transcriptLinkInFlight"
  | "transcriptDeleteInFlight"
  | "transcriptDescriptionDraftByJobId"
  | "transcriptDescriptionErrorByJobId"
  | "transcriptDescriptionSaveInFlight"
  | "NotebookDateField"
  | "NotebookQuarterField"
  | "setTranscriptSegmentSearchByJobId"
  | "setTranscriptDescriptionDraftByJobId"
  | "toggleTranscriptJob"
  | "toggleTranscriptJobFromKeyboard"
  | "runTranscriptJob"
  | "deleteTranscriptJob"
  | "updateTranscriptJobDescription"
  | "updateTranscriptLinkQuery"
  | "linkTranscriptJobCompany"
  | "toggleTranscriptSegment"
  | "openTranscriptNoteDraft"
  | "createTranscriptNotebookEntry"
  | "discardTranscriptNoteDraft"
  | "updateTranscriptNoteForm"
  | "formatAiProvider"
  | "formatGeminiModel"
  | "formatEnumLabel"
> & {
  job: TranscriptJob;
};

export function TranscriptJobRow({
  job,
  companies,
  settings,
  geminiCredentialStatus,
  transcriptJobRunInFlight,
  selectedTranscriptJobId,
  transcriptSegmentsByJobId,
  transcriptSegmentsErrorByJobId,
  transcriptSegmentSearchByJobId,
  selectedTranscriptSegmentIdsByJobId,
  transcriptNoteDraftJobId,
  transcriptNoteForm,
  transcriptNoteErrorByJobId,
  transcriptNoteSaveInFlight,
  transcriptLinkQueryByJobId,
  transcriptLinkErrorByJobId,
  transcriptLinkInFlight,
  transcriptDeleteInFlight,
  transcriptDescriptionDraftByJobId,
  transcriptDescriptionErrorByJobId,
  transcriptDescriptionSaveInFlight,
  NotebookDateField,
  NotebookQuarterField,
  setTranscriptSegmentSearchByJobId,
  setTranscriptDescriptionDraftByJobId,
  toggleTranscriptJob,
  toggleTranscriptJobFromKeyboard,
  runTranscriptJob,
  deleteTranscriptJob,
  updateTranscriptJobDescription,
  updateTranscriptLinkQuery,
  linkTranscriptJobCompany,
  toggleTranscriptSegment,
  openTranscriptNoteDraft,
  createTranscriptNotebookEntry,
  discardTranscriptNoteDraft,
  updateTranscriptNoteForm,
  formatAiProvider,
  formatGeminiModel,
  formatEnumLabel,
}: TranscriptJobRowProps) {
  const transcriptSegments = transcriptSegmentsByJobId[job.id] ?? [];
  const transcriptSegmentSearch = transcriptSegmentSearchByJobId[job.id] ?? "";
  const selectedTranscriptSegmentIds = selectedTranscriptSegmentIdsByJobId[job.id] ?? [];
  const selectedTranscriptSegments = transcriptSegments.filter((segment) =>
    selectedTranscriptSegmentIds.includes(segment.id),
  );
  const transcriptSegmentsError = transcriptSegmentsErrorByJobId[job.id];
  const transcriptNoteError = transcriptNoteErrorByJobId[job.id];
  const transcriptLinkQuery = transcriptLinkQueryByJobId[job.id] ?? "";
  const transcriptLinkError = transcriptLinkErrorByJobId[job.id];
  const transcriptLinkSuggestions = transcriptLinkQuery.trim()
    ? companies
        .filter((company) => {
          const query = transcriptLinkQuery.trim().toLowerCase();
  
          return (
            company.qualifiedTicker.toLowerCase().includes(query) ||
            company.ticker.toLowerCase().includes(query) ||
            company.displayName.toLowerCase().includes(query) ||
            (company.isin?.toLowerCase().includes(query) ?? false)
          );
        })
        .slice(0, 6)
    : [];
  const isTranscriptJobSelected = selectedTranscriptJobId === job.id;
  const isTranscriptNoteDraftOpen = transcriptNoteDraftJobId === job.id;
  const transcriptDescriptionDraft = transcriptDescriptionDraftByJobId[job.id] ?? job.sourceLabel ?? "";
  const transcriptDescriptionError = transcriptDescriptionErrorByJobId[job.id];
  const isTranscriptDescriptionDirty = transcriptDescriptionDraft !== (job.sourceLabel ?? "");
  
  return (
    <div className="source-row-block" key={job.id}>
      <article
        className={[
          "source-row",
          "transcript-row",
          isTranscriptJobSelected ? "source-row-selected" : "",
        ]
          .filter(Boolean)
          .join(" ")}
        aria-label={`Open transcript job: ${job.sourceUrl}`}
        onClick={() => toggleTranscriptJob(job)}
        onKeyDown={(event) => toggleTranscriptJobFromKeyboard(event, job)}
        role="button"
        tabIndex={0}
      >
        <div className="source-row-main">
          <div className="source-title-line">
            <span
              className={
                job.status === "failed"
                  ? "status-dot status-danger"
                  : job.status === "completed"
                    ? "status-dot status-ok"
                    : "status-dot status-warn"
              }
              title={formatTranscriptStatus(job.status)}
            />
            <h2>{job.sourceLabel ?? "Untitled transcript"}</h2>
          </div>
          <p>
            {job.sourceUrl} · {job.company ?? "Unlinked transcript"} · {formatAiProvider(job.providerId)} ·{" "}
            {formatGeminiModel(settings?.aiProviders.youtubeTranscriptionModel)}
          </p>
          <div className="source-chip-list" aria-label={`Transcript job metadata for ${job.id}`}>
            <StatusPill>{formatTranscriptStatus(job.companyResolutionStatus)}</StatusPill>
            <StatusPill>{job.sourceType}</StatusPill>
            {job.errorCode ? <StatusPill>{formatEnumLabel(job.errorCode)}</StatusPill> : null}
          </div>
        </div>
        <div className="source-row-status">
          <span>{formatTranscriptStatus(job.status)}</span>
          {job.status === "queued" || job.status === "failed" ? (
            <Button
              className="compact-button"
              disabled={transcriptJobRunInFlight === job.id || !geminiCredentialStatus?.configured}
              onClick={(event) => {
                event.stopPropagation();
                runTranscriptJob(job.id);
              }}
              title={
                geminiCredentialStatus?.configured
                  ? "Retry Gemini transcription"
                  : "Configure Gemini API key in Settings before running transcription"
              }
            >
              <RefreshCw size={15} />
              {transcriptJobRunInFlight === job.id ? "Running" : "Retry"}
            </Button>
          ) : null}
          <Button
            disabled={transcriptDeleteInFlight === job.id}
            onClick={(event) => {
              event.stopPropagation();
              deleteTranscriptJob(job);
            }}
            title={`Delete transcript job ${job.sourceLabel ?? job.sourceUrl}`}
            variant="danger"
          >
            <Trash2 size={15} />
          </Button>
        </div>
      </article>
      {isTranscriptJobSelected ? (
        <div className="transcript-detail-panel" aria-label="Transcript job details">
          <div className="transcript-description-editor" aria-label="Transcript description editor">
            <label>
              Description
              <input
                aria-label="Edit transcript description"
                placeholder="Optional, e.g. CDR Q2 investor conference"
                value={transcriptDescriptionDraft}
                onChange={(event) =>
                  setTranscriptDescriptionDraftByJobId((current) => ({
                    ...current,
                    [job.id]: event.target.value,
                  }))
                }
              />
            </label>
            <Button
              className="compact-button"
              disabled={transcriptDescriptionSaveInFlight === job.id || !isTranscriptDescriptionDirty}
              onClick={() => updateTranscriptJobDescription(job)}
            >
              <Save size={15} />
              Save
            </Button>
            {transcriptDescriptionError ? (
              <p className="error-text">{transcriptDescriptionError}</p>
            ) : null}
          </div>
          <dl className="source-status-grid source-status-detail">
            <div>
              <dt>Company</dt>
              <dd>{job.company ?? "Unlinked"}</dd>
            </div>
            <div>
              <dt>Status</dt>
              <dd>{formatTranscriptStatus(job.status)}</dd>
            </div>
            <div>
              <dt>Source</dt>
              <dd>{job.sourceUrl}</dd>
            </div>
            <div>
              <dt>Selected</dt>
              <dd>{selectedTranscriptSegmentIds.length}</dd>
            </div>
          </dl>
          {job.status === "failed" ? (
            <div className="transcript-error-panel" aria-label="Transcript job error">
              <strong>{job.errorCode ? formatEnumLabel(job.errorCode) : "Transcript job failed"}</strong>
              <p>{job.error ?? "No detailed provider error was stored."}</p>
            </div>
          ) : null}
          {!job.companyId ? (
            <div className="transcript-link-panel" aria-label="Link transcript company">
              <div>
                <strong>Optional company link</strong>
                <p className="muted-text">
                  Keep this transcript unlinked, or link it when selected segments should become a company notebook note.
                </p>
              </div>
              <label>
                Company or ticker
                <input
                  aria-label="Transcript link company lookup"
                  placeholder="GPW:CDR, CDR, CD PROJEKT"
                  value={transcriptLinkQuery}
                  onChange={(event) => updateTranscriptLinkQuery(job.id, event.target.value)}
                />
              </label>
              {transcriptLinkQuery ? (
                <div className="company-registry-suggestions" aria-label="Transcript link company suggestions">
                  {transcriptLinkSuggestions.length > 0 ? (
                    transcriptLinkSuggestions.map((company) => (
                      <div key={company.id}>
                        <button
                          className="company-registry-suggestion"
                          disabled={transcriptLinkInFlight === job.id}
                          onClick={() => linkTranscriptJobCompany(job.id, company)}
                          type="button"
                        >
                          <strong>{company.qualifiedTicker}</strong>
                          <span>{company.displayName}</span>
                          {company.isin ? <small>{company.isin}</small> : null}
                        </button>
                      </div>
                    ))
                  ) : (
                    <span>No tracked company matches. The transcript can stay unlinked.</span>
                  )}
                </div>
              ) : null}
              {transcriptLinkError ? <p className="error-text">{transcriptLinkError}</p> : null}
            </div>
          ) : null}
          <TranscriptSegmentReview
            job={job}
            transcriptSegments={transcriptSegments}
            transcriptSegmentsError={transcriptSegmentsError}
            transcriptSegmentSearch={transcriptSegmentSearch}
            selectedTranscriptSegmentIds={selectedTranscriptSegmentIds}
            setTranscriptSegmentSearchByJobId={setTranscriptSegmentSearchByJobId}
            toggleTranscriptSegment={toggleTranscriptSegment}
          />
          {job.status === "completed" && transcriptSegments.length > 0 ? (
            <div className="transcript-note-actions">
              <Button
                className="compact-button"
                disabled={!job.companyId || selectedTranscriptSegmentIds.length === 0}
                onClick={() => openTranscriptNoteDraft(job, selectedTranscriptSegments)}
                title={
                  job.companyId
                    ? "Create a company notebook draft from selected segments"
                    : "Link a company before saving selected segments as a company notebook note"
                }
                variant="primary"
              >
                <Plus size={15} />
                Create company note draft
              </Button>
              {transcriptNoteError ? <p className="error-text">{transcriptNoteError}</p> : null}
            </div>
          ) : null}
          {isTranscriptNoteDraftOpen ? (
            <TranscriptNoteDraft
              job={job}
              transcriptNoteForm={transcriptNoteForm}
              transcriptNoteSaveInFlight={transcriptNoteSaveInFlight}
              NotebookDateField={NotebookDateField}
              NotebookQuarterField={NotebookQuarterField}
              createTranscriptNotebookEntry={createTranscriptNotebookEntry}
              discardTranscriptNoteDraft={discardTranscriptNoteDraft}
              updateTranscriptNoteForm={updateTranscriptNoteForm}
            />
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
