import { describe, expect, it } from "vitest";

import inventory from "./shape-inventory.json";
import missingList from "./shape-inventory-missing.json";

/**
 * The **hard privacy boundary** around the shape inventory (epic #40 S6,
 * ADR 0091 dec. 4), enforced on the TypeScript side because this is where the
 * file lives and where a hand edit would land.
 *
 * `shape-inventory.json` is the one artefact the maintainer's real database
 * contributes to the public repo: an anonymized list of which data-state SHAPES
 * exist. Nothing in it may be traceable to a real company. The Rust scan that
 * generates it filters every value through a machine-token guard; this suite is
 * the second lock, and the only one that also covers a file someone edited by
 * hand.
 *
 * The coverage half of the contract (does the synthetic corpus reach every
 * shape?) is the Rust gate `shape_corpus::synthetic_corpus_covers_every_real_data_shape`
 * — it must run against the real read models, so it cannot live here.
 */

type Shape = { key: string; domain: string; description: string };

const shapes = inventory.shapes as Shape[];
const domains = inventory.domains as string[];
const missing = missingList.missing as Record<string, string>;

/** A shape key: lowercase kebab-case words only. No digits, no punctuation. */
const KEY = /^[a-z]+(?:-[a-z]+)*$/;

/**
 * Content markers that must never appear in a committed descriptor. Each one
 * corresponds to a real leak route: an issuer ticker or acronym (uppercase run),
 * a date or report number (digits), a URL, an id, a quoted title.
 */
const CONTENT_MARKERS: ReadonlyArray<readonly [string, RegExp]> = [
  ["an uppercase run (ticker or acronym)", /[A-Z]{2,}/],
  ["a digit (date, report number, id)", /[0-9]/],
  ["a URL", /https?:|www\.|\.pl\b|\.com\b/i],
  ["a filename extension", /\.(?:xhtml|html|htm|pdf|zip)\b/i],
  ["an id-shaped token", /\b[0-9a-f]{8,}\b/i],
  ["a quotation (verbatim content)", /["'«»„”]/],
  ["an at-sign", /@/],
];

describe("shape inventory — privacy boundary (epic #40 S6, ADR 0091 dec. 4)", () => {
  it("is non-empty and every entry is fully described", () => {
    expect(shapes.length).toBeGreaterThan(20);
    for (const shape of shapes) {
      expect(shape.key, "every shape has a key").toBeTruthy();
      expect(domains, `${shape.key} is in a known domain`).toContain(
        shape.domain,
      );
      expect(
        shape.description.length,
        `${shape.key} is described`,
      ).toBeGreaterThan(10);
    }
  });

  it("keys are kebab-case, unique, sorted, and prefixed by their domain", () => {
    const keys = shapes.map((shape) => shape.key);
    expect(new Set(keys).size, "no duplicate keys").toBe(keys.length);
    expect(keys, "sorted, so a rescan diffs cleanly").toEqual([...keys].sort());
    for (const shape of shapes) {
      expect(shape.key, `${shape.key} is kebab-case`).toMatch(KEY);
      expect(
        shape.key.startsWith(`${shape.domain}-`),
        `${shape.key} carries its domain`,
      ).toBe(true);
    }
  });

  it("carries nothing traceable to a real company", () => {
    // Descriptions are prose ABOUT a shape class, so they may contain ordinary
    // words — but never a marker of verbatim row content.
    for (const shape of shapes) {
      for (const [what, pattern] of CONTENT_MARKERS) {
        expect(pattern.test(shape.key), `${shape.key} contains ${what}`).toBe(
          false,
        );
        expect(
          pattern.test(shape.description),
          `the description of ${shape.key} contains ${what}: ${shape.description}`,
        ).toBe(false);
      }
    }
  });

  it("the accepted-gap list names inventory shapes and gives a real reason", () => {
    const keys = new Set(shapes.map((shape) => shape.key));
    for (const [key, reason] of Object.entries(missing)) {
      expect(
        keys.has(key),
        `${key} is excused but is not in the inventory`,
      ).toBe(true);
      expect(
        reason.trim().length,
        `${key} is excused without a reason`,
      ).toBeGreaterThan(20);
    }
    expect(
      Object.keys(missing).length,
      "an inventory the corpus covers almost none of is not a corpus",
    ).toBeLessThan(shapes.length / 2);
  });
});
