import { describe, expect, it } from "vitest";
import { isPeriodicReportFiling } from "./periodicFiling";

// Mirrors the Rust classifier's own case set (periodic_filing_markers.json,
// #427) — decision order, first match wins: (a) body current-report marker
// → false, (b) >=2 body periodic-form markers → true, (c) title negative
// substring → false, (d) title form-code regex → true, (e) title positive
// substring → true, (f) else false. Negatives are checked BEFORE the form
// code so a form-code-looking title that also reads as preliminary/corrected
// is rejected, not short-circuited to true.
describe("isPeriodicReportFiling", () => {
  it("a body with >=2 periodic-form markers is periodic, regardless of title", () => {
    const body = "1. STRONA TYTUŁOWA\n...\nWYBRANE DANE FINANSOWE\n...";
    expect(isPeriodicReportFiling("Komunikat spółki", body)).toBe(true);
  });

  it("a body carrying the current-report marker is never periodic, even with a periodic title", () => {
    const body = "1. RAPORT BIEŻĄCY nr 15/2026\nZarząd informuje...";
    expect(isPeriodicReportFiling("Raport okresowy", body)).toBe(false);
  });

  it("a single body periodic-form marker is not enough on its own (needs >=2 or falls through to title rules)", () => {
    const body = "1. STRONA TYTUŁOWA\nsome unrelated text";
    expect(isPeriodicReportFiling("Zmiana terminu publikacji raportu", body)).toBe(false);
  });

  it("titleFormCode matches a bare form-code title with no body", () => {
    expect(isPeriodicReportFiling("Wyniki finansowe PSr /2026", null)).toBe(true);
  });

  it("a preliminary-results title is excluded by titleNegative even though it mentions results", () => {
    expect(isPeriodicReportFiling("Wstępne wyniki finansowe za 2026 rok", null)).toBe(false);
  });

  it("a publication-date-change notice is excluded by titleNegative", () => {
    expect(isPeriodicReportFiling("Zmiana terminu publikacji raportu okresowego", null)).toBe(false);
  });

  it("titlePositive matches a plain-language periodic-report title with no form code", () => {
    expect(isPeriodicReportFiling("Skonsolidowany raport kwartalny za III kwartał 2026", null)).toBe(true);
  });

  it("a title with none of the markers is not periodic", () => {
    expect(isPeriodicReportFiling("Powiadomienie o transakcjach osób zarządzających", null)).toBe(false);
  });

  it("a title negative wins over a matching form code", () => {
    expect(isPeriodicReportFiling("QSr 1/2026 - wstępne wyniki", null)).toBe(false);
  });

  it("a bare Q&A abbreviation is not a form code", () => {
    expect(isPeriodicReportFiling("Q&A z zarządem", null)).toBe(false);
  });

  it("a bare R&D abbreviation is not a form code", () => {
    expect(isPeriodicReportFiling("R & D: nowa strategia", null)).toBe(false);
  });

  it("a korekta title with no body is not periodic even though it also reads as a periodic report", () => {
    expect(isPeriodicReportFiling("Skonsolidowany raport roczny za 2025 — korekta", null)).toBe(false);
  });

  it("a periodic-form body still witnesses when a section merely mentions 'raport bieżący' in prose", () => {
    const body =
      "Spis treści:1. STRONA TYTUŁOWA2. WYBRANE DANE FINANSOWE3. KOREKTA RAPORTU4. " +
      "ZAWARTOŚĆ RAPORTUTreść korekty: dokument pozostaje raport bieżący nr 15/2026 w części opisowej.";
    expect(isPeriodicReportFiling("Korekta raportu rocznego za 2025 rok", body)).toBe(true);
  });

  it("titleFormCode matches a preliminary-looking-but-not-negative half-year form title", () => {
    expect(isPeriodicReportFiling("Wyniki finansowe PSr", null)).toBe(true);
  });

  it("titleFormCode matches a formularz-prefixed half-year form title", () => {
    expect(isPeriodicReportFiling("P /2026: formularz raportu półrocznego", null)).toBe(true);
  });

  it("matches table-of-contents markers only at an item boundary (11. is not 1.)", () => {
    expect(
      isPeriodicReportFiling(
        "cokolwiek",
        "Spis treści:1. STRONA TYTUŁOWA2. WYBRANE DANE FINANSOWE3. ZAWARTOŚĆ RAPORTU11. RAPORT BIEŻĄCY",
      ),
    ).toBe(true);
    expect(isPeriodicReportFiling("cokolwiek", "Spis treści:11. STRONA TYTUŁOWA2. PODPISY")).toBe(false);
  });
});
