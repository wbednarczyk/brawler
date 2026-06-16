import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// Enforcement (ADR 0037): every Button variant must have real CSS. A variant whose class is
// never defined falls back to the browser's default grey button — the exact "grey buttons"
// regression. This is a closed set (7 variants), so there is no false-positive risk.

const buttonSource = readFileSync(join(process.cwd(), "src", "ui", "Button.tsx"), "utf8");
const css = ["ui.css", "controls.css", "tokens.css"]
  .map((file) => readFileSync(join(process.cwd(), "src", "styles", file), "utf8"))
  .join("\n");

// Pull the variant -> className map out of Button.tsx so the test tracks the real source.
function variantClasses(): string[] {
  const block = buttonSource.match(/variantClassName[^{]*\{([\s\S]*?)\}/);
  expect(block, "variantClassName map should exist in Button.tsx").not.toBeNull();
  const classes = [...block![1].matchAll(/:\s*"([^"]+)"/g)].flatMap((m) => m[1].split(/\s+/));
  return [...new Set(classes)].filter(Boolean);
}

describe("Button variant CSS contract", () => {
  it("defines CSS for every Button variant class", () => {
    const missing = variantClasses().filter(
      (cls) => !new RegExp(`\\.${cls}\\b`).test(css),
    );
    expect(
      missing,
      `Button variants without CSS (would render as default grey buttons): ${missing.join(", ")}`,
    ).toEqual([]);
  });
});
