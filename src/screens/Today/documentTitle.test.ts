import { describe, expect, it } from "vitest";

import { splitDocumentTitle } from "./documentTitle";

describe("splitDocumentTitle", () => {
  it("splits a filename glued onto the human title (owner case)", () => {
    const { statement, filename } = splitDocumentTitle(
      "Y24_25_Sprawozdanie jednostkowe.xhtmlJednostkowe Sprawozdanie Finansowe AB S.A.",
    );
    expect(filename).toBe("Y24_25_Sprawozdanie jednostkowe.xhtml");
    expect(statement).toBe("Jednostkowe Sprawozdanie Finansowe AB S.A.");
  });

  it("treats a filename-only title as no statement (caller substitutes generic copy)", () => {
    const { statement, filename } = splitDocumentTitle(
      "2410_Passus_2023_PSSF_MSSF_skrócone_PL-sig.pdf",
    );
    expect(statement).toBeNull();
    expect(filename).toBe("2410_Passus_2023_PSSF_MSSF_skrócone_PL-sig.pdf");
  });

  it("passes a human-only title through unchanged", () => {
    const { statement, filename } = splitDocumentTitle(
      "Skonsolidowany raport kwartalny Q2 2026",
    );
    expect(statement).toBe("Skonsolidowany raport kwartalny Q2 2026");
    expect(filename).toBeNull();
  });

  it("does not false-split a title that merely contains dots", () => {
    const { statement, filename } = splitDocumentTitle("Wyniki 2024. Podsumowanie roku S.A.");
    expect(statement).toBe("Wyniki 2024. Podsumowanie roku S.A.");
    expect(filename).toBeNull();
  });

  it("is case-insensitive and trims separator glue after the extension", () => {
    const { statement, filename } = splitDocumentTitle("RAPORT_Q3.PDF — Skrócone sprawozdanie");
    expect(filename).toBe("RAPORT_Q3.PDF");
    expect(statement).toBe("Skrócone sprawozdanie");
  });

  it("prefers the longest matching extension (.xhtml over .html/.htm)", () => {
    const { filename } = splitDocumentTitle("doc.xhtmlTytuł");
    expect(filename).toBe("doc.xhtml");
  });

  it("returns nulls for empty / missing input", () => {
    expect(splitDocumentTitle(null)).toEqual({ statement: null, filename: null });
    expect(splitDocumentTitle("   ")).toEqual({ statement: null, filename: null });
  });
});
