import { X } from "lucide-react";
import type { TranscriptJob, TranscriptSegment } from "../../api/types";
import { Button, EmptyState, ErrorText } from "../../ui";
import { useLocale } from "../../shared/locale";
import {
  highlightSearchMatch,
  transcriptSegmentMatchesQuery,
  transcriptSegmentTimestamp,
} from "./transcriptHelpers";
import type { TranscriptsScreenProps } from "./transcriptTypes";

type TranscriptSegmentReviewProps = Pick<
  TranscriptsScreenProps,
  | "setTranscriptSegmentSearchByJobId"
  | "toggleTranscriptSegment"
> & {
  job: TranscriptJob;
  transcriptSegments: TranscriptSegment[];
  transcriptSegmentsError: string | null | undefined;
  transcriptSegmentSearch: string;
  selectedTranscriptSegmentIds: string[];
};

export function TranscriptSegmentReview({
  job,
  transcriptSegments,
  transcriptSegmentsError,
  transcriptSegmentSearch,
  selectedTranscriptSegmentIds,
  setTranscriptSegmentSearchByJobId,
  toggleTranscriptSegment,
}: TranscriptSegmentReviewProps) {
  const { text } = useLocale();
  const filteredTranscriptSegments = transcriptSegments.filter((segment) =>
    transcriptSegmentMatchesQuery(segment, transcriptSegmentSearch),
  );

  return (
    <>
      {job.status !== "completed" ? (
        <p className="muted-text">{text("Transcript segments will be available after the job completes.")}</p>
      ) : null}
      {transcriptSegmentsError ? (
        <ErrorText>{text("Transcript segments unavailable")}: {transcriptSegmentsError}</ErrorText>
      ) : null}
      {job.status === "completed" && transcriptSegments.length === 0 && !transcriptSegmentsError ? (
        <EmptyState>{text("No transcript segments stored for this job.")}</EmptyState>
      ) : null}
      {transcriptSegments.length > 0 ? (
        <div className="transcript-search-panel">
          <label>
            {text("Search transcript")}
            <span className="transcript-search-input-row">
              {/* eslint-disable-next-line no-restricted-syntax -- bespoke search panel: 2-column input+clear grid paired with an external result-count; SearchField's icon/clear layout does not compose here */}
              <input
                aria-label={text("Search transcript segments")}
                placeholder={text("Search text, speaker, language, timestamp")}
                value={transcriptSegmentSearch}
                onChange={(event) =>
                  setTranscriptSegmentSearchByJobId((current) => ({
                    ...current,
                    [job.id]: event.target.value,
                  }))
                }
              />
              {transcriptSegmentSearch ? (
                <Button
                  aria-label={text("Clear transcript search")}
                  className="transcript-search-clear"
                  onClick={() =>
                    setTranscriptSegmentSearchByJobId((current) => ({
                      ...current,
                      [job.id]: "",
                    }))
                  }
                  title={text("Clear transcript search")}
                  variant="icon"
                >
                  <X size={13} />
                </Button>
              ) : null}
            </span>
          </label>
          <span className="transcript-search-count">
            {filteredTranscriptSegments.length}/{transcriptSegments.length}
          </span>
        </div>
      ) : null}
      {transcriptSegments.length > 0 ? (
        <div className="transcript-segment-list" aria-label={text("Transcript segments")}>
          {filteredTranscriptSegments.length > 0 ? (
            filteredTranscriptSegments.map((segment) => (
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
            <EmptyState>{text("No transcript segments match this search.")}</EmptyState>
          )}
        </div>
      ) : null}
    </>
  );
}
