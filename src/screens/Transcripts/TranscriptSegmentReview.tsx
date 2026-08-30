import type { TranscriptJob, TranscriptSegment } from "../../api/types";
import { ActionButton, EmptyState, ErrorText, Figure, SearchField } from "../../ui";
import { useLocale } from "../../shared/locale";
import type { TranscriptPrimary } from "./transcriptPrimary";
import {
  highlightSearchMatch,
  transcriptSegmentMatchesQuery,
  transcriptSegmentTimestamp,
} from "./transcriptHelpers";
import type { TranscriptsScreenProps } from "./transcriptTypes";

type TranscriptSegmentReviewProps = Pick<
  TranscriptsScreenProps,
  "setTranscriptSegmentSearchByJobId" | "toggleTranscriptSegment" | "retryTranscriptSegments" | "openTranscriptNoteDraft"
> & {
  job: TranscriptJob;
  transcriptSegments: TranscriptSegment[];
  transcriptSegmentsError: string | null | undefined;
  transcriptSegmentSearch: string;
  selectedTranscriptSegmentIds: string[];
  selectedSegments: TranscriptSegment[];
  primary: TranscriptPrimary;
  draftOpen: boolean;
  noteError: string | null;
};

// F4b S2 (item 3): search + counter + "Dodaj do notatnika" (the ONLY primary
// candidate this component can carry — the screen-level `primary` enum names
// it explicitly rather than this component guessing from local selection
// state alone, since the draft-open state lives one level up).
export function TranscriptSegmentReview({
  job,
  transcriptSegments,
  transcriptSegmentsError,
  transcriptSegmentSearch,
  selectedTranscriptSegmentIds,
  selectedSegments,
  setTranscriptSegmentSearchByJobId,
  toggleTranscriptSegment,
  retryTranscriptSegments,
  openTranscriptNoteDraft,
  primary,
  draftOpen,
  noteError,
}: TranscriptSegmentReviewProps) {
  const { text } = useLocale();
  const filteredSegments = transcriptSegments.filter((segment) =>
    transcriptSegmentMatchesQuery(segment, transcriptSegmentSearch),
  );
  const setSearch = (value: string) =>
    setTranscriptSegmentSearchByJobId((current) => ({ ...current, [job.id]: value }));

  return (
    <>
      {job.status !== "completed" ? (
        <p className="muted-text">{text("Segments will be available once the transcript is ready.")}</p>
      ) : null}
      {transcriptSegmentsError ? (
        <ErrorText>
          {text("Transcript segments unavailable")}: {transcriptSegmentsError}{" "}
          <ActionButton onClick={() => retryTranscriptSegments(job.id)} variant="ghost" verb="fetch">
            {text("Fetch segments again")}
          </ActionButton>
        </ErrorText>
      ) : null}
      {job.status === "completed" && transcriptSegments.length === 0 && !transcriptSegmentsError ? (
        <EmptyState>{text("No transcript segments stored.")}</EmptyState>
      ) : null}
      {transcriptSegments.length > 0 ? (
        <div className="transcript-search-panel">
          <SearchField
            ariaLabel={text("Search transcript segments")}
            className="transcript-search-field"
            clearLabel={text("Clear search")}
            onChange={setSearch}
            onClear={() => setSearch("")}
            placeholder={text("Search text, speaker, language, timestamp")}
            value={transcriptSegmentSearch}
          />
          <span className="transcript-search-count">
            <Figure kind="count" value={selectedTranscriptSegmentIds.length} /> {text("selected of")}{" "}
            <Figure kind="count" value={transcriptSegments.length} />
          </span>
          <ActionButton
            data-ux-primary-action={primary === "addToNotebook" ? "true" : undefined}
            disabled={!job.companyId || selectedTranscriptSegmentIds.length === 0 || draftOpen}
            onClick={() => openTranscriptNoteDraft(job, selectedSegments)}
            variant={primary === "addToNotebook" ? "primary" : "secondary"}
            verb="add"
          >
            {text("Add to notebook")}
          </ActionButton>
          {noteError ? <ErrorText>{noteError}</ErrorText> : null}
        </div>
      ) : null}
      {transcriptSegments.length > 0 ? (
        <div className="transcript-segment-list" aria-label={text("Transcript segments")}>
          {filteredSegments.length > 0 ? (
            filteredSegments.map((segment) => (
              <label className="transcript-segment-row" key={segment.id}>
                <input
                  aria-label={`${text("Select transcript segment")} ${text(transcriptSegmentTimestamp(segment))}`}
                  checked={selectedTranscriptSegmentIds.includes(segment.id)}
                  onChange={() => toggleTranscriptSegment(job.id, segment.id)}
                  type="checkbox"
                />
                <span className="transcript-segment-time">{text(transcriptSegmentTimestamp(segment))}</span>
                <span className="transcript-segment-text">
                  {highlightSearchMatch(segment.text, transcriptSegmentSearch)}
                </span>
              </label>
            ))
          ) : (
            <EmptyState kind="quiet" reason={text('No segment contains "{query}"').replace("{query}", transcriptSegmentSearch)} />
          )}
        </div>
      ) : null}
    </>
  );
}
