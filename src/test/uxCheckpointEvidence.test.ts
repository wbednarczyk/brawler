import { describe, expect, it } from "vitest";
import {
  isCheckpointRun,
  requireCheckpointMeta,
} from "../../tests/live/helpers/checkpointEvidence";

// Q6 (ADR 0081): the live UX checkpoint helper must REFUSE to run without full,
// valid metadata, so a checkpoint session can never produce unattributable
// evidence. These are pure env-validation branches (no live app / no fs).

describe("UX checkpoint metadata", () => {
  it("refuses when journey, card, or stage is missing", () => {
    expect(() => requireCheckpointMeta({})).toThrow(/BRAWLER_UX_JOURNEY/);
    expect(() =>
      requireCheckpointMeta({ BRAWLER_UX_JOURNEY: "J1", BRAWLER_UX_STAGE: "vertical" }),
    ).toThrow(/BRAWLER_UX_CARD/);
    expect(() =>
      requireCheckpointMeta({ BRAWLER_UX_JOURNEY: "J1", BRAWLER_UX_CARD: "a26cc6e" }),
    ).toThrow(/BRAWLER_UX_STAGE/);
  });

  it("refuses an invalid stage", () => {
    expect(() =>
      requireCheckpointMeta({
        BRAWLER_UX_JOURNEY: "J1",
        BRAWLER_UX_CARD: "a26cc6e",
        BRAWLER_UX_STAGE: "prod",
      }),
    ).toThrow(/vertical \| mid \| release/);
  });

  it("accepts full valid metadata and trims it", () => {
    expect(
      requireCheckpointMeta({
        BRAWLER_UX_JOURNEY: " J1 morning review ",
        BRAWLER_UX_CARD: " a26cc6e ",
        BRAWLER_UX_STAGE: "vertical",
      }),
    ).toEqual({ journey: "J1 morning review", card: "a26cc6e", stage: "vertical" });
  });

  it("detects a checkpoint run from any single metadata var", () => {
    expect(isCheckpointRun({})).toBe(false);
    expect(isCheckpointRun({ BRAWLER_UX_STAGE: "mid" })).toBe(true);
    expect(isCheckpointRun({ BRAWLER_UX_CARD: "a26cc6e" })).toBe(true);
  });
});
