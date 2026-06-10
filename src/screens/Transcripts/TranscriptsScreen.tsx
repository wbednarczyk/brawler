import { RefreshCw } from "lucide-react";
import { Button } from "../../shared/components/Button";
import { EmptyState } from "../../shared/components/EmptyState";
import { useLocale } from "../../shared/locale";
import { TranscriptJobComposer } from "./TranscriptJobComposer";
import { TranscriptJobRow } from "./TranscriptJobRow";
import { TranscriptRuntimeStrip } from "./TranscriptRuntimeStrip";
import type { TranscriptsScreenProps } from "./transcriptTypes";

export function TranscriptsScreen({
  companies,
  settings,
  geminiCredentialStatus,
  transcriptJobs,
  transcriptJobsError,
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
  transcriptDescriptionDraftByJobId,
  transcriptDescriptionErrorByJobId,
  transcriptDescriptionSaveInFlight,
  transcriptCompanySuggestions,
  NotebookDateField,
  NotebookQuarterField,
  setTranscriptJobForm,
  setTranscriptJobCreateError,
  setTranscriptSegmentSearchByJobId,
  setTranscriptDescriptionDraftByJobId,
  refreshTranscriptJobs,
  createTranscriptJob,
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
  selectTranscriptCompany,
  formatAiProvider,
  formatGeminiModel,
  formatCredentialConfigured,
  formatEnumLabel,
}: TranscriptsScreenProps) {
  const { t, text } = useLocale();

  return (
    <section className="feed-panel transcripts-panel" aria-labelledby="transcripts-title">
      <div className="panel-header">
        <div>
          <h1 id="transcripts-title">{t("transcripts.title")}</h1>
          <p>{t("transcripts.description")}</p>
        </div>
        <Button
          className="compact-button"
          onClick={() => {
            void refreshTranscriptJobs();
          }}
        >
          <RefreshCw size={15} />
          {t("action.refreshJobs")}
        </Button>
      </div>

      <TranscriptRuntimeStrip
        geminiCredentialStatus={geminiCredentialStatus}
        settings={settings}
        formatAiProvider={formatAiProvider}
        formatCredentialConfigured={formatCredentialConfigured}
      />

      <TranscriptJobComposer
        createTranscriptJob={createTranscriptJob}
        selectTranscriptCompany={selectTranscriptCompany}
        setTranscriptJobCreateError={setTranscriptJobCreateError}
        setTranscriptJobForm={setTranscriptJobForm}
        transcriptCompanySuggestions={transcriptCompanySuggestions}
        transcriptJobCreateError={transcriptJobCreateError}
        transcriptJobCreateState={transcriptJobCreateState}
        transcriptJobForm={transcriptJobForm}
      />

      <div className="sources-layout" aria-label={text("Transcript jobs")}>
        {transcriptJobsError ? (
          <p className="error-text">{t("error.transcriptJobsUnavailable")}: {transcriptJobsError}</p>
        ) : null}
        {transcriptJobs.length === 0 && !transcriptJobsError ? (
          <EmptyState>{t("empty.noTranscriptJobs")}</EmptyState>
        ) : null}
        {transcriptJobs.map((job) => (
          <TranscriptJobRow
            key={job.id}
            job={job}
            companies={companies}
            settings={settings}
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
            transcriptDescriptionDraftByJobId={transcriptDescriptionDraftByJobId}
            transcriptDescriptionErrorByJobId={transcriptDescriptionErrorByJobId}
            transcriptDescriptionSaveInFlight={transcriptDescriptionSaveInFlight}
            NotebookDateField={NotebookDateField}
            NotebookQuarterField={NotebookQuarterField}
            setTranscriptSegmentSearchByJobId={setTranscriptSegmentSearchByJobId}
            setTranscriptDescriptionDraftByJobId={setTranscriptDescriptionDraftByJobId}
            toggleTranscriptJob={toggleTranscriptJob}
            toggleTranscriptJobFromKeyboard={toggleTranscriptJobFromKeyboard}
            runTranscriptJob={runTranscriptJob}
            deleteTranscriptJob={deleteTranscriptJob}
            updateTranscriptJobDescription={updateTranscriptJobDescription}
            updateTranscriptLinkQuery={updateTranscriptLinkQuery}
            linkTranscriptJobCompany={linkTranscriptJobCompany}
            toggleTranscriptSegment={toggleTranscriptSegment}
            openTranscriptNoteDraft={openTranscriptNoteDraft}
            createTranscriptNotebookEntry={createTranscriptNotebookEntry}
            discardTranscriptNoteDraft={discardTranscriptNoteDraft}
            updateTranscriptNoteForm={updateTranscriptNoteForm}
            formatAiProvider={formatAiProvider}
            formatGeminiModel={formatGeminiModel}
            formatEnumLabel={formatEnumLabel}
          />
        ))}
      </div>
    </section>
  );
}
