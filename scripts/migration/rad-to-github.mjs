#!/usr/bin/env node
// scripts/migration/rad-to-github.mjs
//
// Migrate Radicle issues -> GitHub (issues, labels, milestones, Projects v2 board).
// Plan reference: docs/plans (§E of the CI/tracking migration plan) + §D release:* labels.
//
// Modes (default: --dry-run):
//   --export    Export only: dump every Radicle issue to JSONL. No mutations. Reads `rad` only.
//   --dry-run   Export + compute the migration plan (counts + 5 sample rows). No mutations.
//   --live      Full migration. Requires env MIGRATE_CONFIRM=yes. Runs `gh` mutations.
//
// Design contract (token discipline): stdout carries ONLY summary counters, the dry-run
// plan, and errors. All verbose per-item detail goes to the log file (MIGRATION_LOG env,
// default os.tmpdir()/brawler-migration-log.txt). No issue bodies flow to stdout.
//
// Source of truth for parsing: `rad cob show --format json` (faithful, un-wrapped JSON) —
// NOT the box-drawn `rad issue show` (which hard-wraps bodies to terminal width).
//
// Idempotency / resume:
//   - Export overwrites the JSONL each run.
//   - docs/archive/radicle-issue-map.json ({ "<hex7>": <gh#> }) is written after every
//     created issue; a re-run skips any hex already present in the map.

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

// ---------------------------------------------------------------------------
// Paths & constants
// ---------------------------------------------------------------------------

const SCRIPT_DIR = path.dirname(new URL(import.meta.url).pathname);
const REPO_ROOT = path.resolve(SCRIPT_DIR, '..', '..');
const ARCHIVE_DIR = path.join(REPO_ROOT, 'docs', 'archive');
const EXPORT_PATH = path.join(ARCHIVE_DIR, 'radicle-issues-2026-07.jsonl');
const MAP_PATH = path.join(ARCHIVE_DIR, 'radicle-issue-map.json');
const LOG_PATH = process.env.MIGRATION_LOG || path.join(os.tmpdir(), 'brawler-migration-log.txt');

const ISSUE_TYPE = 'xyz.radicle.issue';
const THROTTLE_MS = 1000; // ~1 req/s for gh mutations (secondary rate limits)
const MAX_RETRIES = 5;

// Status field: single-select option names on the "Brawler board" project.
const STATUS_OPTIONS = ['Backlog', 'Ready', 'In progress', 'Review', 'Done'];
// Map old Radicle state:* label -> Status option name.
const STATE_TO_STATUS = {
  backlog: 'Backlog',
  ready: 'Ready',
  'in-progress': 'In progress',
  doing: 'In progress',
  review: 'Review',
  done: 'Done',
};

// Fixed labels to scaffold (colors are hex without '#'; descriptions per plan §D).
const FIXED_LABELS = [
  { name: 'epic', color: '5319e7', description: 'Epic — groups sub-issues' },
  { name: 'bug', color: 'd73a4a', description: 'Something is broken' },
  { name: 'release:major', color: 'b60205', description: 'Breaking change — bumps MAJOR on merge' },
  { name: 'release:minor', color: 'fbca04', description: 'New feature — bumps MINOR on merge' },
  { name: 'release:patch', color: '0e8a16', description: 'Fix / small change — bumps PATCH on merge' },
  { name: 'release:skip', color: 'cccccc', description: 'Non-shipping change (docs, CI, refactor) — no release' },
];
const PRIORITY_COLORS = { high: 'b60205', medium: 'fbca04', low: '0e8a16' };
const AREA_COLOR = '1d76db';

// ---------------------------------------------------------------------------
// Small utilities
// ---------------------------------------------------------------------------

function sleepMs(ms) {
  // Synchronous sleep without spawning a process.
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

const _logStream = fs.createWriteStream(LOG_PATH, { flags: 'a' });
function log(line) {
  _logStream.write(`[${new Date().toISOString()}] ${line}\n`);
}
function out(line) {
  process.stdout.write(line + '\n');
}
function err(line) {
  process.stderr.write(line + '\n');
}

function run(cmd, args, { allowFail = false } = {}) {
  const r = spawnSync(cmd, args, { encoding: 'utf8', maxBuffer: 128 * 1024 * 1024 });
  if (r.error) {
    if (allowFail) return { ok: false, code: -1, stdout: '', stderr: String(r.error) };
    throw new Error(`spawn ${cmd} failed: ${r.error}`);
  }
  if (r.status !== 0) {
    if (allowFail) return { ok: false, code: r.status, stdout: r.stdout || '', stderr: r.stderr || '' };
    throw new Error(`${cmd} ${args.slice(0, 4).join(' ')} exited ${r.status}: ${(r.stderr || '').slice(0, 500)}`);
  }
  return { ok: true, code: 0, stdout: r.stdout || '', stderr: r.stderr || '' };
}

// gh mutation with throttle + backoff on 403/429/secondary-rate-limit.
function ghRetry(args, { label = '' } = {}) {
  let attempt = 0;
  while (true) {
    const r = run('gh', args, { allowFail: true });
    if (r.ok) {
      sleepMs(THROTTLE_MS);
      return r;
    }
    const rateLimited = /\b(403|429)\b/.test(r.stderr) || /rate limit|secondary rate|abuse/i.test(r.stderr);
    attempt += 1;
    if (rateLimited && attempt <= MAX_RETRIES) {
      const backoff = Math.min(60000, THROTTLE_MS * 2 ** attempt);
      log(`RATE-LIMIT ${label}: attempt ${attempt}, backoff ${backoff}ms :: ${r.stderr.slice(0, 200)}`);
      sleepMs(backoff);
      continue;
    }
    log(`GH-FAIL ${label}: code ${r.code} :: ${r.stderr.slice(0, 400)}`);
    return r;
  }
}

function graphql(query, vars = {}, { label = '' } = {}) {
  const args = ['api', 'graphql', '-f', `query=${query}`];
  for (const [k, v] of Object.entries(vars)) {
    // All our GraphQL variables are string node-ids or string names -> force string with -f.
    args.push('-f', `${k}=${v}`);
  }
  const r = ghRetry(args, { label: label || 'graphql' });
  if (!r.ok) return { ok: false, data: null, stderr: r.stderr };
  try {
    return { ok: true, data: JSON.parse(r.stdout), stderr: '' };
  } catch (e) {
    return { ok: false, data: null, stderr: `JSON parse: ${e}` };
  }
}

// ---------------------------------------------------------------------------
// Radicle read layer (read-only; allowed at all times)
// ---------------------------------------------------------------------------

function radRid() {
  if (process.env.RAD_RID) return process.env.RAD_RID;
  const r = run('rad', ['inspect'], { allowFail: true });
  const m = (r.stdout || '').match(/rad:[0-9A-Za-z]+/);
  if (!m) throw new Error('could not determine Radicle RID (set RAD_RID env)');
  return m[0];
}

function listIssueOids(rid) {
  const r = run('rad', ['cob', 'list', '--repo', rid, '--type', ISSUE_TYPE]);
  return r.stdout.split('\n').map((l) => l.trim()).filter((l) => /^[0-9a-f]{40}$/.test(l));
}

function showIssueCob(rid, oid) {
  const r = run('rad', ['cob', 'show', '--repo', rid, '--type', ISSUE_TYPE, '--object', oid, '--format', 'json'], {
    allowFail: true,
  });
  if (!r.ok) {
    log(`COB-FAIL ${oid}: ${r.stderr.slice(0, 200)}`);
    return null;
  }
  try {
    return JSON.parse(r.stdout.trim());
  } catch (e) {
    log(`COB-PARSE ${oid}: ${e}`);
    return null;
  }
}

// ---------------------------------------------------------------------------
// Record building
// ---------------------------------------------------------------------------

// Some issues store labels as one comma-joined blob ("a,b,c"); normalize by splitting.
function normalizeLabels(labels) {
  const out = [];
  for (const raw of labels || []) {
    for (const piece of String(raw).split(',')) {
      const t = piece.trim();
      if (t) out.push(t);
    }
  }
  return out;
}

function normalizeState(state) {
  const status = state?.status;
  if (status === 'open') return 'open';
  if (status === 'closed') return state?.reason === 'solved' ? 'solved' : 'closed';
  return status || 'unknown';
}

function firstEditTs(comment) {
  const ts = comment?.edits?.[0]?.timestamp;
  return typeof ts === 'number' ? ts : null;
}

function toRecord(oid, cob) {
  const timeline = cob?.thread?.timeline || [];
  const comments = cob?.thread?.comments || {};
  const rootOid = timeline[0] || oid;
  const root = comments[rootOid] || {};
  const rest = timeline.slice(1).map((cid) => {
    const c = comments[cid] || {};
    return {
      id: cid,
      body: c.body || '',
      createdMs: firstEditTs(c),
      author: c.author || null,
      replyTo: c.replyTo || null,
    };
  });
  return {
    id: oid,
    short: oid.slice(0, 7),
    title: cob?.title || '',
    state: normalizeState(cob?.state),
    labels: normalizeLabels(cob?.labels),
    createdMs: firstEditTs(root),
    body: root.body || '',
    comments: rest,
  };
}

// Parse a hex7 out of a "parent:<hex>" / "blocked:<hex>" label value.
function labelValue(labels, prefix) {
  for (const l of labels) {
    if (l.startsWith(prefix)) return l.slice(prefix.length).trim();
  }
  return null;
}
function labelValues(labels, prefix) {
  return labels.filter((l) => l.startsWith(prefix)).map((l) => l.slice(prefix.length).trim());
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

function ensureArchiveDir() {
  fs.mkdirSync(ARCHIVE_DIR, { recursive: true });
}

function exportAll(rid) {
  ensureArchiveDir();
  const oids = listIssueOids(rid);
  log(`EXPORT start: ${oids.length} issue objects from ${rid}`);
  const records = [];
  const lines = [];
  let done = 0;
  for (const oid of oids) {
    const cob = showIssueCob(rid, oid);
    if (!cob) continue;
    const rec = toRecord(oid, cob);
    records.push(rec);
    lines.push(JSON.stringify(rec));
    done += 1;
    if (done % 50 === 0) log(`EXPORT progress: ${done}/${oids.length}`);
  }
  fs.writeFileSync(EXPORT_PATH, lines.join('\n') + (lines.length ? '\n' : ''), 'utf8');
  log(`EXPORT done: wrote ${records.length} records to ${EXPORT_PATH}`);
  return records;
}

// ---------------------------------------------------------------------------
// Plan computation (shared by dry-run and live)
// ---------------------------------------------------------------------------

function computePlan(records) {
  const open = records.filter((r) => r.state === 'open');
  const areas = new Set();
  const priorities = new Set();
  const states = new Map();
  const milestones = new Set();
  let parentCount = 0;
  let blockedCount = 0;
  let commentCount = 0;

  for (const r of open) {
    for (const l of r.labels) {
      if (l.startsWith('area:')) areas.add(l);
      else if (l.startsWith('priority:')) priorities.add(l);
      else if (l.startsWith('state:')) {
        const s = l.slice('state:'.length);
        states.set(s, (states.get(s) || 0) + 1);
      } else if (l.startsWith('milestone:')) milestones.add(l.slice('milestone:'.length));
    }
    if (labelValue(r.labels, 'parent:')) parentCount += 1;
    if (labelValues(r.labels, 'blocked:').length) blockedCount += 1;
    commentCount += r.comments.length;
  }

  const labelSet = new Set(FIXED_LABELS.map((l) => l.name));
  for (const a of areas) labelSet.add(a);
  for (const p of priorities) labelSet.add(p);

  return {
    open,
    epics: open.filter((r) => r.labels.includes('epic')),
    areas: [...areas].sort(),
    priorities: [...priorities].sort(),
    states,
    milestones: [...milestones].sort(),
    labelsToCreate: [...labelSet].sort(),
    parentCount,
    blockedCount,
    commentCount,
  };
}

function mapLabelsForGh(labels) {
  // Keep area:*, priority:*, epic, bug. Drop state:*, milestone:*, parent:*, blocked:*.
  return labels.filter(
    (l) =>
      l.startsWith('area:') ||
      l.startsWith('priority:') ||
      l === 'epic' ||
      l === 'bug'
  );
}

// Pipeline rank; when an issue carries several state:* labels (a data artifact — e.g.
// state:backlog + state:review), pick the most-advanced one.
const STATE_RANK = { backlog: 0, ready: 1, 'in-progress': 2, doing: 2, review: 3, done: 4 };
function statusForRecord(r) {
  const states = labelValues(r.labels, 'state:').filter((s) => s in STATE_TO_STATUS);
  if (!states.length) return 'Backlog';
  const best = states.reduce((a, b) => (STATE_RANK[b] > STATE_RANK[a] ? b : a));
  return STATE_TO_STATUS[best];
}

function milestoneForRecord(r) {
  const m = labelValues(r.labels, 'milestone:');
  return m.length ? m[0] : null;
}

// ---------------------------------------------------------------------------
// Dry-run reporting
// ---------------------------------------------------------------------------

function reportDryRun(records, plan) {
  const stateDist = records.reduce((acc, r) => {
    acc[r.state] = (acc[r.state] || 0) + 1;
    return acc;
  }, {});

  out('=== rad-to-github DRY RUN ===');
  out(`exported total .............. ${records.length}`);
  out(`  state open ................ ${stateDist.open || 0}`);
  out(`  state solved .............. ${stateDist.solved || 0}`);
  out(`  state closed .............. ${stateDist.closed || 0}`);
  out('--- would create (live) ---');
  out(`issues (open only) .......... ${plan.open.length}`);
  out(`  of which epics ............ ${plan.epics.length}`);
  out(`labels ...................... ${plan.labelsToCreate.length}  [${plan.labelsToCreate.join(', ')}]`);
  out(`milestones .................. ${plan.milestones.length}  [${plan.milestones.join(', ')}]`);
  out(`project ..................... 1  ("Brawler board", Status: ${STATUS_OPTIONS.join('/')})`);
  out(`sub-issue links (parent:) ... ${plan.parentCount}`);
  out(`blocked refs (blocked:) ..... ${plan.blockedCount}`);
  out(`comments to carry over ...... ${plan.commentCount}`);
  const statusDist = {};
  for (const r of plan.open) {
    const s = statusForRecord(r);
    statusDist[s] = (statusDist[s] || 0) + 1;
  }
  out(`status distribution ......... ${STATUS_OPTIONS.map((s) => `${s}:${statusDist[s] || 0}`).join('  ')}`);

  out('--- 5 sample open issues ---');
  for (const r of plan.open.slice(0, 5)) {
    out(`#? ${r.short}  "${r.title.slice(0, 70)}"`);
    out(`    labels->gh: [${mapLabelsForGh(r.labels).join(', ') || '(none)'}]`);
    out(`    milestone: ${milestoneForRecord(r) || '(none)'}   status: ${statusForRecord(r)}`);
    const parent = labelValue(r.labels, 'parent:');
    const blocked = labelValues(r.labels, 'blocked:');
    out(`    parent: ${parent || '(none)'}   blocked: ${blocked.join(',') || '(none)'}   comments: ${r.comments.length}`);
  }
  out(`log: ${LOG_PATH}`);
}

// ---------------------------------------------------------------------------
// Live migration
// ---------------------------------------------------------------------------

function ghRepoInfo() {
  const r = run('gh', ['repo', 'view', '--json', 'owner,name,id,nameWithOwner']);
  return JSON.parse(r.stdout);
}

function loadMap() {
  if (fs.existsSync(MAP_PATH)) {
    try {
      return JSON.parse(fs.readFileSync(MAP_PATH, 'utf8'));
    } catch {
      /* fall through */
    }
  }
  return {};
}
function saveMap(map) {
  fs.writeFileSync(MAP_PATH, JSON.stringify(map, null, 2) + '\n', 'utf8');
}

function buildBody(r) {
  let body = `> Migrated from Radicle rad:${r.short}\n\n${r.body}`.trimEnd();
  if (r.comments.length) {
    body += '\n\n## Radicle comments\n';
    for (const c of r.comments) {
      const when = c.createdMs ? new Date(c.createdMs).toISOString().slice(0, 10) : 'unknown date';
      body += `\n**${when}**\n\n${c.body}\n`;
    }
  }
  return body;
}

function scaffoldLabels(plan) {
  const specs = [...FIXED_LABELS];
  for (const a of plan.areas) specs.push({ name: a, color: AREA_COLOR, description: `Area: ${a.slice('area:'.length)}` });
  for (const p of plan.priorities) {
    const lvl = p.slice('priority:'.length);
    specs.push({ name: p, color: PRIORITY_COLORS[lvl] || 'ededed', description: `Priority: ${lvl}` });
  }
  let n = 0;
  for (const s of specs) {
    const r = ghRetry(
      ['label', 'create', s.name, '--color', s.color, '--description', s.description, '--force'],
      { label: `label:${s.name}` }
    );
    if (r.ok) n += 1;
    else log(`LABEL-SKIP ${s.name}: ${r.stderr.slice(0, 150)}`);
  }
  log(`SCAFFOLD labels: ${n}/${specs.length}`);
  return n;
}

function scaffoldMilestones(plan, repo) {
  // Fetch existing to stay idempotent.
  const existing = new Map();
  const r = run('gh', ['api', `repos/${repo.nameWithOwner}/milestones?state=all&per_page=100`], { allowFail: true });
  if (r.ok) {
    try {
      for (const m of JSON.parse(r.stdout)) existing.set(m.title, m.number);
    } catch {
      /* ignore */
    }
  }
  const map = {};
  let created = 0;
  for (const title of plan.milestones) {
    if (existing.has(title)) {
      map[title] = existing.get(title);
      continue;
    }
    const c = ghRetry(
      ['api', `repos/${repo.nameWithOwner}/milestones`, '-f', `title=${title}`,
        '-f', 'description=Historical grouping migrated from Radicle (versioned milestones retired).'],
      { label: `milestone:${title}` }
    );
    if (c.ok) {
      try {
        map[title] = JSON.parse(c.stdout).number;
        created += 1;
      } catch {
        /* ignore */
      }
    }
  }
  log(`SCAFFOLD milestones: created ${created}, total mapped ${Object.keys(map).length}`);
  return map;
}

const Q_FIND_PROJECT = `query($login:String!){ user(login:$login){ projectsV2(first:100){ nodes{ id number title } } } }`;
const Q_CREATE_PROJECT = `mutation($owner:ID!,$title:String!){ createProjectV2(input:{ownerId:$owner,title:$title}){ projectV2{ id number title } } }`;
const Q_PROJECT_FIELDS = `query($id:ID!){ node(id:$id){ ... on ProjectV2 { fields(first:50){ nodes{ ... on ProjectV2SingleSelectField { id name options{ id name } } ... on ProjectV2FieldCommon { id name } } } } } }`;
// Options are inlined literally: color is a GraphQL enum (must be unquoted); names are
// JSON-escaped (JSON string literals are valid GraphQL string literals).
function buildStatusFieldMutation(fieldId, names) {
  const opts = names.map((n) => `{name:${JSON.stringify(n)},color:GRAY,description:""}`).join(',');
  return `mutation{ updateProjectV2Field(input:{fieldId:${JSON.stringify(fieldId)},singleSelectOptions:[${opts}]}){ projectV2Field{ ... on ProjectV2SingleSelectField { id options{ id name } } } } }`;
}
const Q_ADD_ITEM = `mutation($project:ID!,$content:ID!){ addProjectV2ItemById(input:{projectId:$project,contentId:$content}){ item{ id } } }`;
const Q_SET_FIELD = `mutation($project:ID!,$item:ID!,$field:ID!,$opt:String!){ updateProjectV2ItemFieldValue(input:{projectId:$project,itemId:$item,fieldId:$field,value:{singleSelectOptionId:$opt}}){ projectV2Item{ id } } }`;
const Q_ADD_SUBISSUE = `mutation($parent:ID!,$sub:ID!){ addSubIssue(input:{issueId:$parent,subIssueId:$sub}){ issue{ id } } }`;

function scaffoldProject(repo) {
  const login = repo.owner.login;
  // Reuse an existing "Brawler board" if present (resume-safe).
  const found = graphql(Q_FIND_PROJECT, { login }, { label: 'find-project' });
  let project = null;
  if (found.ok) {
    const nodes = found.data?.data?.user?.projectsV2?.nodes || [];
    project = nodes.find((n) => n.title === 'Brawler board') || null;
  }
  if (!project) {
    const c = graphql(Q_CREATE_PROJECT, { owner: repo.owner.id, title: 'Brawler board' }, { label: 'create-project' });
    if (!c.ok) {
      log(`PROJECT-FAIL create: ${c.stderr.slice(0, 300)}`);
      return null;
    }
    project = c.data?.data?.createProjectV2?.projectV2;
  }
  if (!project) return null;

  // Find the Status single-select field.
  const f = graphql(Q_PROJECT_FIELDS, { id: project.id }, { label: 'project-fields' });
  const fields = f.ok ? f.data?.data?.node?.fields?.nodes || [] : [];
  let status = fields.find((x) => x && x.name === 'Status');

  // Ensure our option set. Preferred/robust path: update the built-in Status field options.
  if (status) {
    const u = graphql(buildStatusFieldMutation(status.id, STATUS_OPTIONS), {}, { label: 'update-status-field' });
    if (u.ok && u.data?.data?.updateProjectV2Field?.projectV2Field) {
      status = u.data.data.updateProjectV2Field.projectV2Field;
    } else {
      log(`STATUS-FIELD update failed (using existing options): ${u.stderr.slice(0, 200)}`);
    }
  } else {
    log('PROJECT: no Status field found; issues will be added without status.');
  }

  const optionByName = {};
  for (const o of status?.options || []) optionByName[o.name.toLowerCase()] = o.id;
  log(`SCAFFOLD project: id=${project.id} number=${project.number} status-options=${Object.keys(optionByName).join(',')}`);
  return { id: project.id, number: project.number, statusFieldId: status?.id || null, optionByName };
}

function nodeIdForIssue(repo, number) {
  const r = run('gh', ['api', `repos/${repo.nameWithOwner}/issues/${number}`, '--jq', '.node_id'], { allowFail: true });
  return r.ok ? r.stdout.trim() : null;
}

function createIssues(plan, repo, project, milestoneMap, map, bodies) {
  // Epics first, then the rest — so parent links resolve in the second pass.
  const ordered = [...plan.epics, ...plan.open.filter((r) => !plan.epics.includes(r))];
  let created = 0;
  let skipped = 0;
  for (const r of ordered) {
    if (map[r.short]) {
      skipped += 1;
      bodies[r.short] = bodies[r.short] || buildBody(r);
      continue;
    }
    const body = buildBody(r);
    bodies[r.short] = body;
    const tmp = path.join(os.tmpdir(), `radmig-${r.short}.md`);
    fs.writeFileSync(tmp, body, 'utf8');
    const args = ['issue', 'create', '--repo', repo.nameWithOwner, '--title', r.title, '--body-file', tmp];
    for (const l of mapLabelsForGh(r.labels)) args.push('--label', l);
    const ms = milestoneForRecord(r);
    if (ms && milestoneMap[ms] != null) args.push('--milestone', ms);
    const res = ghRetry(args, { label: `create:${r.short}` });
    try {
      fs.unlinkSync(tmp);
    } catch {
      /* ignore */
    }
    if (!res.ok) {
      err(`ERROR create ${r.short}: ${res.stderr.slice(0, 160)}`);
      continue;
    }
    const m = (res.stdout || '').match(/\/issues\/(\d+)/);
    if (!m) {
      err(`ERROR create ${r.short}: no issue number in output`);
      continue;
    }
    const number = Number(m[1]);
    map[r.short] = number;
    saveMap(map); // incremental — resume-safe
    created += 1;

    // Add to project + set Status.
    if (project) {
      const nodeId = nodeIdForIssue(repo, number);
      if (nodeId) {
        const add = graphql(Q_ADD_ITEM, { project: project.id, content: nodeId }, { label: `add-item:${r.short}` });
        const itemId = add.ok ? add.data?.data?.addProjectV2ItemById?.item?.id : null;
        if (itemId && project.statusFieldId) {
          const optName = statusForRecord(r).toLowerCase();
          const optId = project.optionByName[optName];
          if (optId) {
            graphql(
              Q_SET_FIELD,
              { project: project.id, item: itemId, field: project.statusFieldId, opt: optId },
              { label: `set-status:${r.short}` }
            );
          } else {
            log(`STATUS-OPT missing for "${optName}" (${r.short})`);
          }
        }
      }
    }
    if (created % 25 === 0) out(`  created ${created} issues...`);
  }
  log(`CREATE done: created ${created}, skipped(resumed) ${skipped}`);
  return { created, skipped };
}

function linkStructure(plan, repo, map, bodies) {
  let subOk = 0;
  let subFallback = 0;
  let blockedEdits = 0;
  for (const r of plan.open) {
    const childNum = map[r.short];
    if (!childNum) continue;

    // parent: -> sub-issue
    const parentHex = labelValue(r.labels, 'parent:');
    if (parentHex && map[parentHex]) {
      const parentNum = map[parentHex];
      const parentNode = nodeIdForIssue(repo, parentNum);
      const childNode = nodeIdForIssue(repo, childNum);
      let linked = false;
      if (parentNode && childNode) {
        const g = graphql(Q_ADD_SUBISSUE, { parent: parentNode, sub: childNode }, { label: `subissue:${r.short}` });
        linked = g.ok && !!g.data?.data?.addSubIssue;
      }
      if (linked) {
        subOk += 1;
      } else {
        // Fallback: comment "Parent: #n" + a marker label.
        ghRetry(['issue', 'comment', String(childNum), '--repo', repo.nameWithOwner, '--body', `Parent: #${parentNum}`], {
          label: `parent-comment:${r.short}`,
        });
        ghRetry(['issue', 'edit', String(childNum), '--repo', repo.nameWithOwner, '--add-label', 'sub-issue'], {
          label: `parent-label:${r.short}`,
        });
        subFallback += 1;
      }
    } else if (parentHex) {
      log(`PARENT unresolved for ${r.short}: parent ${parentHex} not in map`);
    }

    // blocked: -> body appendix
    const blocked = labelValues(r.labels, 'blocked:').filter((h) => map[h]);
    if (blocked.length) {
      const refs = blocked.map((h) => `#${map[h]} (rad:${h})`).join(', ');
      const newBody = `${bodies[r.short] || buildBody(r)}\n\n> Blocked by ${refs}`;
      const tmp = path.join(os.tmpdir(), `radmig-blk-${r.short}.md`);
      fs.writeFileSync(tmp, newBody, 'utf8');
      const e = ghRetry(['issue', 'edit', String(childNum), '--repo', repo.nameWithOwner, '--body-file', tmp], {
        label: `blocked-edit:${r.short}`,
      });
      try {
        fs.unlinkSync(tmp);
      } catch {
        /* ignore */
      }
      if (e.ok) blockedEdits += 1;
    }
  }
  log(`LINK done: sub-issues ${subOk}, sub-fallbacks ${subFallback}, blocked edits ${blockedEdits}`);
  return { subOk, subFallback, blockedEdits };
}

function migrateLive(records, plan) {
  if (process.env.MIGRATE_CONFIRM !== 'yes') {
    err('REFUSED: --live requires env MIGRATE_CONFIRM=yes');
    process.exit(2);
  }
  const repo = ghRepoInfo();
  out('=== rad-to-github LIVE ===');
  out(`repo: ${repo.nameWithOwner}`);

  const labels = scaffoldLabels(plan);
  out(`labels ensured: ${labels}`);
  const milestoneMap = scaffoldMilestones(plan, repo);
  out(`milestones mapped: ${Object.keys(milestoneMap).length}`);
  const project = scaffoldProject(repo);
  out(`project: ${project ? `#${project.number}` : 'FAILED'}`);

  const map = loadMap();
  const bodies = {};
  const c = createIssues(plan, repo, project, milestoneMap, map, bodies);
  out(`issues created: ${c.created}  resumed/skipped: ${c.skipped}`);
  const l = linkStructure(plan, repo, map, bodies);
  out(`sub-issues: ${l.subOk} (native) + ${l.subFallback} (fallback)   blocked edits: ${l.blockedEdits}`);
  out(`map: ${MAP_PATH}  (${Object.keys(map).length} entries)`);
  out(`log: ${LOG_PATH}`);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  const args = process.argv.slice(2);
  let mode = 'dry-run';
  if (args.includes('--live')) mode = 'live';
  else if (args.includes('--export')) mode = 'export';
  else if (args.includes('--dry-run')) mode = 'dry-run';

  log(`START mode=${mode} argv=${JSON.stringify(args)}`);
  const rid = radRid();
  const records = exportAll(rid);

  if (mode === 'export') {
    const stateDist = records.reduce((a, r) => ((a[r.state] = (a[r.state] || 0) + 1), a), {});
    out(`exported ${records.length} issues -> ${EXPORT_PATH}`);
    out(`states: open ${stateDist.open || 0}, solved ${stateDist.solved || 0}, closed ${stateDist.closed || 0}`);
    out(`log: ${LOG_PATH}`);
    return;
  }

  const plan = computePlan(records);
  if (mode === 'dry-run') {
    reportDryRun(records, plan);
    return;
  }
  if (mode === 'live') {
    migrateLive(records, plan);
  }
}

try {
  main();
} catch (e) {
  err(`FATAL: ${e.message || e}`);
  log(`FATAL: ${e.stack || e}`);
  process.exitCode = 1;
} finally {
  _logStream.end();
}
