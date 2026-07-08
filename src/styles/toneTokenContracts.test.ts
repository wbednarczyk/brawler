import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// Enforcement (ADR 0076 D3): the semantic tone system is only sound if
//   (a) every StatusChip tone that carries meaning has a real CSS class — a tone
//       whose modifier class is never defined silently falls back to the neutral
//       base (the same class of "grey button" regression Button guards against);
//   (b) every `--tone-*` token is defined in ALL FOUR theme blocks (2 palettes ×
//       2 modes) so a component referencing a tone resolves in every theme —
//       a token missing from one block renders as an invalid/inherited color in
//       exactly that palette×mode, the light-theme-only regression this catches.
// Both are closed sets, so there is no false-positive risk.

const stylesDir = join(process.cwd(), "src", "styles");
const uiCss = readFileSync(join(stylesDir, "ui.css"), "utf8");
const themesCss = readFileSync(join(stylesDir, "themes.css"), "utf8");
const chipSource = readFileSync(join(process.cwd(), "src", "ui", "StatusChip.tsx"), "utf8");

// Pull the tone union out of StatusChip.tsx so the test tracks the real source.
function chipTones(): string[] {
  const block = chipSource.match(/StatusChipTone\s*=\s*([^;]+);/);
  expect(block, "StatusChipTone union should exist in StatusChip.tsx").not.toBeNull();
  const tones = [...block![1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
  return [...new Set(tones)].filter(Boolean);
}

// The four palette × mode blocks each anchor the tone aliases; every such block
// defines `--primary` (an anchor), so this selects exactly the theme blocks.
function themeBlocks(): string[] {
  const blocks = [...themesCss.matchAll(/\{([^}]*)\}/g)].map((m) => m[1]);
  return blocks.filter((body) => /--primary:/.test(body));
}

const TONE_TOKENS = [
  "--tone-positive",
  "--tone-caution",
  "--tone-negative",
  "--tone-neutral",
  "--tone-agent",
  "--tone-official",
  "--tone-media",
  "--tone-user",
];

describe("StatusChip tone CSS contract", () => {
  it("defines a CSS class for every meaning-bearing tone (neutral uses the base chip)", () => {
    // `neutral` deliberately has no modifier class — it is the base .ui-status-chip
    // styling, so there is nothing to fall back from.
    const missing = chipTones()
      .filter((tone) => tone !== "neutral")
      .filter((tone) => !new RegExp(`\\.ui-status-chip-${tone}\\b`).test(uiCss));
    expect(
      missing,
      `StatusChip tones without CSS (would render as the neutral base): ${missing.join(", ")}`,
    ).toEqual([]);
  });

  it("routes chip color through --tone-* tokens, never raw palette anchors", () => {
    // The toned chip rules must consume the semantic tokens, not --success/
    // --warning/--danger/--accent directly (that is what makes them theme-safe).
    const tonedChipRules = uiCss.match(/\.ui-status-chip-\w+\s*\{[^}]*\}/g) ?? [];
    expect(tonedChipRules.length, "expected toned chip rules to exist").toBeGreaterThan(0);
    for (const rule of tonedChipRules) {
      expect(rule, `toned chip rule must use a --tone-* token:\n${rule}`).toMatch(/var\(--tone-/);
      expect(rule, `toned chip rule must not use raw --success/--warning/--danger/--accent:\n${rule}`)
        .not.toMatch(/var\(--(?:success|warning|danger|accent|ok)\b/);
    }
  });
});

describe("Semantic tone token contract", () => {
  it("defines all --tone-* tokens in all four palette × mode theme blocks", () => {
    const blocks = themeBlocks();
    expect(blocks.length, "expected exactly four palette × mode theme blocks").toBe(4);
    blocks.forEach((body, index) => {
      const missing = TONE_TOKENS.filter((token) => !new RegExp(`${token}:`).test(body));
      expect(
        missing,
        `theme block #${index + 1} is missing tone tokens: ${missing.join(", ")}`,
      ).toEqual([]);
    });
  });
});
