import { ChevronRight, Plus, RefreshCw, Save, Trash2 } from "lucide-react";
import { useState } from "react";
import type { TranscriptJob } from "../../api/types";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { ActionRow, Button, ChipList, DenseRow, ErrorText, InfoGrid, InlineConfirm, StatusPill, TextField } from "../../ui";
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
  const { text } = useLocale();
  // Cascading (ADR 0076 D5): deleting a job also drops its stored segments, so
  // confirm in place rather than via a native dialog.
  const [confirmDelete, setConfirmDelete] = useState(false);
  // U7-E2 density (ADR 0076 D6): segments fold behind a disclosure at S/short.
  // Collapsed by default so the CSS tier switch can hide the body only there;
  // at M/L the body renders inline regardless (the toggle is CSS-hidden).
  const [segmentsExpanded, setSegmentsExpanded] = useState(false);
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
    <div className="source-row-block" key={job.id} data-transcript-job-id={job.id}>
      <DenseRow
        className={[
          "source-row",
          "transcript-row",
          isTranscriptJobSelected ? "source-row-selected" : "",
        ]
          .filter(Boolean)
          .join(" ")}
        aria-label={`${text("Open transcript job")}: ${job.sourceUrl}`}
        onClick={() => toggleTranscriptJob(job)}
        onKeyDown={(event) => toggleTranscriptJobFromKeyboard(event, job)}
        role="button"
        selected={isTranscriptJobSelected}
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
            <h2>{job.sourceLabel ?? text("Untitled transcript")}</h2>
          </div>
          <p>
            {job.sourceUrl} ·{" "}
            {job.company ? <TickerLabel value={job.company} /> : text("Unlinked transcript")} ·{" "}
            {formatAiProvider(job.providerId)} ·{" "}
            {formatGeminiModel(settings?.aiProviders.youtubeTranscriptionModel)}
          </p>
          <ChipList ariaLabel={`${text("Transcript job metadata for")} ${job.id}`}>
            <StatusPill>{formatTranscriptStatus(job.companyResolutionStatus)}</StatusPill>
            <StatusPill>{job.sourceType}</StatusPill>
            {job.errorCode ? <StatusPill>{formatEnumLabel(job.errorCode)}</StatusPill> : null}
          </ChipList>
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
                  ? text("Retry Gemini transcription")
                  : text("Configure Gemini API key in Settings before running transcription")
              }
            >
              <RefreshCw size={15} />
              {transcriptJobRunInFlight === job.id ? text("Running") : text("Retry")}
            </Button>
          ) : null}
          {confirmDelete ? (
            <InlineConfirm
                            verb="remove"
              cancelLabel={text("Cancel")}
              confirmLabel={text("Delete")}
              disabled={transcriptDeleteInFlight === job.id}
              onCancel={() => setConfirmDelete(false)}
              onConfirm={() => {
                setConfirmDelete(false);
                deleteTranscriptJob(job);
              }}
            >
              {text("Stored transcript segments for this job will also be removed.")}
            </InlineConfirm>
          ) : (
            <Button
              className="danger-button"
              disabled={transcriptDeleteInFlight === job.id}
              onClick={(event) => {
                event.stopPropagation();
                setConfirmDelete(true);
              }}
              title={`${text("Delete transcript job")} ${job.sourceLabel ?? job.sourceUrl}`}
              variant="icon"
            >
              <Trash2 size={15} />
            </Button>
          )}
        </div>
      </DenseRow>
      {isTranscriptJobSelected ? (
        <div className="transcript-detail-panel" aria-label={text("Transcript job details")}>
          <div className="transcript-description-editor" aria-label={text("Transcript description editor")}>
            <TextField
              label={text("Description")}
              aria-label={text("Edit transcript description")}
              placeholder={text("Optional, e.g. CDR Q2 investor conference")}
              value={transcriptDescriptionDraft}
              onChange={(event) =>
                setTranscriptDescriptionDraftByJobId((current) => ({
                  ...current,
                  [job.id]: event.target.value,
                }))
              }
            />
            <Button
              className="compact-button"
              disabled={transcriptDescriptionSaveInFlight === job.id || !isTranscriptDescriptionDirty}
              onClick={() => updateTranscriptJobDescription(job)}
            >
              <Save size={15} />
              {text("Save")}
            </Button>
            {transcriptDescriptionError ? (
              <ErrorText>{transcriptDescriptionError}</ErrorText>
            ) : null}
          </div>
          <InfoGrid
            className="source-status-grid source-status-detail"
            items={[
              {
                label: text("Company"),
                value: job.company ? <TickerLabel exposeText={false} value={job.company} /> : text("Unlinked"),
              },
              { label: text("Status"), value: formatTranscriptStatus(job.status) },
              { label: text("Source"), value: job.sourceUrl },
              { label: text("Selected"), value: selectedTranscriptSegmentIds.length },
            ]}
          />
          {job.status === "failed" ? (
            <div className="transcript-error-panel" aria-label={text("Transcript job error")}>
              <strong>{job.errorCode ? formatEnumLabel(job.errorCode) : text("Transcript job failed")}</strong>
              <p>{job.error ?? text("No detailed provider error was stored.")}</p>
            </div>
          ) : null}
          {!job.companyId ? (
            <div className="transcript-link-panel" aria-label={text("Link transcript company")}>
              <div>
                <strong>{text("Optional company link")}</strong>
                <p className="muted-text">
                  {text("Keep this transcript unlinked, or link it when selected segments should become a company notebook note.")}
                </p>
              </div>
              <TextField
                label={text("Company or ticker")}
                aria-label={text("Transcript link company lookup")}
                placeholder="GPW:CDR, CDR, CD PROJEKT"
                value={transcriptLinkQuery}
                onChange={(event) => updateTranscriptLinkQuery(job.id, event.target.value)}
              />
              {transcriptLinkQuery ? (
                <div className="company-registry-suggestions" aria-label={text("Transcript link company suggestions")}>
                  {transcriptLinkSuggestions.length > 0 ? (
                    transcriptLinkSuggestions.map((company) => (
                      <div key={company.id}>
                        <button
                          className="company-registry-suggestion"
                          disabled={transcriptLinkInFlight === job.id}
                          onClick={() => linkTranscriptJobCompany(job.id, company)}
                          type="button"
                        >
                          <strong><TickerLabel value={company.qualifiedTicker} /></strong>
                          <span>{company.displayName}</span>
                          {company.isin ? <small>{company.isin}</small> : null}
                        </button>
                      </div>
                    ))
                  ) : (
                    <span>{text("No tracked company matches. The transcript can stay unlinked.")}</span>
                  )}
                </div>
              ) : null}
              {transcriptLinkError ? <ErrorText>{transcriptLinkError}</ErrorText> : null}
            </div>
          ) : null}
          {/* U7-E2 density contract (ADR 0076 D6): segments fold behind the
              disclosure at the S/short tiers (reachable, never lost); at L the
              segments sit beside the note draft. The toggle is CSS-hidden above
              S — segments render inline there regardless of `segmentsExpanded`. */}
          <div className="transcript-review-columns">
            <div className={segmentsExpanded ? "transcript-segments transcript-segments-open" : "transcript-segments"}>
              <button
                aria-expanded={segmentsExpanded}
                className="transcript-segments-toggle"
                onClick={() => setSegmentsExpanded((open) => !open)}
                type="button"
              >
                <ChevronRight aria-hidden="true" className="transcript-segments-chevron" size={15} />
                {text("Segments")}
              </button>
              <div className="transcript-segments-body">
                <TranscriptSegmentReview
                  job={job}
                  transcriptSegments={transcriptSegments}
                  transcriptSegmentsError={transcriptSegmentsError}
                  transcriptSegmentSearch={transcriptSegmentSearch}
                  selectedTranscriptSegmentIds={selectedTranscriptSegmentIds}
                  setTranscriptSegmentSearchByJobId={setTranscriptSegmentSearchByJobId}
                  toggleTranscriptSegment={toggleTranscriptSegment}
                />
              </div>
            </div>
            <div className="transcript-note-column">
              {job.status === "completed" && transcriptSegments.length > 0 ? (
                <ActionRow className="transcript-note-actions">
                  <Button
                    className="compact-button"
                    disabled={!job.companyId || selectedTranscriptSegmentIds.length === 0}
                    onClick={() => openTranscriptNoteDraft(job, selectedTranscriptSegments)}
                    title={
                      job.companyId
                        ? text("Create a company notebook draft from selected segments")
                        : text("Link a company before saving selected segments as a company notebook note")
                    }
                    variant="primary"
                  >
                    <Plus size={15} />
                    {text("Create company note draft")}
                  </Button>
                  {transcriptNoteError ? <ErrorText>{transcriptNoteError}</ErrorText> : null}
                </ActionRow>
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
          </div>
        </div>
      ) : null}
    </div>
  );
}
