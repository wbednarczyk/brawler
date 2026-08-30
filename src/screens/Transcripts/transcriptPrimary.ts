// Transcripts primary-action enum (F4b S2, docs/plans/f4b-contracts/s2-transcripts.md
// item 4 / docs/plans/frontend-v2-f4b.md § Transcripts decision 6): derived
// purely from existing screen state — no new atoms. `TranscriptsScreen.tsx`
// passes `data-ux-primary-action` + `variant="primary"` ONLY to the element
// this enum names; every other action stays quiet.
export type TranscriptPrimary = "openSettings" | "none" | "fetch" | "addToNotebook" | "saveNote";

export type DerivePrimaryInput = {
  geminiConfigured: boolean;
  /** True while the transcript list is loading or failed to load. */
  loading: boolean;
  error: boolean;
  /** Segment ids selected in the currently expanded transcript, if any. */
  selectedSegmentIds: string[];
  draftOpen: boolean;
};

export function derivePrimary({
  geminiConfigured,
  loading,
  error,
  selectedSegmentIds,
  draftOpen,
}: DerivePrimaryInput): TranscriptPrimary {
  // Missing key takes precedence over every other state.
  if (!geminiConfigured) {
    return "openSettings";
  }

  if (loading || error) {
    return "none";
  }

  if (draftOpen) {
    return "saveNote";
  }

  if (selectedSegmentIds.length > 0) {
    return "addToNotebook";
  }

  return "fetch";
}
