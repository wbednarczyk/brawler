#!/usr/bin/env node
// Retired-surface gate (ADR 0096 follow-up, docs-truth cleanup): retired
// commands, settings, targets, and workflows may not reappear in LIVE
// documentation as if they existed. docs-drift proves inventories (does a
// documented command exist?); this gate proves the inverse direction for the
// retired set — the class of rot where a doc keeps specifying a surface an ADR
// removed. Manifest: docs/retired-surface.json. Per-entry `allow` lists the
// files where a one-line retirement pointer legitimately names the token;
// history homes (ADRs, kanban-archive, bad-ideas, retros, completed plans,
// CHANGELOG) are excluded from the scan entirely.

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { pathToFileURL } from "node:url";

// Directories/files that legitimately carry history — never scanned.
const EXCLUDED = [
  "docs/adr",
  "docs/retros",
  "docs/kanban-archive.md",
  "docs/bad-ideas.md",
  "docs/mockups",
];
// docs/plans: completed plans are archaeology (see docs/plans/README.md);
// only the README itself is live guidance.
const PLANS_DIR = "docs/plans";
const PLANS_LIVE = ["docs/plans/README.md"];

export function isScanned(rel) {
  const p = rel.split(sep).join("/");
  if (!p.endsWith(".md")) return false;
  if (!(p.startsWith("docs/") || p.startsWith("wiki/"))) return false;
  if (EXCLUDED.some((e) => p === e || p.startsWith(`${e}/`))) return false;
  if (p.startsWith(`${PLANS_DIR}/`)) return PLANS_LIVE.includes(p);
  return true;
}

function listMarkdown(root, dir, out) {
  let entries;
  try {
    entries = readdirSync(join(root, dir));
  } catch {
    return out;
  }
  for (const name of entries) {
    const rel = dir ? `${dir}/${name}` : name;
    const full = join(root, rel);
    if (statSync(full).isDirectory()) listMarkdown(root, rel, out);
    else if (isScanned(rel)) out.push(rel);
  }
  return out;
}

// Identifier boundary: the token must not be embedded in a larger identifier.
// `-` counts as an identifier character so `check-epic` never matches inside a
// legitimately distinct hyphenated name like `check-epic-v2`.
const boundary = (ch) => ch === undefined || !/[A-Za-z0-9_-]/.test(ch);

export function findHits(content, token) {
  const hits = [];
  const lines = content.split("\n");
  for (let i = 0; i < lines.length; i++) {
    let idx = lines[i].indexOf(token);
    while (idx !== -1) {
      if (boundary(lines[i][idx - 1]) && boundary(lines[i][idx + token.length])) {
        hits.push({ line: i + 1, text: lines[i] });
        break;
      }
      idx = lines[i].indexOf(token, idx + 1);
    }
  }
  return hits;
}

// A line in an `allow`-listed file passes ONLY when it is pointer-shaped —
// it visibly talks about the retirement rather than specifying live behavior.
// A whole-file exemption is `allowFile` (reserved for docs whose subject IS
// the retired surface, e.g. a stored-state auditor's methodology).
// LIMITATION (deliberate): this is a lexical tripwire, not semantic proof — a
// line can carry a retirement word and still specify live behavior. The
// backstop is review: every `allow`/`allowFile` change ships in a reviewed
// diff, and the gate's job is to force that conversation, not to win it.
const POINTER_RE = /retir|dropp|remov|supersed|legacy|historic|delet|renam|no longer/i;
export const isPointerLine = (text) => POINTER_RE.test(text);

export function scan(root, manifest) {
  const files = [];
  listMarkdown(root, "docs", files);
  listMarkdown(root, "wiki", files);
  const violations = [];
  for (const rel of files) {
    const content = readFileSync(join(root, rel), "utf8");
    for (const entry of manifest.retired) {
      if ((entry.allowFile ?? []).includes(rel)) continue;
      const pointerOk = (entry.allow ?? []).includes(rel);
      for (const hit of findHits(content, entry.token)) {
        if (pointerOk && isPointerLine(hit.text)) continue;
        violations.push({ file: rel, line: hit.line, token: entry.token, adr: entry.adr });
      }
    }
  }
  return violations;
}

function main() {
  const root = process.env.RETIRED_SURFACE_ROOT || ".";
  const manifestPath =
    process.env.RETIRED_SURFACE_MANIFEST || "docs/retired-surface.json";
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(join(root, manifestPath), "utf8"));
  } catch (err) {
    console.error(`retired-surface: cannot read ${manifestPath}: ${err.message}`);
    process.exit(2);
  }
  if (!Array.isArray(manifest.retired) || manifest.retired.length === 0) {
    console.error(`retired-surface: ${manifestPath} has no \`retired\` entries.`);
    process.exit(2);
  }
  const violations = scan(root, manifest);
  if (violations.length > 0) {
    console.error(`✖ retired-surface: ${violations.length} live-doc reference(s) to retired surface:`);
    for (const v of violations) {
      console.error(`  ${v.file}:${v.line} mentions \`${v.token}\` (retired — ${v.adr}).`);
    }
    console.error(
      `  A live doc must describe the present. A deliberate retirement pointer needs BOTH the file in\n` +
        `  that token's \`allow\` list (docs/retired-surface.json) AND retirement wording on the line itself;\n` +
        `  \`allowFile\` (whole-file) is reserved for docs whose subject IS the retired surface.`,
    );
    process.exit(1);
  }
  console.log(`✓ retired-surface: no live doc references retired surface (${manifest.retired.length} tokens).`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
