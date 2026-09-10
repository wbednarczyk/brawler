import { test } from "node:test";
import assert from "node:assert/strict";
import { compare, headingsOf } from "./docs-headings-ratchet.mjs";

test("headingsOf keeps ## and ### headings and ignores fenced code", () => {
  const md = "# Title\n\n## A\ntext\n### A.1\n```\n## not a heading\n```\n#### too deep\n## B ##\n## C#\n";
  assert.deepEqual(headingsOf(md), ["## A", "### A.1", "## B", "## C#"]);
});

test("a longer fence containing a shorter one stays one fence (review r2 finding 4)", () => {
  const md = "## Real\n````md\n```\n## Origin Model\n```\n````\n~~~\n## tilde fenced\n~~~\n## After\n";
  assert.deepEqual(headingsOf(md), ["## Real", "## After"]);
});

test("a heading that vanished reddens; additions and reorders do not", () => {
  const baseline = { "docs/x.md": ["## Origin Model", "## Company Event Model", "### Entitlements"] };
  const swallowed = { "docs/x.md": ["### Entitlements", "## New Section"] };
  assert.deepEqual(compare(baseline, swallowed), {
    "docs/x.md": ["## Origin Model", "## Company Event Model"],
  });
  const reordered = { "docs/x.md": ["### Entitlements", "## Company Event Model", "## Origin Model", "## Extra"] };
  assert.deepEqual(compare(baseline, reordered), {});
});
