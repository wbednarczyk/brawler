import { test, expect } from "@playwright/test";

// F0.5 (ADR 0104 dec. 2): the two design-language faces ship as committed
// variable-woff2 subsets (latin + latin-ext) — the desktop app must never
// fetch a font from the network. The oracle is the CSS-connected FontFace
// status after an explicit load with Polish text (exercises latin-ext);
// `fonts.check()` alone is spec-permitted to return true for unknown families,
// so it is never the oracle here. The weight matrix is representative, not
// exhaustive: the variable `wght` axis makes intermediates true instances.
const MATRIX = [
  { family: "Schibsted Grotesk", weights: [400, 500, 650, 800], subsets: 2 },
  { family: "JetBrains Mono", weights: [400, 600], subsets: 2 },
];

test("bundled variable fonts load Polish glyphs at authored weights", async ({ page }) => {
  await page.goto("/");

  const results = await page.evaluate(async (matrix) => {
    const out: {
      family: string;
      weight: number;
      usedFaces: number;
      allUsedLoaded: boolean;
      connectedFaces: number;
    }[] = [];
    for (const { family, weights } of matrix) {
      for (const weight of weights) {
        // `load` resolves with the faces the text actually needs — [] when the
        // family has no CSS-connected @font-face at all.
        const used = await document.fonts.load(`${weight} 13px "${family}"`, "Zażółć gęślą jaźń");
        const connected = [...document.fonts].filter(
          (f) => f.family.replace(/["']/g, "") === family,
        );
        out.push({
          family,
          weight,
          usedFaces: used.length,
          allUsedLoaded: used.length > 0 && used.every((f) => f.status === "loaded"),
          connectedFaces: connected.length,
        });
      }
    }
    return out;
  }, MATRIX);

  // Pure latin-ext probe (sol diff finding 4): every glyph here is U+0100–017F,
  // so the load can ONLY be satisfied by the latin-ext face — a bundle that
  // drops it cannot pass via the latin subset.
  const latinExt = await page.evaluate(async (families) => {
    const out: { family: string; usedFaces: number; allLoaded: boolean }[] = [];
    for (const family of families) {
      const used = await document.fonts.load(`400 13px "${family}"`, "żźćęąśłń");
      out.push({
        family,
        usedFaces: used.length,
        allLoaded: used.length > 0 && used.every((f) => f.status === "loaded"),
      });
    }
    return out;
  }, MATRIX.map((m) => m.family));
  for (const r of latinExt) {
    expect.soft(r.usedFaces, `${r.family}: latin-ext glyphs map to a face`).toBeGreaterThan(0);
    expect.soft(r.allLoaded, `${r.family}: the latin-ext face loads`).toBe(true);
  }

  for (const r of results) {
    const label = `${r.family} @ ${r.weight}`;
    expect.soft(r.usedFaces, `${label}: PL text maps to at least one @font-face`).toBeGreaterThan(0);
    expect.soft(r.allUsedLoaded, `${label}: every used face reports status=loaded`).toBe(true);
  }
  for (const { family, subsets } of MATRIX) {
    const connected = results.find((r) => r.family === family)?.connectedFaces ?? 0;
    expect
      .soft(connected, `${family}: latin + latin-ext subsets are CSS-connected`)
      .toBeGreaterThanOrEqual(subsets);
  }
});
