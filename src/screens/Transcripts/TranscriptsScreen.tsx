import { ActionButton, EmptyState, ErrorText, PanelHeader, Skeleton, TextField } from "../../ui";
import { useLocale } from "../../shared/locale";
import { useTranscriptsViewModel } from "../../app/state/screenViewModels";
import { derivePrimary } from "./transcriptPrimary";
import { TranscriptJobComposer } from "./TranscriptJobComposer";
import { TranscriptJobRow } from "./TranscriptJobRow";

const EMPTY_COMPOSER_FORM_ID = "transcript-empty-composer-form";

// F4b S2 redesign (docs/plans/f4b-contracts/s2-transcripts.md, approved
// mockup docs/mockups/frontend-v2-f4/transcripts.html): the empty state (no
// transcripts) IS the composer — an invitation whose `source` is the
// explanatory line + the two composer fields (inputs, not actions — the
// invitation primitive counts focusables only inside `action`) and whose
// `action` is the composer's own `Pobierz transkrypcję` button. Gemini-key-
// missing is a second, higher-precedence invitation (decision 6): the
// composer never renders while the key is missing.
export function TranscriptsScreen() {
  const {
    companies,
    geminiCredentialStatus,
    transcriptJobs,
    transcriptJobsError,
    transcriptsLoading,
    transcriptJobForm,
    transcriptJobCreateError,
    transcriptJobCreateState,
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
    transcriptCompanySuggestions,
    NotebookDateField,
    NotebookQuarterField,
    setTranscriptJobForm,
    setTranscriptJobCreateError,
    setTranscriptSegmentSearchByJobId,
    refreshTranscriptJobs,
    retryTranscriptSegments,
    createTranscriptJob,
    toggleTranscriptJob,
    toggleTranscriptJobFromKeyboard,
    runTranscriptJob,
    deleteTranscriptJob,
    updateTranscriptLinkQuery,
    linkTranscriptJobCompany,
    toggleTranscriptSegment,
    openTranscriptNoteDraft,
    createTranscriptNotebookEntry,
    discardTranscriptNoteDraft,
    updateTranscriptNoteForm,
    selectTranscriptCompany,
    openCompanyWorkspaceById,
    openSettings,
  } = useTranscriptsViewModel();
  const { t, text } = useLocale();

  const geminiConfigured = Boolean(geminiCredentialStatus?.configured);
  const primary = derivePrimary({
    geminiConfigured,
    loading: transcriptsLoading,
    error: Boolean(transcriptJobsError),
    selectedSegmentIds: selectedTranscriptJobId
      ? selectedTranscriptSegmentIdsByJobId[selectedTranscriptJobId] ?? []
      : [],
    draftOpen: transcriptNoteDraftJobId !== null,
  });

  const composer = (
    <TranscriptJobComposer
      createTranscriptJob={createTranscriptJob}
      selectTranscriptCompany={selectTranscriptCompany}
      setTranscriptJobCreateError={setTranscriptJobCreateError}
      setTranscriptJobForm={setTranscriptJobForm}
      transcriptCompanySuggestions={transcriptCompanySuggestions}
      transcriptJobCreateError={transcriptJobCreateError}
      transcriptJobCreateState={transcriptJobCreateState}
      transcriptJobForm={transcriptJobForm}
      openSettings={openSettings}
      primary={primary}
      geminiConfigured={geminiConfigured}
    />
  );

  // Missing key gates the WHOLE screen only while there is nothing to show
  // yet (no transcripts fetched so far) — an existing list (and its own
  // per-row retry gating) stays reachable even if the key was since cleared.
  const showKeyMissingInvitation = !geminiConfigured && !transcriptsLoading && transcriptJobs.length === 0;
  // The header's Refresh button doubles as the list-error retry (contract §
  // Action inventory) — ONE button total. It stays hidden while either
  // invitation (missing key / no transcripts yet) is the whole screen: there
  // is nothing to refresh yet, and the invitation's own action is the only
  // primary in that state.
  const isPureInvitation =
    showKeyMissingInvitation || (!transcriptsLoading && !transcriptJobsError && transcriptJobs.length === 0);

  return (
    <section className="feed-panel transcripts-panel" aria-labelledby="transcripts-title">
      <PanelHeader
        title={t("transcripts.title")}
        description={t("transcripts.description")}
        titleId="transcripts-title"
        actions={
          isPureInvitation ? undefined : (
            <ActionButton
              onClick={() => {
                void refreshTranscriptJobs();
              }}
              verb="refresh"
            >
              {text("Refresh transcripts")}
            </ActionButton>
          )
        }
      />

      {transcriptsLoading ? (
        <Skeleton variant="list-row" count={4} label={text("Loading transcripts…")} />
      ) : showKeyMissingInvitation ? (
        <EmptyState
          kind="invitation"
          title={text("Gemini key needed first")}
          source={text("Gemini does the transcription (the app's only AI). The key is stored in your system's keychain.")}
          action={
            <ActionButton kind="destination" data-ux-primary-action="true" onClick={openSettings} variant="primary">
              {text("Open settings")}
            </ActionButton>
          }
        />
      ) : transcriptJobsError ? (
        <>
          {composer}
          <ErrorText>
            {text("Couldn't load transcripts")}
            {": "}
            {transcriptJobsError}
          </ErrorText>
        </>
      ) : transcriptJobs.length === 0 ? (
        <EmptyState
          kind="invitation"
          title={text("You don't have any transcripts yet")}
          source={
            <>
              <p>
                {text(
                  "Paste a YouTube recording link. Gemini turns speech into text — segments you select become a company notebook note, linked to the minute in the recording.",
                )}
              </p>
              <form
                aria-label={text("New transcript")}
                className="transcript-composer transcript-composer-empty"
                id={EMPTY_COMPOSER_FORM_ID}
                onSubmit={createTranscriptJob}
              >
                <TextField
                  className="transcript-composer-url"
                  label={text("Recording link")}
                  aria-label={text("Recording link")}
                  placeholder="https://www.youtube.com/watch?v=…"
                  value={transcriptJobForm.url}
                  onChange={(event) =>
                    setTranscriptJobForm((current) => ({ ...current, url: event.target.value }))
                  }
                />
                <TextField
                  label={text("Company (optional)")}
                  aria-label={text("Company (optional)")}
                  placeholder={text("Optional, e.g. GPW:CDR, CDR, CD PROJEKT")}
                  value={transcriptJobForm.companyQuery}
                  onChange={(event) =>
                    setTranscriptJobForm((current) => ({
                      ...current,
                      companyId: "",
                      companyQuery: event.target.value,
                    }))
                  }
                />
              </form>
              {transcriptJobCreateError ? <ErrorText>{text(transcriptJobCreateError)}</ErrorText> : null}
            </>
          }
          action={
            <ActionButton
              data-ux-primary-action="true"
              disabled={transcriptJobCreateState === "refreshing" || !transcriptJobForm.url.trim()}
              form={EMPTY_COMPOSER_FORM_ID}
              type="submit"
              variant="primary"
              verb="fetch"
            >
              {transcriptJobCreateState === "refreshing" ? text("Fetching…") : text("Fetch transcript")}
            </ActionButton>
          }
        />
      ) : (
        <>
          {composer}
          <div className="transcript-list" aria-label={t("transcripts.title")}>
            {transcriptJobs.map((job) => (
              <TranscriptJobRow
                key={job.id}
                job={job}
                companies={companies}
                geminiCredentialStatus={geminiCredentialStatus}
                transcriptJobRunInFlight={transcriptJobRunInFlight}
                selectedTranscriptJobId={selectedTranscriptJobId}
                transcriptSegmentsByJobId={transcriptSegmentsByJobId}
                transcriptSegmentsErrorByJobId={transcriptSegmentsErrorByJobId}
                transcriptSegmentSearchByJobId={transcriptSegmentSearchByJobId}
                selectedTranscriptSegmentIdsByJobId={selectedTranscriptSegmentIdsByJobId}
                transcriptNoteDraftJobId={transcriptNoteDraftJobId}
                transcriptNoteForm={transcriptNoteForm}
                transcriptNoteErrorByJobId={transcriptNoteErrorByJobId}
                transcriptNoteSaveInFlight={transcriptNoteSaveInFlight}
                transcriptLinkQueryByJobId={transcriptLinkQueryByJobId}
                transcriptLinkErrorByJobId={transcriptLinkErrorByJobId}
                transcriptLinkInFlight={transcriptLinkInFlight}
                transcriptDeleteInFlight={transcriptDeleteInFlight}
                NotebookDateField={NotebookDateField}
                NotebookQuarterField={NotebookQuarterField}
                setTranscriptSegmentSearchByJobId={setTranscriptSegmentSearchByJobId}
                toggleTranscriptJob={toggleTranscriptJob}
                toggleTranscriptJobFromKeyboard={toggleTranscriptJobFromKeyboard}
                runTranscriptJob={runTranscriptJob}
                deleteTranscriptJob={deleteTranscriptJob}
                updateTranscriptLinkQuery={updateTranscriptLinkQuery}
                linkTranscriptJobCompany={linkTranscriptJobCompany}
                toggleTranscriptSegment={toggleTranscriptSegment}
                openTranscriptNoteDraft={openTranscriptNoteDraft}
                createTranscriptNotebookEntry={createTranscriptNotebookEntry}
                discardTranscriptNoteDraft={discardTranscriptNoteDraft}
                updateTranscriptNoteForm={updateTranscriptNoteForm}
                retryTranscriptSegments={retryTranscriptSegments}
                openCompanyWorkspaceById={openCompanyWorkspaceById}
                primary={primary}
              />
            ))}
          </div>
        </>
      )}
    </section>
  );
}
