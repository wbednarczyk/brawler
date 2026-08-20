import { describe, expect, it } from "vitest";
import { parseFilingBody } from "./parseFilingBody";

// Real-shaped sample: ESPI/EBI `body_text` glues section labels directly onto
// their values (no separating whitespace) — this mirrors an actual stored blob.
const REAL_SHAPED_BODY =
  "Spis treści:1. RAPORT BIEŻĄCY2. MESSAGE (ENGLISH VERSION)KOMISJA NADZORU FINANSOWEGO" +
  "Raport bieżący nr22/2026Data sporządzenia:2026-08-19Skrócona nazwa emitentaPZU" +
  "Podstawa prawnaArt. 56 ust. 1 pkt 2 Ustawy o ofercie – informacje bieżące i okresowe" +
  "Treść raportu:Rada Nadzorcza powołała w skład Zarządu PZU SA Pana Piotra Matczuka, " +
  "powierzając mu funkcję Członka Zarządu Spółki. Uchwała weszła w życie z chwilą podjęcia.";

describe("parseFilingBody", () => {
  it("extracts the report number, legal basis, and content from a real-shaped blob", () => {
    const parsed = parseFilingBody(REAL_SHAPED_BODY);
    expect(parsed.reportNumber).toBe("22/2026");
    expect(parsed.legalBasis).toBe(
      "Art. 56 ust. 1 pkt 2 Ustawy o ofercie – informacje bieżące i okresowe",
    );
    expect(parsed.content).toBe(
      "Rada Nadzorcza powołała w skład Zarządu PZU SA Pana Piotra Matczuka, powierzając mu " +
        "funkcję Członka Zarządu Spółki. Uchwała weszła w życie z chwilą podjęcia.",
    );
  });

  it("returns a null content when the blob has no 'Treść raportu:' marker (section hidden)", () => {
    const body =
      "Spis treści:1. RAPORT BIEŻĄCYRaport bieżący nr5/2026Data sporządzenia:2026-08-01" +
      "Podstawa prawnaArt. 17 ust. 1 MAR – informacje poufne";
    const parsed = parseFilingBody(body);
    expect(parsed.content).toBeNull();
    expect(parsed.reportNumber).toBe("5/2026");
    expect(parsed.legalBasis).toBe("Art. 17 ust. 1 MAR – informacje poufne");
  });

  it("returns every field null for an empty string", () => {
    expect(parseFilingBody("")).toEqual({ reportNumber: null, legalBasis: null, content: null });
  });

  it("caps content at ~600 chars with an ellipsis", () => {
    const longContent = "x".repeat(650);
    const parsed = parseFilingBody(`Treść raportu:${longContent}`);
    expect(parsed.content).toHaveLength(601);
    expect(parsed.content?.endsWith("…")).toBe(true);
    expect(parsed.content?.startsWith("x".repeat(600))).toBe(true);
  });

  it("bounds the content at the trailing form sections (bilingual blob never leaks)", () => {
    const body =
      REAL_SHAPED_BODY +
      "MESSAGE (ENGLISH VERSION)The Supervisory Board appointed…INFORMACJE O PODMIOCIE" +
      "PZU SAPODPISY OSÓB REPREZENTUJĄCYCH SPÓŁKĘ";
    const parsed = parseFilingBody(body);
    expect(parsed.content).not.toContain("MESSAGE");
    expect(parsed.content).not.toContain("Supervisory Board");
    expect(parsed.content).toContain("Uchwała weszła w życie");
  });

  it("ignores a quoted marker outside the KNF form (prose before the header)", () => {
    const body =
      'W artykule cytowano frazę "Treść raportu:" bez znaczenia strukturalnego. ' +
      REAL_SHAPED_BODY;
    const parsed = parseFilingBody(body);
    expect(parsed.content).toContain("Rada Nadzorcza powołała");
    expect(parsed.content).not.toContain("bez znaczenia strukturalnego");
  });
});
