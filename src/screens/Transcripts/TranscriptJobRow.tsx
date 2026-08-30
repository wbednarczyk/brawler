import { useEffect, useRef, useState } from "react";
import type { TranscriptJob } from "../../api/types";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { pluralNoun, SEGMENT_FORMS } from "../../shared/locale/plural";
import { ActionButton, ErrorText, Figure, InlineConfirm, TextField } from "../../ui";
import { transcriptErrorCodeLabel, transcriptJobTitle, transcriptStatusLabel } from "./transcriptHelpers";
import type { TranscriptPrimary } from "./transcriptPrimary";
import { TranscriptNoteDraft } from "./TranscriptNoteDraft";
import { TranscriptSegmentReview } from "./TranscriptSegmentReview";
import type { TranscriptsScreenProps } from "./transcriptTypes";

type TranscriptJobRowProps = Pick<
  TranscriptsScreenProps,
  | "companies"
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
  | "retryTranscriptSegments"
  | "openCompanyWorkspaceById"
> & {
  job: TranscriptJob;
  primary: TranscriptPrimary;
};

// F4b S2 (#430b, mockup plansza 2): a real `<button aria-pressed>` row (never
// an `<article role="button">` wrapping real buttons — the a11y defect S1
// flagged) with exactly ONE status chip. Row actions (fetch again / link
// company / remove) render as SIBLINGS of that button, never nested inside it
// (nested-interactive).
export function TranscriptJobRow({
  job,
  companies,
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
  retryTranscriptSegments,
  openCompanyWorkspaceById,
  primary,
}: TranscriptJobRowProps) {
  const { text, locale } = useLocale();
  // Cascading (ADR 0076 D5): deleting a job also drops its stored segments, so
  // confirm in place rather than via a native dialog.
  const [confirmDelete, setConfirmDelete] = useState(false);
  // U7-E2 density (ADR 0076 D6): segments fold behind a disclosure at S/short.
  const [segmentsExpanded, setSegmentsExpanded] = useState(false);
  // Fix wave (integrator review): the company picker opens on demand from
  // `Link company` — not unconditionally whenever the row is expanded.
  const [linkPickerOpen, setLinkPickerOpen] = useState(false);
  // Rename (fix wave, deviation #1 rejected): a quiet inline title edit,
  // reusing the existing `updateTranscriptJobDescription` (sourceLabel) path.
  const [renameOpen, setRenameOpen] = useState(false);
  // Success strip (item 3): "just saved" is a transient, row-local flag — the
  // controller clears the draft on both save and discard, so this component
  // tells them apart itself (discard flips `discardedRef` first).
  const [justSavedCompanyName, setJustSavedCompanyName] = useState<string | null>(null);
  const discardedRef = useRef(false);
  const wasDraftOpenRef = useRef(false);

  const isSelected = selectedTranscriptJobId === job.id;
  const segments = transcriptSegmentsByJobId[job.id] ?? [];
  const segmentSearch = transcriptSegmentSearchByJobId[job.id] ?? "";
  const selectedSegmentIds = selectedTranscriptSegmentIdsByJobId[job.id] ?? [];
  const selectedSegments = segments.filter((segment) => selectedSegmentIds.includes(segment.id));
  const segmentsError = transcriptSegmentsErrorByJobId[job.id] ?? null;
  const noteError = transcriptNoteErrorByJobId[job.id] ?? null;
  const linkQuery = transcriptLinkQueryByJobId[job.id] ?? "";
  const linkError = transcriptLinkErrorByJobId[job.id] ?? null;
  const draftOpen = transcriptNoteDraftJobId === job.id;
  const title = transcriptJobTitle(job.sourceLabel);
  const renameDraft = transcriptDescriptionDraftByJobId[job.id] ?? job.sourceLabel ?? "";
  const renameError = transcriptDescriptionErrorByJobId[job.id] ?? null;

  useEffect(() => {
    if (wasDraftOpenRef.current && !draftOpen) {
      if (discardedRef.current) {
        discardedRef.current = false;
      } else if (job.companyId) {
        setJustSavedCompanyName(job.companyName ?? job.company ?? "");
      }
    }
    wasDraftOpenRef.current = draftOpen;
  }, [draftOpen, job.company, job.companyId, job.companyName]);

  useEffect(() => {
    if (!isSelected) {
      setJustSavedCompanyName(null);
    }
  }, [isSelected]);

  // The picker closes itself once the link lands (job.companyId set) or the
  // row collapses — never lingers open for a job that no longer needs it.
  useEffect(() => {
    if (!isSelected || job.companyId) {
      setLinkPickerOpen(false);
    }
  }, [isSelected, job.companyId]);

  const linkSuggestions = linkQuery.trim()
    ? companies
        .filter((company) => {
          const query = linkQuery.trim().toLowerCase();
          return (
            company.qualifiedTicker.toLowerCase().includes(query) ||
            company.ticker.toLowerCase().includes(query) ||
            company.displayName.toLowerCase().includes(query) ||
            (company.isin?.toLowerCase().includes(query) ?? false)
          );
        })
        .slice(0, 6)
    : [];

  const dateLabel = job.status === "queued" ? text("Added") : job.status === "running" ? text("Started") : text("Fetched");
  const dateValue =
    job.status === "queued" ? job.createdAt : job.status === "running" ? job.startedAt ?? job.createdAt : job.finishedAt ?? job.createdAt;

  return (
    <div className="transcript-row-block" data-transcript-job-id={job.id}>
      <div className={["transcript-row-line", isSelected ? "transcript-row-line-selected" : ""].filter(Boolean).join(" ")}>
        <button
          aria-label={`${text("Open transcript")}: ${title}`}
          aria-pressed={isSelected}
          className="transcript-row-button"
          data-action-kind="control"
          onClick={() => toggleTranscriptJob(job)}
          onKeyDown={(event) => toggleTranscriptJobFromKeyboard(event, job)}
          type="button"
        >
          <span className="transcript-row-main">
            <span className="transcript-row-title">{title}</span>
            <span className="transcript-row-url">{job.sourceUrl}</span>
            {job.status === "failed" ? (
              <span className="transcript-row-reason">{text(transcriptErrorCodeLabel(job.errorCode))}</span>
            ) : null}
          </span>
          <span className="transcript-row-status" data-transcript-status={job.status}>
            {text(transcriptStatusLabel(job.status))}
          </span>
          <span className="transcript-row-company">
            <span className="transcript-row-kv-label">{text("Company")}</span>
            {job.companyId ? (
              <span className="transcript-row-kv-value">
                <TickerLabel value={job.company ?? ""} /> {job.companyName}
              </span>
            ) : (
              <span className="transcript-row-kv-value transcript-row-unlinked">—</span>
            )}
          </span>
          <span className="transcript-row-figures">
            <span className="transcript-row-kv-label">{dateLabel}</span>
            <span className="transcript-row-kv-value">
              {dateValue ? <Figure kind="datetime" value={dateValue} /> : "—"}
              {job.status === "completed" && segments.length > 0 ? (
                <>
                  {" · "}
                  <Figure kind="count" value={segments.length} /> {pluralNoun(locale, segments.length, SEGMENT_FORMS)}
                </>
              ) : null}
            </span>
          </span>
        </button>
        <span className="transcript-row-actions">
          {renameOpen ? (
            <>
              <TextField
                aria-label={text("Transcript title")}
                value={renameDraft}
                onChange={(event) =>
                  setTranscriptDescriptionDraftByJobId((current) => ({
                    ...current,
                    [job.id]: event.target.value,
                  }))
                }
              />
              <ActionButton
                disabled={transcriptDescriptionSaveInFlight === job.id}
                onClick={() => {
                  updateTranscriptJobDescription(job);
                  setRenameOpen(false);
                }}
                variant="ghost"
                verb="save"
              >
                {text("Save")}
              </ActionButton>
              <ActionButton kind="control" onClick={() => setRenameOpen(false)} variant="ghost">
                {text("Cancel")}
              </ActionButton>
              {renameError ? <ErrorText>{renameError}</ErrorText> : null}
            </>
          ) : (
            <ActionButton
              onClick={() => {
                setTranscriptDescriptionDraftByJobId((current) => ({
                  ...current,
                  [job.id]: job.sourceLabel ?? "",
                }));
                setRenameOpen(true);
              }}
              variant="ghost"
              verb="rename"
            >
              {text("Rename")}
            </ActionButton>
          )}
          {!job.companyId ? (
            <ActionButton
              kind="control"
              onClick={() => {
                if (!isSelected) toggleTranscriptJob(job);
                setLinkPickerOpen(true);
              }}
              variant="ghost"
            >
              {text("Link company")}
            </ActionButton>
          ) : null}
          {job.status === "queued" || job.status === "failed" ? (
            <ActionButton
              disabled={transcriptJobRunInFlight === job.id || !geminiCredentialStatus?.configured}
              onClick={() => runTranscriptJob(job.id)}
              title={
                geminiCredentialStatus?.configured
                  ? undefined
                  : text("Configure Gemini API key in Settings before running transcription")
              }
              variant="ghost"
              verb="fetch"
            >
              {transcriptJobRunInFlight === job.id ? text("Fetching…") : text("Fetch again")}
            </ActionButton>
          ) : null}
          {confirmDelete ? (
            <InlineConfirm
              cancelLabel={text("Cancel")}
              confirmLabel={text("Remove")}
              disabled={transcriptDeleteInFlight === job.id}
              onCancel={() => setConfirmDelete(false)}
              onConfirm={() => {
                setConfirmDelete(false);
                deleteTranscriptJob(job);
              }}
              verb="remove"
            >
              {text("Segments for this transcript will also be removed.")}
            </InlineConfirm>
          ) : (
            <ActionButton
              disabled={transcriptDeleteInFlight === job.id}
              onClick={() => setConfirmDelete(true)}
              variant="ghost"
              verb="remove"
            >
              {text("Remove")}
            </ActionButton>
          )}
        </span>
      </div>

      {isSelected ? (
        <div className="transcript-detail-panel" aria-label={`${text("Transcript details")}: ${title}`}>
          {!job.companyId && linkPickerOpen ? (
            <div className="transcript-link-panel" aria-label={text("Link transcript company")}>
              <TextField
                label={text("Company or ticker")}
                aria-label={text("Transcript link company lookup")}
                placeholder="GPW:CDR, CDR, CD PROJEKT"
                value={linkQuery}
                onChange={(event) => updateTranscriptLinkQuery(job.id, event.target.value)}
              />
              {linkQuery ? (
                <div className="company-registry-suggestions" aria-label={text("Transcript link company suggestions")}>
                  {linkSuggestions.length > 0 ? (
                    linkSuggestions.map((company) => (
                      <div key={company.id}>
                        <button
                          className="company-registry-suggestion"
                          disabled={transcriptLinkInFlight === job.id}
                          onClick={() => linkTranscriptJobCompany(job.id, company)}
                          type="button"
                        >
                          <strong><TickerLabel value={company.qualifiedTicker} /></strong>
                          <span>{company.displayName}</span>
                        </button>
                      </div>
                    ))
                  ) : (
                    <span>{text("No tracked company matches. The transcript can stay unlinked.")}</span>
                  )}
                </div>
              ) : null}
              {linkError ? <ErrorText>{linkError}</ErrorText> : null}
              <ActionButton kind="control" onClick={() => setLinkPickerOpen(false)} variant="ghost">
                {text("Cancel")}
              </ActionButton>
            </div>
          ) : null}
          {justSavedCompanyName ? (
            <div className="transcript-note-saved-strip">
              <span>{text("Note saved to the {company} notebook").replace("{company}", justSavedCompanyName)}</span>
              <ActionButton kind="destination" onClick={() => openCompanyWorkspaceById(job.companyId!, "Notebook")} variant="ghost">
                {text("Open notebook")}
              </ActionButton>
            </div>
          ) : null}
          <div className="transcript-review-columns">
            <div className={segmentsExpanded ? "transcript-segments transcript-segments-open" : "transcript-segments"}>
              <button
                aria-expanded={segmentsExpanded}
                className="transcript-segments-toggle"
                onClick={() => setSegmentsExpanded((open) => !open)}
                type="button"
              >
                {text("Show segments")}
              </button>
              <div className="transcript-segments-body">
                <TranscriptSegmentReview
                  job={job}
                  transcriptSegments={segments}
                  transcriptSegmentsError={segmentsError}
                  transcriptSegmentSearch={segmentSearch}
                  selectedTranscriptSegmentIds={selectedSegmentIds}
                  setTranscriptSegmentSearchByJobId={setTranscriptSegmentSearchByJobId}
                  toggleTranscriptSegment={toggleTranscriptSegment}
                  retryTranscriptSegments={retryTranscriptSegments}
                  openTranscriptNoteDraft={openTranscriptNoteDraft}
                  selectedSegments={selectedSegments}
                  primary={primary}
                  draftOpen={draftOpen}
                  noteError={noteError}
                />
              </div>
            </div>
            {draftOpen ? (
              <TranscriptNoteDraft
                job={job}
                transcriptNoteForm={transcriptNoteForm}
                transcriptNoteSaveInFlight={transcriptNoteSaveInFlight}
                NotebookDateField={NotebookDateField}
                NotebookQuarterField={NotebookQuarterField}
                createTranscriptNotebookEntry={createTranscriptNotebookEntry}
                discardTranscriptNoteDraft={() => {
                  discardedRef.current = true;
                  discardTranscriptNoteDraft();
                }}
                updateTranscriptNoteForm={updateTranscriptNoteForm}
                primary={primary}
              />
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  );
}
