// Pure parser for the ESPI/EBI `body_text` blob (F1 S4): a filing's stored body
// glues section labels directly onto their values with no separating
// whitespace (`"Raport bieżący nr22/2026Data sporządzenia:2026-08-19…"`), so
// this is deterministic substring slicing between known markers, not a
// generic text parser. Real-shaped sample lives in the test file.

export type ParsedFilingBody = {
  reportNumber: string | null;
  legalBasis: string | null;
  content: string | null;
};

// The detail pane caps the displayed content excerpt; the full body remains
// reachable through "Open source" (ADR 0104 dec. 5 — detail shrinks to its content).
const CONTENT_MAX_CHARS = 600;

/** The text strictly between `startMarker` and the first of `endMarkers` found
 * after it (or end of string when none match), trimmed. `null` when
 * `startMarker` is absent or the captured span is empty after trimming. */
function captureBetween(source: string, startMarker: string, endMarkers: readonly string[]): string | null {
  const startIndex = source.indexOf(startMarker);
  if (startIndex === -1) return null;

  const from = startIndex + startMarker.length;
  let end = source.length;
  for (const marker of endMarkers) {
    const markerIndex = source.indexOf(marker, from);
    if (markerIndex !== -1 && markerIndex < end) end = markerIndex;
  }

  const captured = source.slice(from, end).trim();
  return captured.length > 0 ? captured : null;
}

export function parseFilingBody(bodyText: string): ParsedFilingBody {
  const reportNumber = captureBetween(bodyText, "Raport bieżący nr", [
    "Data sporządzenia",
    "Podstawa prawna",
    "Treść raportu:",
  ]);
  const legalBasis = captureBetween(bodyText, "Podstawa prawna", ["Treść raportu:"]);
  const rawContent = captureBetween(bodyText, "Treść raportu:", []);
  const content =
    rawContent && rawContent.length > CONTENT_MAX_CHARS
      ? `${rawContent.slice(0, CONTENT_MAX_CHARS)}…`
      : rawContent;

  return { reportNumber, legalBasis, content };
}
