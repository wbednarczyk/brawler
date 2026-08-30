import { describe, expect, it } from "vitest";
import { derivePrimary } from "./transcriptPrimary";

// F4b S2 contract (docs/plans/f4b-contracts/s2-transcripts.md item 4): one
// unit test per named row of the primary-state table
// (docs/plans/frontend-v2-f4b.md § Transcripts, § 6 Primary action).
describe("Transcripts derivePrimary (F4b contract § Transcripts, decision 6)", () => {
  it("openSettings: Gemini key missing takes precedence over every other state", () => {
    expect(
      derivePrimary({
        geminiConfigured: false,
        loading: false,
        error: false,
        selectedSegmentIds: ["seg1"],
        draftOpen: true,
      }),
    ).toBe("openSettings");
  });

  it("none: list loading, key configured", () => {
    expect(
      derivePrimary({ geminiConfigured: true, loading: true, error: false, selectedSegmentIds: [], draftOpen: false }),
    ).toBe("none");
  });

  it("none: list error, key configured", () => {
    expect(
      derivePrimary({ geminiConfigured: true, loading: false, error: true, selectedSegmentIds: [], draftOpen: false }),
    ).toBe("none");
  });

  it("fetch: nothing selected, no draft", () => {
    expect(
      derivePrimary({ geminiConfigured: true, loading: false, error: false, selectedSegmentIds: [], draftOpen: false }),
    ).toBe("fetch");
  });

  it("addToNotebook: segments selected, draft closed", () => {
    expect(
      derivePrimary({
        geminiConfigured: true,
        loading: false,
        error: false,
        selectedSegmentIds: ["seg1"],
        draftOpen: false,
      }),
    ).toBe("addToNotebook");
  });

  it("saveNote: draft open (segments still selected)", () => {
    expect(
      derivePrimary({
        geminiConfigured: true,
        loading: false,
        error: false,
        selectedSegmentIds: ["seg1"],
        draftOpen: true,
      }),
    ).toBe("saveNote");
  });

  it("saveNote: draft open even with no selection left", () => {
    expect(
      derivePrimary({ geminiConfigured: true, loading: false, error: false, selectedSegmentIds: [], draftOpen: true }),
    ).toBe("saveNote");
  });
});
