import { describe, expect, it } from "vitest";

// Test hygiene ratchet (G10, docs/testing.md § Frontend test
// responsibilities): a `it()`/`test()` block with no assertion silently
// "passes" no matter what the code does — the runner's green checkmark
// proves nothing. Two escape hatches for genuine exceptions: the title ends
// with "renders"/"does not throw" (the crash-if-broken IS the assertion), or
// a `// no-assert-ok: <reason>` comment on the line before the block. New
// offenders redden immediately; the baseline (like coverage-baseline.json)
// only shrinks.

const sourceModules = import.meta.glob("/src/**/*.test.{ts,tsx}", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const baselineModule = import.meta.glob("/src/test/testHygiene.baseline.json", {
  import: "default",
  eager: true,
});
const baseline = new Set(Object.values(baselineModule)[0] as string[]);

type Block = { title: string; line: number; body: string; prevLine: string };

// ponytail: a brace-depth scan over the raw source, not a real parser — good
// enough for a hygiene lint. Ceiling: a title/string containing unbalanced
// parens would throw the depth count off. Upgrade to an AST walk (the
// project already depends on typescript-eslint) if that ever bites.

// Returns the index of the `)` matching the `(` at `openParenIdx` — or
// `src.length` if depth never returns to 0 (an unbalanced paren inside a
// title/regex literal throws the count off, the documented ceiling above;
// falling through to EOF rather than a sentinel keeps a real assertion
// further down still inside the slice, matching this scan's original
// tolerant behavior).
function matchingParen(src: string, openParenIdx: number): number {
  let depth = 0;
  let i = openParenIdx;
  for (; i < src.length; i++) {
    if (src[i] === "(") depth++;
    else if (src[i] === ")") {
      depth--;
      if (depth === 0) break;
    }
  }
  return i;
}

function findTestBlocks(src: string): Block[] {
  const results: Block[] = [];
  const callRe = /\b(?:it|test)\(\s*(["'`])((?:\\.|(?!\1).)*)\1/g;
  let match: RegExpExecArray | null;
  while ((match = callRe.exec(src))) {
    const title = match[2];
    const startIdx = match.index;
    const openParenIdx = src.indexOf("(", startIdx);
    const closeParenIdx = matchingParen(src, openParenIdx);
    const before = src.slice(0, startIdx);
    const lines = before.split("\n");
    results.push({
      title,
      line: lines.length,
      body: src.slice(openParenIdx, closeParenIdx + 1),
      prevLine: lines[lines.length - 2] ?? "",
    });
  }

  // it.each([...])("title", fn) / test.each([...])("title", fn): no literal
  // `it(`/`test(` token exists — the table call and the title+fn call are two
  // separate `(…)` groups. A nested `it()`/`test()` inside a `describe.each`
  // callback still matches the plain scan above, so only `.each` on it/test
  // itself needs handling here.
  const eachCallRe = /\b(?:it|test)\.each(?:\.\w+)?\(/g;
  while ((match = eachCallRe.exec(src))) {
    const tableOpenIdx = match.index + match[0].length - 1;
    const tableCloseIdx = matchingParen(src, tableOpenIdx);
    let invOpenIdx = tableCloseIdx + 1;
    while (invOpenIdx < src.length && /\s/.test(src[invOpenIdx])) invOpenIdx++;
    if (src[invOpenIdx] !== "(") continue;
    const titleMatch = src.slice(invOpenIdx).match(/^\(\s*(["'`])((?:\\.|(?!\1).)*)\1/);
    if (!titleMatch) continue;
    const invCloseIdx = matchingParen(src, invOpenIdx);
    const before = src.slice(0, match.index);
    const lines = before.split("\n");
    results.push({
      title: titleMatch[2],
      line: lines.length,
      body: src.slice(invOpenIdx, invCloseIdx + 1),
      prevLine: lines[lines.length - 2] ?? "",
    });
  }

  return results;
}

// Matches a literal `expect(`/`expectTypeOf(` as well as this codebase's
// established `expectXxx(...)` custom-assertion-helper convention
// (expectSinglePrimary, expectNoPageOverflow, …) so a test that asserts
// through a named helper isn't flagged as assertion-less.
function hasAssertion(body: string): boolean {
  return /\bexpect\w*\s*\(/.test(body) || /\bassert\w*\s*[(.]/.test(body);
}

function isExemptTitle(title: string): boolean {
  return /\b(does not throw|renders)$/i.test(title.trim());
}

function isExemptComment(prevLine: string): boolean {
  return /\/\/\s*no-assert-ok:/.test(prevLine);
}

function findOffenders(): string[] {
  const offenders: string[] = [];
  for (const [path, src] of Object.entries(sourceModules)) {
    for (const block of findTestBlocks(src)) {
      if (hasAssertion(block.body) || isExemptTitle(block.title) || isExemptComment(block.prevLine)) continue;
      offenders.push(`${path}:${block.line} — "${block.title}"`);
    }
  }
  return offenders;
}

// B1 (owner-approved hard gate 2026-09-07): the baseline started empty and
// is not a way for a NEW assertion-less block to go green — the only fix is
// to add an assertion, an exempt title, or a `// no-assert-ok:` comment.
// This ceiling may only be LOWERED, never raised to fit a new offender.
const TEST_HYGIENE_BASELINE_CEILING = 0;

describe("test hygiene — every it()/test() block asserts something", () => {
  const offenders = findOffenders();

  it("the baseline ceiling only shrinks, never grows", () => {
    expect(
      baseline.size,
      `testHygiene.baseline.json has ${baseline.size} entries, above ` +
        `TEST_HYGIENE_BASELINE_CEILING (${TEST_HYGIENE_BASELINE_CEILING}) in testHygiene.test.ts. ` +
        "The ceiling may only be lowered, never raised — a new assertion-less block must be fixed " +
        "(assertion, exempt title, or a documented no-assert-ok), never baselined to go green.",
    ).toBeLessThanOrEqual(TEST_HYGIENE_BASELINE_CEILING);
  });

  it("has no NEW assertion-less test blocks beyond the baseline", () => {
    const newOffenders = offenders.filter((entry) => !baseline.has(entry));
    expect(
      newOffenders,
      `Test block(s) with no expect*/assert* call, no "renders"/"does not throw" title, and no ` +
        `// no-assert-ok comment — file:line:\n${newOffenders.join("\n")}`,
    ).toEqual([]);
  });

  it("keeps the baseline honest (no stale entries for tests that now assert or moved)", () => {
    const offenderSet = new Set(offenders);
    const stale = [...baseline].filter((entry) => !offenderSet.has(entry)).sort();
    expect(
      stale,
      `Baseline entries that no longer apply (test now asserts, or the file:line shifted) — remove ` +
        `them from testHygiene.baseline.json:\n${stale.join("\n")}`,
    ).toEqual([]);
  });
});
