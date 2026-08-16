// Negative tests for the MCP manifest/snapshot coherence gate (#387, ADR 0065).
// The gate splits the tool catalog read/act by the REGISTRY-TRUTH manifest, not
// a tool-name regex, and asserts the manifest, both tools/list snapshots, and
// each doc's acquisition-workflow inventory stay mutually coherent.
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  parseManifestTools,
  manifestCoherenceErrors,
  extractWorkflowInventoryNames,
  buildMcpCatalogBlock,
} from "./docs-drift.mjs";

// The nine acquisition tools in KPI_ACQUISITION_TOOLS (contract) order.
const ACQ9 = [
  "start_kpi_ingest",
  "list_pending_kpi_ingests",
  "get_kpi_ingest_context",
  "get_kpi_ingest_document",
  "stage_kpi_observations",
  "validate_kpi_ingest",
  "commit_kpi_ingest",
  "get_kpi_ingest_status",
  "cancel_kpi_ingest",
];
// Real per-tool tiers (reads: list_pending + the three get_*; acts: the rest).
const ACQ_READ = new Set([
  "list_pending_kpi_ingests",
  "get_kpi_ingest_context",
  "get_kpi_ingest_document",
  "get_kpi_ingest_status",
]);

const wire = (name) => ({ name, description: `does ${name}`, inputSchema: { type: "object" } });

function fixtures() {
  const manifest = parseManifestTools([
    { name: "get_thing", tier: "read", acquisition: false },
    { name: "do_thing", tier: "act", acquisition: false },
    ...ACQ9.map((name) => ({
      name,
      tier: ACQ_READ.has(name) ? "read" : "act",
      acquisition: true,
    })),
  ]);
  const full = manifest.tools.map((t) => wire(t.name));
  const acquisition = full.filter((t) => manifest.byName.get(t.name).acquisition);
  const inventories = [
    { rel: "wiki/mcp-agent-guide.md", names: new Set(ACQ9) },
    { rel: ".claude/skills/brawler-mcp/SKILL.md", names: new Set(ACQ9) },
  ];
  return { manifest, full, acquisition, inventories };
}

test("parseManifestTools rejects a duplicate tool name", () => {
  assert.throws(
    () =>
      parseManifestTools([
        { name: "a", tier: "read", acquisition: false },
        { name: "a", tier: "act", acquisition: false },
      ]),
    /duplicate/,
  );
});

test("parseManifestTools rejects an unknown tier", () => {
  assert.throws(
    () => parseManifestTools([{ name: "a", tier: "write", acquisition: false }]),
    /malformed/,
  );
});

test("parseManifestTools rejects a missing acquisition flag", () => {
  assert.throws(() => parseManifestTools([{ name: "a", tier: "read" }]), /malformed/);
});

test("coherent manifest, snapshots, and inventories yield no errors", () => {
  const { manifest, full, acquisition, inventories } = fixtures();
  assert.deepEqual(manifestCoherenceErrors(manifest, full, acquisition, inventories), []);
});

test("full tools/list order drift is caught", () => {
  const { manifest, full, acquisition, inventories } = fixtures();
  const shuffled = [full[1], full[0], ...full.slice(2)];
  const errs = manifestCoherenceErrors(manifest, shuffled, acquisition, inventories);
  assert.ok(errs.some((e) => /out of sync with the full tools\/list/.test(e)));
});

test("acquisition-snapshot projection drift is caught", () => {
  const { manifest, full, acquisition, inventories } = fixtures();
  const dropped = acquisition.slice(1); // one acquisition tool missing from the scoped snapshot
  const errs = manifestCoherenceErrors(manifest, full, dropped, inventories);
  assert.ok(errs.some((e) => /ordered projection/.test(e)));
});

test("a workflow inventory missing a tool is caught", () => {
  const { manifest, full, acquisition } = fixtures();
  const short = new Set(ACQ9.slice(1));
  const errs = manifestCoherenceErrors(manifest, full, acquisition, [
    { rel: ".claude/skills/brawler-mcp/SKILL.md", names: short },
  ]);
  assert.ok(errs.some((e) => /inventory != the nine/.test(e)));
});

test("an extra tool in the workflow inventory is caught", () => {
  const { manifest, full, acquisition } = fixtures();
  const bloated = new Set([...ACQ9, "record_financial_facts"]);
  const errs = manifestCoherenceErrors(manifest, full, acquisition, [
    { rel: "wiki/mcp-agent-guide.md", names: bloated },
  ]);
  assert.ok(errs.some((e) => /inventory != the nine/.test(e) && /record_financial_facts/.test(e)));
});

test("absent workflow markers are caught", () => {
  const { manifest, full, acquisition } = fixtures();
  const errs = manifestCoherenceErrors(manifest, full, acquisition, [
    { rel: ".claude/skills/brawler-mcp/SKILL.md", names: null },
  ]);
  assert.ok(errs.some((e) => /no acquisition-workflow markers/.test(e)));
});

test("extractWorkflowInventoryNames reads only names inside the markers", () => {
  const text =
    "outside `ignored_before`\n" +
    "<!-- BEGIN ACQUISITION WORKFLOW TOOLS -->\n" +
    "1. `start_kpi_ingest`\n2. `cancel_kpi_ingest`\n" +
    "<!-- END ACQUISITION WORKFLOW TOOLS -->\n" +
    "outside `ignored_after`";
  assert.deepEqual([...extractWorkflowInventoryNames(text)].sort(), [
    "cancel_kpi_ingest",
    "start_kpi_ingest",
  ]);
});

test("extractWorkflowInventoryNames returns null without markers", () => {
  assert.equal(extractWorkflowInventoryNames("no markers `here`"), null);
});

test("catalog split follows the manifest tier, not the tool-name prefix", () => {
  // `get_report` is an ACT tool and `refresh_source` is a READ tool per the
  // manifest — the OPPOSITE of what a /^(get|list|search)_/ regex would infer.
  const manifest = parseManifestTools([
    { name: "get_report", tier: "act", acquisition: false },
    { name: "refresh_source", tier: "read", acquisition: false },
  ]);
  const block = buildMcpCatalogBlock([wire("get_report"), wire("refresh_source")], manifest);
  const readsIdx = block.indexOf("Read tools");
  const actsIdx = block.indexOf("Act tools");
  const readSection = block.slice(readsIdx, actsIdx);
  const actSection = block.slice(actsIdx);
  assert.ok(readSection.includes("`refresh_source`"), "read tool listed under Read");
  assert.ok(actSection.includes("`get_report`"), "act tool listed under Act");
  assert.ok(!readSection.includes("`get_report`"), "act tool not under Read");
});
