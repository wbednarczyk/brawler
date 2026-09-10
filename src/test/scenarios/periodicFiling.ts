import markers from "../../../src-tauri/src/source_adapters/periodic_filing_markers.json";

// Mock-runtime witness of "is this an Official report filing a PERIODIC
// report" (F2 D4/#427) — ports the Rust classifier's exact decision order
// (`src-tauri/src/source_adapters/periodic_filing_markers.json` is the single
// shared marker source both sides read; its own `decisionOrder` field is the
// canonical statement of this order, kept here only for readability) so the
// TS mock's Today non-arrival witness stops matching on presentationKind
// alone (a current report/ESPI notice about an unrelated matter was wrongly
// witnessing a periodic-report expectation). Decision order, first match wins:
//   (a) body has a bodyCurrentReport marker → false (a current report, never periodic)
//   (b) body has >=2 bodyPeriodicForm markers → true (the PSr/QSr form skeleton)
//   (c) title contains a titleNegative substring → false (preliminary/estimate/schedule
//       change/correction) — checked BEFORE the form code, so a form-code-looking
//       title that also reads as preliminary/corrected is rejected, not short-circuited
//   (d) title matches titleFormCode → true (the "R/RS/QSr/…" form code)
//   (e) title contains a titlePositive substring → true
//   (f) else → false
const titleFormCode = new RegExp(markers.titleFormCode, "i");

// A table-of-contents marker matches only at an item boundary: "1. RAPORT
// BIEŻĄCY" must not be found inside "11. RAPORT BIEŻĄCY" (mirrors the Rust
// `contains_item_marker`).
function containsItemMarker(lowerHaystack: string, needle: string): boolean {
  const lowerNeedle = needle.toLowerCase();
  let from = 0;
  for (;;) {
    const idx = lowerHaystack.indexOf(lowerNeedle, from);
    if (idx === -1) return false;
    if (idx === 0 || !/[0-9]/.test(lowerHaystack[idx - 1])) return true;
    from = idx + 1;
  }
}

function containsMarker(haystack: string, needles: string[]): boolean {
  const lower = haystack.toLowerCase();
  return needles.some((needle) => containsItemMarker(lower, needle));
}

function countMarkers(haystack: string, needles: string[]): number {
  const lower = haystack.toLowerCase();
  return needles.reduce((count, needle) => (containsItemMarker(lower, needle) ? count + 1 : count), 0);
}

export function isPeriodicReportFiling(title: string, body: string | null): boolean {
  if (body) {
    if (containsMarker(body, markers.bodyCurrentReport)) return false;
    if (countMarkers(body, markers.bodyPeriodicForm) >= 2) return true;
  }
  const lowerTitle = title.toLowerCase();
  if (markers.titleNegative.some((needle) => lowerTitle.includes(needle.toLowerCase()))) return false;
  if (titleFormCode.test(title)) return true;
  if (markers.titlePositive.some((needle) => lowerTitle.includes(needle.toLowerCase()))) return true;
  return false;
}
