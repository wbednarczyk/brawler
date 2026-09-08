import { describe, expect, it } from "vitest";

import { makeTextTranslator } from "../../locale";
import { plText } from "../../locale/resources/plText";
import type { PresentationKind } from "../../../api/generated/PresentationKind";
import { feedKindChip } from "./feedPresentation";

// Every `PresentationKind` member (dogfooding #8, same technique as
// `alertLabels.test.ts`): the `satisfies Record` object fails to compile
// when the generated union gains a member this list does not name.
const ALL_PRESENTATION_KINDS = {
  media: true,
  filing: true,
  report: true,
  redFlag: true,
} satisfies Record<PresentationKind, true>;
const ALL_KINDS = Object.keys(ALL_PRESENTATION_KINDS) as PresentationKind[];

// Identity `text()`: returns the source string untouched, so the label
// recovers the raw EN source constant regardless of locale — used to look
// that constant up in `plText` directly.
const identityText = (value: string) => value;

describe("feedKindChip — exhaustive over PresentationKind (dogfooding #8)", () => {
  it.each(ALL_KINDS)("returns a non-empty label + tone for %s — never null", (kind) => {
    const chip = feedKindChip(kind, identityText);
    expect(chip.label.length).toBeGreaterThan(0);
    expect(chip.tone).toBeTruthy();
  });

  it.each(ALL_KINDS)("%s resolves a real plText entry — Polish never falls back to the English source", (kind) => {
    const source = feedKindChip(kind, identityText).label;
    expect(plText[source as keyof typeof plText], `missing plText entry for "${source}"`).toBeDefined();
  });

  // Regression: `FeedDetailContent`'s old detail-only map returned `null` for
  // `redFlag`, which meant NO chip anywhere for it (not even the wrong one).
  it("redFlag gets a real label + a danger tone, never a null chip", () => {
    const chip = feedKindChip("redFlag", identityText);
    expect(chip.label).toBe("Red flag");
    expect(chip.tone).toBe("danger");
  });

  it("English labels", () => {
    const text = makeTextTranslator("en");
    expect(feedKindChip("media", text).label).toBe("Media");
    expect(feedKindChip("filing", text).label).toBe("ESPI notice");
    expect(feedKindChip("report", text).label).toBe("Periodic report");
    expect(feedKindChip("redFlag", text).label).toBe("Red flag");
  });

  it("Polish labels", () => {
    const text = makeTextTranslator("pl");
    expect(feedKindChip("media", text).label).toBe(plText["Media"]);
    expect(feedKindChip("filing", text).label).toBe(plText["ESPI notice"]);
    expect(feedKindChip("report", text).label).toBe(plText["Periodic report"]);
    expect(feedKindChip("redFlag", text).label).toBe(plText["Red flag"]);
  });
});
