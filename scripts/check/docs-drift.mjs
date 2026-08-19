#!/usr/bin/env node
// Spec <-> code drift gate (ADR 0065). Verifies the machine-checkable subset of
// "the spec matches the code" as a hard-fail step of `make check`, and honors
// the planned-section convention (`Status: planned (vX.Y.Z, ADR NNNN)`) so a
// spec section describing not-yet-built scope does not read as a doc→code miss.
//
// Checks (all hard-fail; see docs/adr/0065-spec-code-drift-gates.md):
//   1. Commands, doc -> code: every command name documented in contracts.md
//      exists as a `#[tauri::command]` fn in src-tauri/src, unless its section
//      is tagged `Status: planned (...)` — in which case a command that DOES
//      exist in code fails instead ("planned section delivered").
//   2. Commands, code -> doc: every `#[tauri::command]` fn name is mentioned
//      somewhere in contracts.md (word-boundary, backticked or plain).
//   3. Screens: every nav destination in src/app/navigation.ts is represented
//      in docs/ui-information-architecture.md, either as a `##`/`###` heading
//      (via a structural alias in STRUCTURAL_MAPPINGS) or as a plain
//      case-insensitive word-boundary mention anywhere in the file.
//   4. Settings keys: every settings KV key read/written in
//      src-tauri/src/storage/settings.rs is documented in data-model.md's
//      `### Settings` section.
//   5. ADR hygiene: every docs/adr/NNNN-*.md carries a `Status:` line in its
//      first 5 non-empty lines; docs/adr/INDEX.md stays in sync with a
//      generated index (regenerate: `node scripts/check/docs-drift.mjs
//      --write-adr-index`).
//   6. MCP catalog drift (ADR 0088 dec. 5; content-compare hardened epic #285
//      T10): the generated tool-catalog sections in wiki/mcp-agent-guide.md
//      and .claude/skills/brawler-mcp/SKILL.md are compared BYTE-FOR-BYTE
//      against a freshly regenerated block (names AND descriptions) — a
//      name-set-only compare let a hand-edited description, or a hand-added
//      line inside the do-not-edit span, drift silently while still passing.
//      The sections are machine-generated (name + description from the
//      tools/list snapshot, split read vs act by the REGISTRY-TRUTH capability
//      manifest — never a tool-name regex, #387) and must never be
//      hand-edited; regenerate with
//      `node scripts/check/docs-drift.mjs --write-mcp-catalog`.
//   7. MCP manifest/snapshot coherence (#387): the capability manifest, the
//      full tools/list snapshot, and the acquisition-scoped snapshot stay
//      mutually consistent (order + deep-equal projection), and each doc's
//      hand-authored acquisition-workflow inventory covers exactly the ten
//      acquisition tools (nine since #386, +propose_kpi_definition #399 S4).
//
// Extraction heuristics (contracts.md documents commands two ways — both are
// captured):
//   A. Bulleted lists under a lead-in line that mentions "command"/"commands"
//      and ends with `:` (covers `Commands:`, `Local commands:`,
//      `Typed commands:`, `Initial local commands:`, `Initial Tauri command
//      groups:`, etc.) — collected until the bulleted block ends.
//   B. Standalone paragraph definitions: a line starting with a backtick-
//      wrapped, all-lowercase-snake_case name immediately followed by `(` or
//      the closing backtick (e.g. "`delete_company(companyId)` deletes...").
//      CamelCase settings-field mentions (e.g. `` `pinnedCompanyIds` ``) do not
//      match — the character class is deliberately lowercase-only.
// Sanity floor: fewer than 150 extracted command names means the heuristic
// itself broke silently — fail loud instead of passing vacuously.

import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, join, resolve } from "node:path";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const errors = [];

// A screen documented under a different label than its nav `Section` value.
// Structural patterns only (never a per-name silencer for a genuinely missing
// screen) — see ADR 0065 Decision 3.
const STRUCTURAL_MAPPINGS = {
  Today: "Today / Pulse",
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function readText(relPath) {
  return readFileSync(resolve(repoRoot, relPath), "utf8");
}

function walkRustFiles(dir) {
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...walkRustFiles(full));
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      out.push(full);
    }
  }
  return out;
}

// ---------------------------------------------------------------------------
// 1/2. Commands: extract from contracts.md, extract from src-tauri, compare.
// ---------------------------------------------------------------------------

function extractSections(lines) {
  // Every ## or ### heading, in order, as { line: 0-based index, level, title }.
  const sections = [];
  const headingRe = /^(#{2,3})\s+(.*)/;
  for (let i = 0; i < lines.length; i++) {
    const m = headingRe.exec(lines[i]);
    if (m) sections.push({ line: i, level: m[1].length, title: m[2].trim() });
  }
  return sections;
}

function sectionForLine(sections, idx) {
  let cur = null;
  for (const s of sections) {
    if (s.line <= idx) cur = s;
    else break;
  }
  return cur;
}

function sectionIsPlanned(lines, section) {
  if (!section) return false;
  const start = section.line;
  let seen = 0;
  for (let i = start; i < lines.length && seen < 6; i++) {
    const t = lines[i].trim();
    if (t === "") continue;
    seen++;
    if (/^Status:\s*planned\s*\(/i.test(t)) return true;
  }
  return false;
}

function extractContractsCommands(contractsPath) {
  const text = readText(contractsPath);
  const lines = text.split("\n");
  const sections = extractSections(lines);

  // name -> { lines: Set<number>, sections: Set<Section> }
  const found = new Map();
  function record(name, lineIdx) {
    const section = sectionForLine(sections, lineIdx);
    if (!found.has(name)) found.set(name, { lines: new Set(), sections: new Set() });
    const entry = found.get(name);
    entry.lines.add(lineIdx + 1);
    if (section) entry.sections.add(section);
  }

  const bulletNameRe = /^\s*-\s*`([a-z][a-z0-9_]*)/;
  const leadInRe = /command/i;

  // Pattern A: bulleted lists under a "...command(s)...:" lead-in line.
  for (let i = 0; i < lines.length; i++) {
    const trimmed = lines[i].trim();
    if (!trimmed.endsWith(":") || !leadInRe.test(trimmed)) continue;
    let j = i + 1;
    while (j < lines.length) {
      const cur = lines[j];
      if (cur.trim() === "") {
        let k = j + 1;
        while (k < lines.length && lines[k].trim() === "") k++;
        if (k < lines.length && /^\s*-\s/.test(lines[k])) {
          j = k;
          continue;
        }
        break;
      }
      if (/^\s*-\s/.test(cur)) {
        // Only a TOP-LEVEL bullet declares a command. Indented sub-bullets under
        // a command entry describe DTO fields/cells (e.g. "  - `canonical` is
        // `true`…") and repeatedly false-flagged as documented-but-unimplemented
        // commands (T2.1 `report:`/`facts:`, Panel B `canonical`; 2026-07-09).
        // The code→doc direction (every #[tauri::command] must be documented)
        // is a separate pass and is unaffected by skipping nested bullets here.
        if (/^-\s/.test(cur)) {
          const m = bulletNameRe.exec(cur);
          if (m) record(m[1], j);
        }
        j++;
        continue;
      }
      break;
    }
  }

  // Pattern B: standalone paragraph command definitions, e.g.
  // "`delete_company(companyId)` deletes a company...". Lowercase-only
  // character class deliberately excludes camelCase settings-field mentions.
  const standaloneRe = /^`([a-z][a-z0-9_]*)([(`])/;
  for (let i = 0; i < lines.length; i++) {
    const m = standaloneRe.exec(lines[i]);
    if (!m) continue;
    // Backtick-terminated (no-parens) form additionally requires an underscore:
    // all-lowercase single-word payload FIELDS are documented paragraph-first
    // too (`severity` ∈ ...) and would otherwise be misread as commands, while
    // real commands are multi-word snake_case — except `health`/`search`,
    // which Pattern A bullets already cover (harvest 2026-07-27).
    if (m[2] === "`" && !m[1].includes("_")) continue;
    record(m[1], i);
  }

  return { found, sections };
}

function extractTauriCommands(srcDir) {
  const names = new Map(); // name -> file path (first occurrence)
  const fnRe = /\bfn\s+([a-zA-Z_][a-zA-Z0-9_]*)/;
  for (const file of walkRustFiles(srcDir)) {
    const lines = readFileSync(file, "utf8").split("\n");
    for (let i = 0; i < lines.length; i++) {
      if (!lines[i].includes("tauri::command")) continue;
      for (let j = i + 1; j < Math.min(i + 8, lines.length); j++) {
        const m = fnRe.exec(lines[j]);
        if (m) {
          if (!names.has(m[1])) names.set(m[1], file);
          break;
        }
      }
    }
  }
  return names;
}

function checkCommands() {
  const contractsRel = "docs/contracts.md";
  const contractsText = readText(contractsRel);
  const contractsLines = contractsText.split("\n");
  const { found: docCommands } = extractContractsCommands(contractsRel);
  const codeCommands = extractTauriCommands(resolve(repoRoot, "src-tauri/src"));

  if (docCommands.size < 150) {
    errors.push(
      `docs-drift: only extracted ${docCommands.size} command names from ${contractsRel} (floor: 150).\n` +
        "    The extraction heuristic likely broke — inspect contracts.md's command-list formatting" +
        " before trusting this gate again.",
    );
  }

  // Doc -> code, respecting the planned-section convention.
  for (const [name, entry] of docCommands) {
    const inCode = codeCommands.has(name);
    const plannedSections = [...entry.sections].filter((s) => sectionIsPlanned(contractsLines, s));
    if (plannedSections.length > 0) {
      if (inCode) {
        errors.push(
          `docs-drift: \`${name}\` is documented under a planned section ` +
            `("${plannedSections[0].title}", ${contractsRel}:${[...entry.lines][0]}) but ALREADY EXISTS in code` +
            ` (${codeCommands.get(name)}).\n` +
            "    planned section delivered (or partially) — remove/resolve the planned tag and finish the spec.",
        );
      }
      continue;
    }
    if (!inCode) {
      errors.push(
        `docs-drift: \`${name}\` is documented in ${contractsRel}:${[...entry.lines][0]} but no ` +
          "`#[tauri::command]` fn with that name exists in src-tauri/src.\n" +
          "    Fix the doc (rename/remove) or the code — never delete the check.",
      );
    }
  }

  // Code -> doc: every real command mentioned somewhere in contracts.md.
  for (const [name, file] of codeCommands) {
    const wordRe = new RegExp(`\\b${name}\\b`);
    if (!wordRe.test(contractsText)) {
      errors.push(
        `docs-drift: \`#[tauri::command] fn ${name}\` (${file}) is not mentioned anywhere in ${contractsRel}.`,
      );
    }
  }
}

// ---------------------------------------------------------------------------
// 3. Screens: nav destinations vs ui-information-architecture.md
// ---------------------------------------------------------------------------

function extractNavDestinations(navPath) {
  const text = readText(navPath);
  // navGroups: [{ id, items: [{ label: "X", ... }, ...] }, ...] — pull every
  // `label: "X"` inside the navGroups array literal (the reachable destinations;
  // deep-link-only `Section` values not present in navGroups are out of scope).
  const groupsMatch = /export const navGroups[^=]*=\s*(\[[\s\S]*?\n\];)/.exec(text);
  const body = groupsMatch ? groupsMatch[1] : text;
  const labelRe = /label:\s*"([A-Za-z]+)"/g;
  const out = new Set();
  let m;
  while ((m = labelRe.exec(body))) out.add(m[1]);
  return out;
}

function checkScreens() {
  const navPath = "src/app/navigation.ts";
  const iaPath = "docs/ui-information-architecture.md";
  const destinations = extractNavDestinations(navPath);
  if (destinations.size === 0) {
    errors.push(`docs-drift: extracted 0 nav destinations from ${navPath} — the parser heuristic likely broke.`);
    return;
  }
  const iaText = readText(iaPath);
  const headingRe = /^#{2,3}\s+(.*)$/gm;
  const headings = [];
  let hm;
  while ((hm = headingRe.exec(iaText))) headings.push(hm[1].trim());

  for (const dest of destinations) {
    const alias = STRUCTURAL_MAPPINGS[dest] ?? dest;
    const headingHit = headings.some((h) => h.toLowerCase().includes(alias.toLowerCase()));
    const wordRe = new RegExp(`\\b${alias.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\b`, "i");
    const wordHit = wordRe.test(iaText);
    if (!headingHit && !wordHit) {
      errors.push(
        `docs-drift: nav destination "${dest}" (${navPath}) is not represented in ${iaPath}` +
          " (no matching heading and no word-boundary mention).\n" +
          "    Add a section, or a STRUCTURAL_MAPPINGS alias if it is documented under a different name.",
      );
    }
  }
}

// ---------------------------------------------------------------------------
// 4. Settings keys: settings.rs vs data-model.md § Settings
// ---------------------------------------------------------------------------

function extractSettingsKeys(settingsPath) {
  const text = readText(settingsPath);
  const names = new Set();
  const re1 = /setting_\w+\(\s*connection,\s*"([a-z0-9_]+)"/g;
  const re2 = /(?:upsert_setting|update_setting)\(\s*connection,\s*"([a-z0-9_]+)"/g;
  let m;
  while ((m = re1.exec(text))) names.add(m[1]);
  while ((m = re2.exec(text))) names.add(m[1]);
  return names;
}

function extractDataModelSettingsSection(dataModelPath) {
  const text = readText(dataModelPath);
  const lines = text.split("\n");
  const headingIdx = lines.findIndex((l) => /^###\s+Settings\s*$/.test(l.trim()));
  if (headingIdx === -1) return null;
  const out = [];
  for (let i = headingIdx + 1; i < lines.length; i++) {
    if (/^#{2,3}\s+/.test(lines[i])) break;
    out.push(lines[i]);
  }
  return out.join("\n");
}

function checkSettingsKeys() {
  const settingsPath = "src-tauri/src/storage/settings.rs";
  const dataModelPath = "docs/data-model.md";
  const keys = extractSettingsKeys(settingsPath);
  if (keys.size < 8) {
    errors.push(`docs-drift: only extracted ${keys.size} settings keys from ${settingsPath} (floor: 8).`);
    return;
  }
  const section = extractDataModelSettingsSection(dataModelPath);
  if (section === null) {
    errors.push(`docs-drift: ${dataModelPath} has no "### Settings" section to check settings keys against.`);
    return;
  }
  for (const key of keys) {
    const wordRe = new RegExp(`\\b${key}\\b`);
    if (!wordRe.test(section)) {
      errors.push(
        `docs-drift: settings key \`${key}\` (${settingsPath}) is not documented in ${dataModelPath}'s` +
          " \"### Settings\" section.",
      );
    }
  }
}

// ---------------------------------------------------------------------------
// 5. ADR hygiene: Status: line + INDEX.md
// ---------------------------------------------------------------------------

function listAdrFiles() {
  const adrDir = resolve(repoRoot, "docs/adr");
  return readdirSync(adrDir)
    .filter((f) => /^\d{4}-.*\.md$/.test(f))
    .sort();
}

function firstNonEmptyLines(text, n) {
  const out = [];
  for (const line of text.split("\n")) {
    if (line.trim() === "") continue;
    out.push(line.trim());
    if (out.length >= n) break;
  }
  return out;
}

function truncateStatus(statusLine) {
  // "Status: <text>" -> "<text>", truncated at the first sentence/em-dash, max ~80 chars.
  // Markdown links are flattened to their text FIRST — a blind slice can cut a
  // link mid-destination and render the whole INDEX row broken.
  let text = statusLine.replace(/^Status:\s*/, "").replace(/\[([^\]]*)\]\([^)]*\)/g, "$1");
  const cutRe = /(\. |\.$| — |—)/;
  const cm = cutRe.exec(text);
  if (cm && cm.index > 0) text = text.slice(0, cm.index);
  if (text.length > 80) text = text.slice(0, 80).trimEnd();
  return text.trim();
}

function buildAdrIndex() {
  const files = listAdrFiles();
  const rows = [];
  for (const file of files) {
    const num = file.slice(0, 4);
    const text = readText(`docs/adr/${file}`);
    const firstLines = firstNonEmptyLines(text, 5);
    const h1 = firstLines.find((l) => l.startsWith("# ADR"));
    const title = h1 ? h1.replace(/^#\s*ADR\s*\d+:\s*/, "") : file;
    const statusLine = firstLines.find((l) => /^Status:/.test(l));
    const status = statusLine ? truncateStatus(statusLine) : "(missing Status)";
    rows.push(`- [${num}](${file}) — ${title} — ${status}`);
  }
  const header = "<!-- generated by docs-drift — do not edit -->\n# ADR Index\n\n";
  return header + rows.join("\n") + "\n";
}

function checkAdrHygiene(writeIndex) {
  for (const file of listAdrFiles()) {
    const text = readText(`docs/adr/${file}`);
    const firstLines = firstNonEmptyLines(text, 5);
    if (!firstLines.some((l) => /^Status:/.test(l))) {
      errors.push(`docs-drift: docs/adr/${file} has no "Status:" line in its first 5 non-empty lines.`);
    }
  }

  const generated = buildAdrIndex();
  const indexPath = resolve(repoRoot, "docs/adr/INDEX.md");
  if (writeIndex) {
    writeFileSync(indexPath, generated);
    console.log("✓ docs-drift: wrote docs/adr/INDEX.md");
    return;
  }
  let existing = null;
  try {
    existing = readFileSync(indexPath, "utf8");
  } catch {
    existing = null;
  }
  if (existing !== generated) {
    errors.push(
      "docs-drift: docs/adr/INDEX.md is stale or missing — run `node scripts/check/docs-drift.mjs --write-adr-index`.",
    );
  }
}

// ---------------------------------------------------------------------------
// 6. MCP catalog: generated tool tables vs the tools/list snapshot (ADR 0088).
// ---------------------------------------------------------------------------

const MCP_SNAPSHOT_REL =
  "src-tauri/src/mcp/snapshots/brawler_lib__mcp__protocol__tests__tools_list_schema.snap";
// The acquisition-scoped tools/list snapshot and the registry-truth capability
// manifest (#387, ADR 0065): the manifest is the ONLY artifact carrying each
// tool's authoritative `tier` (the wire `tools/list` shape omits it), so the
// catalog split reads it instead of guessing read/act from the tool name.
const MCP_ACQ_SNAPSHOT_REL =
  "src-tauri/src/mcp/snapshots/brawler_lib__mcp__protocol__tests__tools_list_schema_acquisition.snap";
const MCP_MANIFEST_REL =
  "src-tauri/src/mcp/snapshots/brawler_lib__mcp__registry__tests__mcp_registry_manifest.snap";
const MCP_CATALOG_BEGIN = "<!-- BEGIN GENERATED MCP CATALOG";
const MCP_CATALOG_END = "<!-- END GENERATED MCP CATALOG -->";
// The hand-authored acquisition-workflow tool inventory (#387): the rewritten
// playbook lists the ten workflow tools between these markers; the gate parses
// the span and asserts its unique name set equals the manifest's acquisition
// set (operational sequencing in the surrounding prose is tested separately).
const ACQ_WORKFLOW_BEGIN = "<!-- BEGIN ACQUISITION WORKFLOW TOOLS -->";
const ACQ_WORKFLOW_END = "<!-- END ACQUISITION WORKFLOW TOOLS -->";
// Every file that must carry an in-sync generated catalog section.
const MCP_CATALOG_TARGETS = ["wiki/mcp-agent-guide.md", ".claude/skills/brawler-mcp/SKILL.md"];

function escapeRe(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

const MCP_CATALOG_SPAN_RE = new RegExp(
  `${escapeRe(MCP_CATALOG_BEGIN)}[\\s\\S]*?${escapeRe(MCP_CATALOG_END)}`,
);
const ACQ_WORKFLOW_SPAN_RE = new RegExp(
  `${escapeRe(ACQ_WORKFLOW_BEGIN)}[\\s\\S]*?${escapeRe(ACQ_WORKFLOW_END)}`,
);

// Both the snapshot files and the manifest are insta snapshots: a `---`-delimited
// YAML header (no braces) followed by a pretty-printed JSON body. The JSON is
// everything from the first `{`.
function readSnapshotJson(relPath) {
  const text = readText(relPath);
  const jsonStart = text.indexOf("{");
  if (jsonStart === -1) throw new Error(`no JSON body found in ${relPath}`);
  return JSON.parse(text.slice(jsonStart));
}

function readMcpSnapshotTools() {
  const tools = readSnapshotJson(MCP_SNAPSHOT_REL)?.result?.tools;
  if (!Array.isArray(tools)) throw new Error("result.tools is not an array");
  return tools.map((t) => ({ name: String(t.name), description: String(t.description ?? "") }));
}

// The registry-truth manifest: [{name, tier: "read"|"act", acquisition}] in
// registry order. Validates schema, name uniqueness, and the tier vocabulary,
// then returns the ordered list plus a name->entry map (#387). Pure — exported
// for the negative test.
export function parseManifestTools(tools) {
  if (!Array.isArray(tools)) throw new Error("manifest.tools is not an array");
  const byName = new Map();
  for (const entry of tools) {
    if (
      typeof entry?.name !== "string" ||
      (entry.tier !== "read" && entry.tier !== "act") ||
      typeof entry.acquisition !== "boolean"
    ) {
      throw new Error(`manifest entry malformed: ${JSON.stringify(entry)}`);
    }
    if (byName.has(entry.name)) throw new Error(`manifest duplicate tool: ${entry.name}`);
    byName.set(entry.name, entry);
  }
  return { tools, byName };
}

function readMcpManifest() {
  return parseManifestTools(readSnapshotJson(MCP_MANIFEST_REL)?.tools);
}

// The pure manifest/snapshot coherence assertions (#387) — no IO, returns a
// list of error strings. `inventories` is [{rel, names: Set|null}] (null =
// markers absent). Exported for the negative test.
export function manifestCoherenceErrors(manifest, full, acquisition, inventories) {
  const out = [];

  // (a) full tools/list names in EXACTLY the manifest's order.
  const manifestNames = manifest.tools.map((t) => t.name);
  const fullNames = full.map((t) => String(t.name));
  if (JSON.stringify(fullNames) !== JSON.stringify(manifestNames)) {
    out.push(
      "docs-drift: the capability manifest is out of sync with the full tools/list snapshot" +
        " (name set or order differs).\n    regenerate the manifest: cargo insta accept" +
        " (after `cargo nextest run capability_manifest_is_the_frozen_contract`).",
    );
  }

  // (b) acquisition tools/list = ordered deep-equal projection of the full
  //     surface filtered to the manifest's acquisition tools.
  const acqExpected = full.filter((t) => manifest.byName.get(String(t.name))?.acquisition === true);
  if (JSON.stringify(acquisition) !== JSON.stringify(acqExpected)) {
    out.push(
      "docs-drift: the scoped acquisition tools/list is not the ordered projection of the full" +
        " surface (KPI_ACQUISITION_TOOLS ∩ tools/list). Regenerate the scoped snapshot" +
        " (cargo insta accept) after a deliberate surface change.",
    );
  }

  // (c) each doc's workflow inventory covers exactly the acquisition set.
  const acqSet = new Set(manifest.tools.filter((t) => t.acquisition).map((t) => t.name));
  for (const { rel, names } of inventories) {
    if (names === null) {
      out.push(
        `docs-drift: ${rel} has no acquisition-workflow markers (\`${ACQ_WORKFLOW_BEGIN}\` / \`${ACQ_WORKFLOW_END}\`).`,
      );
      continue;
    }
    const missing = [...acqSet].filter((n) => !names.has(n)).sort();
    const extra = [...names].filter((n) => !acqSet.has(n)).sort();
    if (missing.length || extra.length) {
      out.push(
        `docs-drift: ${rel} acquisition-workflow inventory != the ten acquisition tools.` +
          (missing.length ? `\n    missing: ${missing.join(", ")}` : "") +
          (extra.length ? `\n    extra: ${extra.join(", ")}` : ""),
      );
    }
  }

  return out;
}

// Extract the backticked tool names inside a doc's ACQUISITION WORKFLOW span;
// null when the markers are absent. Pure — exported for the negative test.
export function extractWorkflowInventoryNames(fileText) {
  const span = ACQ_WORKFLOW_SPAN_RE.exec(fileText);
  if (!span) return null;
  const names = new Set();
  const nameRe = /`([a-z0-9_]+)`/g;
  let m;
  while ((m = nameRe.exec(span[0]))) names.add(m[1]);
  return names;
}

export function buildMcpCatalogBlock(tools, manifest) {
  const cell = (s) => s.replace(/\s+/g, " ").replace(/\|/g, "\\|").trim();
  const table = (rows) =>
    [
      "| Tool | What it does |",
      "| --- | --- |",
      ...rows.map((t) => `| \`${t.name}\` | ${cell(t.description)} |`),
    ].join("\n");
  const tierOf = (name) => manifest.byName.get(name)?.tier;
  const reads = tools.filter((t) => tierOf(t.name) === "read");
  const acts = tools.filter((t) => tierOf(t.name) === "act");
  return [
    `${MCP_CATALOG_BEGIN} — do not edit; regenerate: node scripts/check/docs-drift.mjs --write-mcp-catalog -->`,
    "",
    `**Read tools** — always available once the server is on (${reads.length}):`,
    "",
    table(reads),
    "",
    `**Act tools** — dispatchable only with *Settings → MCP server → Allow write tools* on (${acts.length}):`,
    "",
    table(acts),
    "",
    MCP_CATALOG_END,
  ].join("\n");
}

function extractMcpCatalogNames(fileText) {
  const m = MCP_CATALOG_SPAN_RE.exec(fileText);
  if (!m) return null;
  const names = new Set();
  const rowRe = /^\|\s*`([a-z0-9_]+)`\s*\|/gm;
  let r;
  while ((r = rowRe.exec(m[0]))) names.add(r[1]);
  return names;
}

function checkMcpCatalog(writeCatalog, manifest) {
  if (!manifest) return; // coherence check already reported the parse failure
  let tools;
  try {
    tools = readMcpSnapshotTools();
  } catch (e) {
    errors.push(
      `docs-drift: could not parse the MCP tools/list snapshot (${MCP_SNAPSHOT_REL}): ${e.message}.`,
    );
    return;
  }
  if (tools.length < 80) {
    errors.push(
      `docs-drift: only ${tools.length} MCP tools parsed from ${MCP_SNAPSHOT_REL} (floor: 80) — the snapshot parser likely broke.`,
    );
    return;
  }
  const snapshotNames = new Set(tools.map((t) => t.name));
  const block = buildMcpCatalogBlock(tools, manifest);

  for (const rel of MCP_CATALOG_TARGETS) {
    const abs = resolve(repoRoot, rel);
    let text;
    try {
      text = readFileSync(abs, "utf8");
    } catch {
      errors.push(`docs-drift: MCP catalog target ${rel} is missing.`);
      continue;
    }
    if (!MCP_CATALOG_SPAN_RE.test(text)) {
      errors.push(
        `docs-drift: ${rel} has no MCP catalog markers (\`${MCP_CATALOG_BEGIN} … -->\` / \`${MCP_CATALOG_END}\`).`,
      );
      continue;
    }
    if (writeCatalog) {
      writeFileSync(abs, text.replace(MCP_CATALOG_SPAN_RE, block));
      console.log(`✓ docs-drift: wrote MCP catalog into ${rel}`);
      continue;
    }
    // Byte-for-byte content compare (names AND descriptions) against a
    // freshly regenerated block — a name-set-only compare let a hand-edited
    // description (or a hand-added line inside the do-not-edit span) drift
    // silently while still passing (epic #285 T10 guardrail harvest).
    const actual = MCP_CATALOG_SPAN_RE.exec(text)[0];
    if (actual !== block) {
      const names = extractMcpCatalogNames(text);
      const missing = [...snapshotNames].filter((n) => !names.has(n)).sort();
      const extra = [...names].filter((n) => !snapshotNames.has(n)).sort();
      const actualLines = actual.split("\n");
      const blockLines = block.split("\n");
      let firstDiffLine = -1;
      for (let i = 0; i < Math.max(actualLines.length, blockLines.length); i++) {
        if (actualLines[i] !== blockLines[i]) {
          firstDiffLine = i;
          break;
        }
      }
      errors.push(
        `docs-drift: ${rel} MCP catalog content is out of sync with the tools/list snapshot` +
          " (byte-for-byte compare, names + descriptions)." +
          (missing.length ? `\n    missing tool (in snapshot, not in doc): ${missing.join(", ")}` : "") +
          (extra.length ? `\n    extra tool (in doc, not in snapshot): ${extra.join(", ")}` : "") +
          (firstDiffLine !== -1
            ? `\n    first differing line (${firstDiffLine + 1} of the generated block):` +
              `\n      doc:  ${actualLines[firstDiffLine] ?? "(doc block ends here)"}` +
              `\n      gen:  ${blockLines[firstDiffLine] ?? "(generated block ends here)"}`
            : "") +
          "\n    regenerate: node scripts/check/docs-drift.mjs --write-mcp-catalog",
      );
    }
  }
}

// Manifest/snapshot coherence (#387, ADR 0065): registry-driven read/act plus
// dual-snapshot awareness. This is artifact coherence and docs coverage — NOT
// independent registry verification (manifest, full snapshot, and acquisition
// snapshot all derive from the same `entries()`; each has its own frozen Rust
// test). It catches partially-regenerated artifacts and a workflow inventory
// that drifts from the acquisition surface. Returns the parsed manifest for the
// catalog split, or null on a parse failure (already reported).
function checkMcpManifestCoherence() {
  let manifest;
  let full;
  let acquisition;
  try {
    manifest = readMcpManifest();
    full = readSnapshotJson(MCP_SNAPSHOT_REL)?.result?.tools;
    acquisition = readSnapshotJson(MCP_ACQ_SNAPSHOT_REL)?.result?.tools;
  } catch (e) {
    errors.push(`docs-drift: could not parse the MCP manifest/snapshots: ${e.message}.`);
    return null;
  }
  if (!Array.isArray(full) || !Array.isArray(acquisition)) {
    errors.push("docs-drift: MCP tools/list snapshot(s) have no result.tools array.");
    return null;
  }

  const inventories = [];
  for (const rel of MCP_CATALOG_TARGETS) {
    let names;
    try {
      names = extractWorkflowInventoryNames(readText(rel));
    } catch {
      errors.push(`docs-drift: MCP catalog target ${rel} is missing.`);
      continue;
    }
    inventories.push({ rel, names });
  }

  for (const msg of manifestCoherenceErrors(manifest, full, acquisition, inventories)) {
    errors.push(msg);
  }
  return manifest;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

// Guarded so importing this module (the negative test) never runs the gate.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const writeIndex = process.argv.includes("--write-adr-index");
  const writeCatalog = process.argv.includes("--write-mcp-catalog");

  checkCommands();
  checkScreens();
  checkSettingsKeys();
  checkAdrHygiene(writeIndex);
  const mcpManifest = checkMcpManifestCoherence();
  checkMcpCatalog(writeCatalog, mcpManifest);

  if (errors.length > 0) {
    console.error(`✖ docs-drift: ${errors.length} spec↔code drift instance(s) found (ADR 0065):\n`);
    for (const e of errors) console.error(`  - ${e}\n`);
    process.exit(1);
  }

  console.log(
    "✓ docs-drift: contracts.md, ui-information-architecture.md, and data-model.md match the code; ADR hygiene intact.",
  );
}
