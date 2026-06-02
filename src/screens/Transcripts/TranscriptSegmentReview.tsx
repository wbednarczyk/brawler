import { X } from "lucide-react";
import type { TranscriptJob, TranscriptSegment } from "../../api/types";
import { Button } from "../../shared/components/Button";
import { EmptyState } from "../../shared/components/EmptyState";
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
  const filteredTranscriptSegments = transcriptSegments.filter((segment) =>
    transcriptSegmentMatchesQuery(segment, transcriptSegmentSearch),
  );

  return (
    <>
      {job.status !== "completed" ? (
        <p className="muted-text">Transcript segments will be available after the job completes.</p>
      ) : null}
      {transcriptSegmentsError ? (
        <p className="error-text">Transcript segments unavailable: {transcriptSegmentsError}</p>
      ) : null}
      {job.status === "completed" && transcriptSegments.length === 0 && !transcriptSegmentsError ? (
        <EmptyState>No transcript segments stored for this job.</EmptyState>
      ) : null}
      {transcriptSegments.length > 0 ? (
        <div className="transcript-search-panel">
          <label>
            Search transcript
            <span className="transcript-search-input-row">
              <input
                aria-label="Search transcript segments"
                placeholder="Search text, speaker, language, timestamp"
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
                  aria-label="Clear transcript search"
                  className="transcript-search-clear"
                  onClick={() =>
                    setTranscriptSegmentSearchByJobId((current) => ({
                      ...current,
                      [job.id]: "",
                    }))
                  }
                  title="Clear transcript search"
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
        <div className="transcript-segment-list" aria-label="Transcript segments">
          {filteredTranscriptSegments.length > 0 ? (
            filteredTranscriptSegments.map((segment) => (
              <label className="transcript-segment-row" key={segment.id}>
                <input
                  aria-label={`Select transcript segment ${transcriptSegmentTimestamp(segment)}`}
                  checked={selectedTranscriptSegmentIds.includes(segment.id)}
                  onChange={() => toggleTranscriptSegment(job.id, segment.id)}
                  type="checkbox"
                />
                <span className="transcript-segment-time">{transcriptSegmentTimestamp(segment)}</span>
                <span className="transcript-segment-text">
                  {highlightSearchMatch(segment.text, transcriptSegmentSearch)}
                </span>
              </label>
            ))
          ) : (
            <EmptyState>No transcript segments match this search.</EmptyState>
          )}
        </div>
      ) : null}
    </>
  );
}
