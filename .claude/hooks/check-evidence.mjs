#!/usr/bin/env node
// check-evidence PreToolUse hook (S2, hard-gates closing wave, DoD §K): denies
// `gh pr create`/`gh pr ready` (without --undo, and without --draft/-d — a
// draft PR is WIP) and a `git push` that updates a branch with an open
// non-draft PR unless a check-stamp (scripts/check/check-stamp.mjs)
// certifies `make check-local`/`check-docs` ran green on the pushed
// source's exact tree — a "done" claim (a PR, or a push that updates one)
// must be backed by a fresh gate run, not a stale or imagined one.
// `git push` is evaluated per refspec (`src:dst` uses `dst` for the PR
// lookup and `src` for the stamped tree; a bare `branch` is `branch:branch`;
// a leading `+` force marker is stripped from the whole spec; a deletion
// refspec, or `--delete`, updates nothing and is never denied; `--repo`/
// `--receive-pack`/`--exec`/`-o`/`--push-option` and their values are
// skipped rather than mistaken for the remote or a refspec). Only the
// DESTINATION's `refs/heads/` prefix is stripped (it drives `gh pr list
// --head <dst>`, which wants a bare name) — the SOURCE keeps whatever
// qualification the user gave it, since stripping it would let a same-named
// tag win the tree lookup over the actual branch (git's plain-name
// resolution order). `gh pr create --head`/`-H` resolves DIRECTLY to the
// named local branch's tree (the PR doesn't exist yet, so there is nothing
// to look up); `gh pr ready <number|branch|url>`/`--head`/`-H`/bare (current
// branch) resolves the PR's actual head COMMIT via `gh pr view` and checks
// that tree — a commit not present locally is denied naming the `git fetch`
// needed, so stale local content (including a HEAD that merely looks done)
// can never stand in for the real evidence. `gh pr create`'s `--draft`/`-d`
// and `gh pr ready --undo` are WIP exemptions applied before any repository
// check. Leading global options (`git -C`/`-c`/`--git-dir`/`--work-tree`/
// `--no-pager`, …; `gh -R`/`--repo`/`--hostname`) are skipped before the
// subcommand is located, and `--repo`/`-R` given AFTER the `pr create`/
// `pr ready` subcommand is honored the same way; `git -C`/`--git-dir`/
// `--work-tree` redirect every evidence lookup to that repository (not the
// invoking cwd, and `-C` is never dropped just because `--git-dir`/
// `--work-tree` are also present — `gh` itself has no `--git-dir`
// equivalent, so a `--git-dir`-only push resolves the real worktree first so
// the PR-list lookup runs there too), and `gh -R`/`--repo` is compared
// against the checkout's own `origin` remote (accepting `git@host:o/r`,
// `ssh://`/`git+ssh://`/`https://` with an optional `user@`, a trailing
// slash, and a `.git` suffix, case-insensitively) — a mismatch denies
// (evidence is per-checkout). Claude-tool defence-in-depth; DoD §K
// stays the rule — a shell alias, a script file's own body, or a
// hand-edited `.artifacts/*.json` stamp sit outside what this hook can
// intercept: local bookkeeping, never authenticated evidence.
//
// Protocol matches git-boundaries.mjs: JSON on stdin, deny writes
// {hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"deny",
// permissionDecisionReason}} and exits 0; allow writes nothing. Fail-open on
// any parse/git error except the explicit denials this hook decides.
//
// Escape: BRAWLER_CHECK_EVIDENCE_OFF=1 read from THIS process's own env only.
//
// Parsing reuses splitSegments/wrappedScript/tokenizeRaw from
// one-heavy-build-classify.mjs (as git-boundaries.mjs does), with a reduced,
// purpose-built leaf-segment walk copied from git-boundaries.mjs's own
// approach — prefix stripping (rtk/env/timeout/command) and `cd` cwd-tracking
// only; no eval/$()/backtick/subshell unwrapping (ponytail: this hook's two
// rules are narrower and less adversarial than git-boundaries' full matrix —
// raise if a real command needs more).
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { splitSegments, wrappedScript, tokenizeRaw } from "./one-heavy-build-classify.mjs";

const RULE_HOME = "engineering-workflow.md § Definition of Done (DoD §K)";
const ASSIGNMENT_RE = /^[A-Za-z_][A-Za-z0-9_]*=/;
const SHELL_WRAPPERS = new Set(["bash", "sh", "zsh", "dash"]);

function collectLeafSegments(cmd) {
  const out = [];
  for (const segment of splitSegments(cmd)) {
    const inner = wrappedScript(segment);
    if (inner !== null) out.push(...collectLeafSegments(inner));
    else out.push(segment);
  }
  return out;
}

function shellWrapperBody(raw, idx) {
  const head = raw[idx]?.text;
  if (!head) return undefined;
  if (!SHELL_WRAPPERS.has(head.replace(/^.*\//, ""))) return undefined;
  for (let j = idx + 1; j < raw.length; j++) {
    const t = raw[j].text;
    if (/^-[a-zA-Z]*c[a-zA-Z]*$/.test(t)) return raw[j + 1] ? raw[j + 1].text : null;
    if (!t.startsWith("-")) return undefined;
  }
  return undefined;
}

/** Git global options consumed before the subcommand: -C changes the invocation's cwd; -c,
 * --namespace, --super-prefix, --config-env, --exec-path take a value (or an attached `=value`)
 * that is evidence-irrelevant and just skipped; --git-dir/--work-tree (bare or `=value`) name the
 * repository the evidence lookup must actually target — captured into `gitPrefix` and passed
 * through to every spawned git call, rather than folded into `cwd` (which a bare `-C` still
 * tracks for shelling out to `gh`, since gh has no --git-dir equivalent). Any other leading `-…`
 * token (--no-pager, --bare, …) is a boolean global. Returns the remaining tokens (starting at
 * the subcommand), the resolved cwd (worktree, when known), and the git invocation prefix. */
function parseGitGlobals(rest, startCwd) {
  const VALUE_ONLY = new Set(["-c", "--namespace", "--super-prefix", "--config-env", "--exec-path"]);
  let cwd = startCwd;
  let gitDir;
  let workTree;
  let sawC = false;
  let i = 0;
  while (i < rest.length) {
    const t = rest[i].text;
    if (t === "-C") {
      cwd = path.resolve(cwd, rest[i + 1]?.text ?? ".");
      sawC = true;
      i += 2;
      continue;
    }
    if (t === "--git-dir" || t === "--work-tree") {
      const val = rest[i + 1]?.text ?? "";
      if (t === "--git-dir") gitDir = path.resolve(cwd, val);
      else workTree = path.resolve(cwd, val);
      i += 2;
      continue;
    }
    let m = t.match(/^--git-dir=(.*)$/);
    if (m) {
      gitDir = path.resolve(cwd, m[1]);
      i += 1;
      continue;
    }
    m = t.match(/^--work-tree=(.*)$/);
    if (m) {
      workTree = path.resolve(cwd, m[1]);
      i += 1;
      continue;
    }
    if (VALUE_ONLY.has(t)) {
      i += 2;
      continue;
    }
    if (/^-c.+=/.test(t)) {
      i += 1;
      continue;
    }
    if (t.startsWith("-")) {
      i += 1; // other boolean global (--no-pager, --bare, --literal-pathspecs, …)
      continue;
    }
    break;
  }
  // -C's own repo discovery is still needed when --git-dir/--work-tree name a relative or
  // ambiguous path, or when a bare --git-dir needs a starting point (resolveGhCwd below) — an
  // explicit -C is never dropped just because --git-dir/--work-tree are also present.
  const gitPrefix = [
    ...(sawC || (!gitDir && !workTree) ? ["-C", cwd] : []),
    ...(gitDir ? ["--git-dir", gitDir] : []),
    ...(workTree ? ["--work-tree", workTree] : []),
  ];
  return { rest: rest.slice(i), cwd: workTree ?? cwd, gitPrefix };
}

/** gh global options consumed before the subcommand: -R/--repo and --hostname take a value
 * (or an attached `=value`). `--repo`'s value is captured (for the repository-identity check)
 * rather than only skipped. */
function parseGhGlobals(rest) {
  let repo;
  let i = 0;
  while (i < rest.length) {
    const t = rest[i].text;
    if (t === "-R" || t === "--repo") {
      repo = rest[i + 1]?.text;
      i += 2;
      continue;
    }
    if (t === "--hostname") {
      i += 2;
      continue;
    }
    const m = t.match(/^(--repo|--hostname)=(.*)$/);
    if (m) {
      if (m[1] === "--repo") repo = m[2];
      i += 1;
      continue;
    }
    break;
  }
  return { rest: rest.slice(i), repo };
}

/** Strip transparent prefixes (assignments, command/env/rtk/timeout), tracking `cd` as cwd
 * state on `state`; unwrap a trailing shell-wrapper body; dispatch git/gh. Returns a deny reason
 * or null. */
function processLeafSegment(segmentText, state) {
  const raw = tokenizeRaw(segmentText);
  let i = 0;
  for (let guard = 0; guard < 10; guard++) {
    const t = raw[i]?.text;
    if (t === undefined) break;
    if (ASSIGNMENT_RE.test(t)) {
      i++;
      continue;
    }
    if (t === "command") {
      i++;
      continue;
    }
    if (t === "env") {
      i++;
      while (raw[i] && (raw[i].text === "-u" || ASSIGNMENT_RE.test(raw[i].text))) {
        i += raw[i].text === "-u" ? 2 : 1;
      }
      continue;
    }
    if (t === "rtk") {
      i++;
      if (raw[i]?.text === "proxy") i++;
      continue;
    }
    if (t === "timeout") {
      i++;
      while (raw[i] && raw[i].text.startsWith("-")) {
        if (/^(-s|-k|--signal|--kill-after)$/.test(raw[i].text)) i++;
        i++;
      }
      i++;
      continue;
    }
    break;
  }
  if (raw[i]?.text === "cd" && raw[i + 1]) {
    state.cwd = path.resolve(state.cwd, raw[i + 1].text);
    return null;
  }
  const wrapperBody = shellWrapperBody(raw, i);
  if (wrapperBody !== undefined) {
    return wrapperBody ? evaluateInner(wrapperBody, state) : null;
  }
  const head = raw[i]?.text;
  if (!head) return null;
  const base = head.replace(/^.*\//, "");
  const rest = raw.slice(i + 1);
  if (base === "gh") {
    const { rest: ghRest, repo } = parseGhGlobals(rest);
    return checkGh(ghRest, { cwd: state.cwd, gitPrefix: ["-C", state.cwd], repoOverride: repo });
  }
  if (base === "git") {
    const { rest: gitRest, cwd, gitPrefix } = parseGitGlobals(rest, state.cwd);
    return checkGit(gitRest, { cwd, gitPrefix });
  }
  return null;
}

function evaluateInner(cmd, state) {
  for (const segment of collectLeafSegments(cmd)) {
    const reason = processLeafSegment(segment, state);
    if (reason) return reason;
  }
  return null;
}

// ---- stamp evidence ----

function toplevel(gitPrefix) {
  const r = spawnSync("git", [...gitPrefix, "rev-parse", "--show-toplevel"], {
    cwd: resolveGhCwd(gitPrefix, process.cwd()),
    encoding: "utf8",
  });
  return r.status === 0 ? r.stdout.trim() : null;
}

function treeOf(gitPrefix, ref) {
  const r = spawnSync("git", [...gitPrefix, "rev-parse", `${ref}^{tree}`], { encoding: "utf8" });
  return r.status === 0 ? r.stdout.trim() : null;
}

function commitExists(gitPrefix, oid) {
  const r = spawnSync("git", [...gitPrefix, "cat-file", "-e", `${oid}^{commit}`], { encoding: "utf8" });
  return r.status === 0;
}

/** `gh` has no `--git-dir` equivalent — it needs an actual cwd inside the target repo to know
 * which GitHub repository to query. When `gitPrefix` carries a `--git-dir` but no `--work-tree`
 * (so `cwd` still points at the invoking directory, not the repo), resolve the real worktree: seed
 * a candidate (the git-dir's parent, the usual `.git` layout) and confirm it via `rev-parse
 * --show-toplevel` run FROM that candidate — never from the invoking cwd, which `--show-toplevel`
 * would silently echo back as a bogus answer instead of erroring. Falls back to the git-dir-derived
 * candidate itself when rev-parse fails (a bare repo has no worktree to confirm). `-C`/`--work-tree`
 * (or neither global given) already leave `cwd` correct — returned unchanged. */
function resolveGhCwd(gitPrefix, cwd) {
  const gitDirIdx = gitPrefix.indexOf("--git-dir");
  if (gitDirIdx === -1 || gitPrefix.includes("--work-tree")) return cwd;
  const gitDir = gitPrefix[gitDirIdx + 1];
  const candidate = path.basename(gitDir) === ".git" ? path.dirname(gitDir) : gitDir;
  const r = spawnSync("git", ["--git-dir", gitDir, "rev-parse", "--show-toplevel"], {
    cwd: candidate,
    encoding: "utf8",
  });
  return r.status === 0 && r.stdout.trim() ? r.stdout.trim() : candidate;
}

function readStamp(top, target) {
  try {
    return JSON.parse(readFileSync(path.join(top, ".artifacts", `${target}.json`), "utf8"));
  } catch {
    return null;
  }
}

function mergeBase(gitPrefix, ref) {
  const r = spawnSync("git", [...gitPrefix, "merge-base", "origin/master", ref], { encoding: "utf8" });
  return r.status === 0 ? r.stdout.trim() : null;
}

/** Docs-only per `git diff --name-status -M`: a rename/copy must have BOTH endpoints under
 * docs/wiki/*.md; every other status (A/D/M/T/…) checks its one path — a type change (T) is
 * judged the same as a modification. */
function isDocsOnly(gitPrefix, base, ref) {
  const r = spawnSync("git", [...gitPrefix, "diff", "--name-status", "-M", `${base}..${ref}`], {
    encoding: "utf8",
  });
  if (r.status !== 0) return false;
  const lines = r.stdout.split("\n").filter(Boolean);
  if (lines.length === 0) return false;
  const isDoc = (p) => p.startsWith("docs/") || p.startsWith("wiki/") || p.endsWith(".md");
  return lines.every((line) => {
    const [status, ...paths] = line.split("\t");
    if (status[0] === "R" || status[0] === "C") return isDoc(paths[0]) && isDoc(paths[1]);
    return isDoc(paths[0]);
  });
}

const DOD_HINT =
  "run `make check-local` (or `make check-docs` for a docs-only change) on this tree and commit " +
  "everything it checked (the stamp tree must equal the pushed source's tree — untracked or uncommitted files break the match)";

/** Whether a fresh check-local/check-docs stamp certifies `ref`'s exact tree, resolved via
 * `gitPrefix` (`-C <cwd>` normally, or the `--git-dir`/`--work-tree` a leading git global named). */
function hasFreshStamp(gitPrefix, ref = "HEAD") {
  const top = toplevel(gitPrefix);
  const tree = top ? treeOf(gitPrefix, ref) : null;
  if (!top || !tree) {
    return { ok: false, reason: `check-evidence: could not resolve ${ref}^{tree} — ${RULE_HOME}` };
  }
  const base = mergeBase(gitPrefix, ref);
  if (base === null) {
    return {
      ok: false,
      reason: `check-evidence: no merge-base with origin/master for ${ref} — run \`git fetch origin master\` first, then ${DOD_HINT} — ${RULE_HOME}`,
    };
  }
  const target = isDocsOnly(gitPrefix, base, ref) ? "check-docs" : "check-local";
  const stamp = readStamp(top, target);
  if (!stamp || stamp.tree !== tree || stamp.toplevel !== top) {
    return { ok: false, reason: `check-evidence: ${DOD_HINT} — ${RULE_HOME}` };
  }
  return { ok: true };
}

/** Normalize a GitHub repo identity (a `gh --repo` value, or an `origin` remote URL) to
 * `owner/repo` so the two can be compared regardless of form: `git@github.com:o/r.git` (scp-like
 * SSH), `ssh://`/`git+ssh://` with an optional `user@`, `https://` with an optional `user@`, a
 * trailing slash, a `.git` suffix, or an already-bare `o/r`. Comparison at the call site is
 * case-insensitive; this only strips the wrapping. */
function normalizeSlug(s) {
  if (!s) return null;
  let v = s.trim();
  let m = v.match(/^git@github\.com:(.+)$/);
  if (!m) m = v.match(/^(?:git\+)?(?:https?|ssh):\/\/(?:[^@/]+@)?github\.com\/(.+)$/);
  if (m) v = m[1];
  return v.replace(/\/+$/, "").replace(/\.git$/, "");
}

function currentRepoSlug(gitPrefix) {
  const r = spawnSync("git", [...gitPrefix, "remote", "get-url", "origin"], { encoding: "utf8" });
  return r.status === 0 ? normalizeSlug(r.stdout.trim()) : null;
}

/** `gh -R/--repo <slug>` names a different GitHub repository than this checkout's own `origin`
 * remote: evidence (the stamp, the tree) is always per-checkout, so a mismatch is denied outright
 * rather than silently checking the wrong repo's evidence. Fails open when either side can't be
 * resolved (offline, no origin remote, …). */
function repoMismatch(state) {
  if (!state.repoOverride) return null;
  const current = currentRepoSlug(state.gitPrefix);
  const wanted = normalizeSlug(state.repoOverride);
  if (!current || !wanted || current.toLowerCase() === wanted.toLowerCase()) return null;
  return (
    `check-evidence: --repo ${state.repoOverride} targets a different repository than this ` +
    `checkout's origin (${current}) — evidence is per-checkout; run the command from that repository — ${RULE_HOME}`
  );
}

/** Resolve a PR identifier (number/branch/url) to its head commit oid + branch via `gh pr view`;
 * `undefined` resolves the CURRENT branch's PR (`gh pr view` with no positional arg). Null
 * (fail-open) if it can't be resolved locally — the caller then skips the evidence check. */
function resolveGhPrHead(arg, cwd) {
  const args = ["pr", "view", ...(arg !== undefined ? [arg] : []), "--json", "headRefOid,headRefName,isDraft"];
  const gh = spawnSync("gh", args, {
    cwd,
    encoding: "utf8",
    timeout: 5000,
  });
  if (gh.status !== 0 || !gh.stdout) return null;
  try {
    const data = JSON.parse(gh.stdout);
    if (typeof data.headRefOid !== "string" || typeof data.headRefName !== "string") return null;
    return { oid: data.headRefOid, branch: data.headRefName };
  } catch {
    return null;
  }
}

/** `gh pr create --head <branch>`: the PR doesn't exist yet, so there is nothing to look up via
 * `gh pr view` — resolve the creation head DIRECTLY as the named local branch's tree. `undefined`
 * (no --head given): publishing the checked-out branch, checked at HEAD as before. */
function checkPrCreate(headBranch, state) {
  if (headBranch === undefined) {
    const { ok, reason } = hasFreshStamp(state.gitPrefix);
    return ok ? null : reason;
  }
  const { ok, reason } = hasFreshStamp(state.gitPrefix, headBranch);
  return ok ? null : reason;
}

/** `gh pr ready <number|branch|url>`/`--head`/bare (current branch): the PR already exists, so its
 * actual head COMMIT (not just branch name — a fork, an unfetched branch, or a local HEAD that
 * simply looks done may share no relationship with it) is resolved via `gh pr view` and checked
 * directly; a commit not present locally is denied naming the `git fetch` needed, since stale local
 * content must never stand in for the real evidence. `undefined` (no identifier given) resolves the
 * CURRENT branch's PR the same way — `gh pr view` with no positional arg. Not resolvable locally
 * (no PR, offline, …): fails open, same as the numbered form. */
function checkPrReady(arg, state) {
  const head = resolveGhPrHead(arg, state.cwd);
  if (!head) return null; // fail-open: PR/branch not resolvable locally
  if (!commitExists(state.gitPrefix, head.oid)) {
    return (
      `check-evidence: PR head commit ${head.oid} (${head.branch}) is not present locally — ` +
      `run \`git fetch origin ${head.branch}\` first, then retry — ${RULE_HOME}`
    );
  }
  const { ok, reason } = hasFreshStamp(state.gitPrefix, head.oid);
  return ok ? null : reason;
}

// gh pr create options that take a value — their value must never be mistaken for --draft/-d or
// another flag (e.g. `--title "--draft"`).
const GH_PR_CREATE_VALUE_OPTS = new Set([
  "--title",
  "-t",
  "--body",
  "-b",
  "--body-file",
  "-F",
  "--base",
  "-B",
  "--head",
  "-H",
  "--reviewer",
  "-r",
  "--assignee",
  "-a",
  "--label",
  "-l",
  "--milestone",
  "-m",
  "--project",
  "-p",
  "--template",
  "-T",
  "--repo",
  "-R",
]);

/** Value-aware scan of `gh pr create` args: every value-taking option's value is consumed and
 * never re-examined as a flag, so `--title "--draft"` is never mistaken for `--draft`. `--repo`/
 * `-R` given here (after the subcommand) is captured too — `gh` accepts it in either position. */
function parseGhPrCreateArgs(args) {
  let draft = false;
  let head;
  let repo;
  let i = 0;
  while (i < args.length) {
    const t = args[i].text;
    if (t === "--draft" || t === "-d") {
      draft = true;
      i++;
      continue;
    }
    if (t.startsWith("-")) {
      const eq = t.indexOf("=");
      const name = eq === -1 ? t : t.slice(0, eq);
      if (name === "--head" || name === "-H") {
        head = eq === -1 ? args[i + 1]?.text : t.slice(eq + 1);
      }
      if (name === "--repo" || name === "-R") {
        repo = eq === -1 ? args[i + 1]?.text : t.slice(eq + 1);
      }
      if (GH_PR_CREATE_VALUE_OPTS.has(name) && eq === -1) {
        i += 2;
        continue;
      }
      i++;
      continue;
    }
    i++;
  }
  return { draft, head, repo };
}

// gh pr ready options that take a value.
const GH_PR_READY_VALUE_OPTS = new Set(["--repo", "-R", "--head", "-H"]);

/** Value-aware scan of `gh pr ready` args, mirroring `parseGhPrCreateArgs`: `--repo`/`-R` given
 * after the subcommand is captured, and its value (and `--head`/`-H`'s) is never mistaken for the
 * positional PR identifier. */
function parseGhPrReadyArgs(args) {
  let undo = false;
  let head;
  let repo;
  let positional;
  let i = 0;
  while (i < args.length) {
    const t = args[i].text;
    if (t === "--undo") {
      undo = true;
      i++;
      continue;
    }
    if (t.startsWith("-")) {
      const eq = t.indexOf("=");
      const name = eq === -1 ? t : t.slice(0, eq);
      if (name === "--head" || name === "-H") {
        head = eq === -1 ? args[i + 1]?.text : t.slice(eq + 1);
      }
      if (name === "--repo" || name === "-R") {
        repo = eq === -1 ? args[i + 1]?.text : t.slice(eq + 1);
      }
      if (GH_PR_READY_VALUE_OPTS.has(name) && eq === -1) {
        i += 2;
        continue;
      }
      i++;
      continue;
    }
    if (positional === undefined) positional = t;
    i++;
  }
  return { undo, head, repo, positional };
}

/** `--draft`/`-d` (create) and `--undo` (ready) are WIP exemptions and apply BEFORE the
 * repository-mismatch check — a draft or an un-readying is never a "done" claim, regardless of
 * which repo it targets. `--repo`/`-R` given after the subcommand is folded into the mismatch
 * check the same as a leading `gh --repo`/`-R` (the per-subcommand value wins when both appear). */
function checkGh(rest, state) {
  const sub = rest[0]?.text;
  const sub2 = rest[1]?.text;
  const args = rest.slice(2);
  if (!(sub === "pr" && (sub2 === "create" || sub2 === "ready"))) return null;
  if (sub2 === "create") {
    const { draft, head, repo } = parseGhPrCreateArgs(args);
    if (draft) return null; // a draft is WIP
    const mismatch = repoMismatch({ ...state, repoOverride: repo ?? state.repoOverride });
    if (mismatch) return mismatch;
    return checkPrCreate(head, state);
  }
  // sub2 === "ready"
  const { undo, head, repo, positional } = parseGhPrReadyArgs(args);
  if (undo) return null;
  const mismatch = repoMismatch({ ...state, repoOverride: repo ?? state.repoOverride });
  if (mismatch) return mismatch;
  return checkPrReady(head ?? positional, state);
}

// git push options that take a value (their value must never be mistaken for a refspec).
const PUSH_VALUE_OPTS = new Set(["-o", "--push-option", "--repo", "--receive-pack", "--exec"]);

/** Split `git push` args (after the leading `push` token) into refspecs, skipping option tokens
 * (and their values) rather than assuming a fixed position — `-u`/`--set-upstream`,
 * `--force-with-lease` (boolean unless given as `--opt=value`), `-o <value>` and the remote name
 * itself must never be mistaken for a refspec. `--repo`/`--repo=<r>` names the remote itself (as
 * `git push --repo=<repository> [<refspec>...]`), so when present there is no separate positional
 * remote to drop — every positional is a refspec. */
function parsePush(rest) {
  let i = 0;
  let deleteFlag = false;
  let sawRepo = false;
  const positionals = [];
  while (i < rest.length) {
    const t = rest[i].text;
    if (t === "--delete" || t === "-d") {
      deleteFlag = true;
      i++;
      continue;
    }
    if (t.startsWith("-")) {
      const eq = t.indexOf("=");
      const name = eq === -1 ? t : t.slice(0, eq);
      if (name === "--repo") sawRepo = true;
      if (PUSH_VALUE_OPTS.has(name) && eq === -1) {
        i += 2;
        continue;
      }
      i++;
      continue;
    }
    positionals.push(t);
    i++;
  }
  return { refspecs: sawRepo ? positionals : positionals.slice(1), deleteFlag }; // positionals[0] is the remote, unless --repo already named it
}

function stripRefPrefix(s) {
  return s.replace(/^refs\/heads\//, "");
}

/** A refspec's {src, dst} for the evidence check, or null when it deletes (never denied):
 * `src:dst` (dst drives the PR lookup, src is the tree to stamp — `HEAD:branch` stamps HEAD),
 * a bare `branch` is `branch:branch`, `:dst` or `--delete` deletes and updates nothing. A leading
 * `+` force marker is stripped from the whole spec — it never changes which branch is meant. Only
 * the DESTINATION gets its `refs/heads/` prefix stripped (it drives `gh pr list --head <dst>`,
 * which wants the bare branch name — `--head refs/heads/feature` matches nothing). The SOURCE stays
 * exactly as the user wrote it: stripping `refs/heads/` there would resolve `<name>^{tree}` via
 * git's plain revision lookup, which prefers `refs/tags/<name>` over `refs/heads/<name>` — silently
 * stamping a same-named tag's tree instead of the branch actually being pushed. */
function refspecTarget(spec, deleteFlag) {
  if (deleteFlag) return null;
  if (spec.startsWith("+")) spec = spec.slice(1);
  const idx = spec.indexOf(":");
  if (idx === -1) {
    return { src: spec, dst: stripRefPrefix(spec) };
  }
  const rawSrc = spec.slice(0, idx);
  if (rawSrc === "") return null; // `:dst` deletion
  return { src: rawSrc, dst: stripRefPrefix(spec.slice(idx + 1)) };
}

function checkPushTarget(src, dst, cwd, gitPrefix) {
  const gh = spawnSync("gh", ["pr", "list", "--head", dst, "--state", "open", "--json", "isDraft,number"], {
    cwd: resolveGhCwd(gitPrefix, cwd),
    encoding: "utf8",
    timeout: 5000,
  });
  if (gh.status !== 0 || !gh.stdout) return null; // offline/error: fail-open
  let prs;
  try {
    prs = JSON.parse(gh.stdout);
  } catch {
    return null;
  }
  const openNonDraft = Array.isArray(prs) ? prs.find((p) => p.isDraft === false) : null;
  if (!openNonDraft) return null;
  const { ok } = hasFreshStamp(gitPrefix, src);
  if (ok) return null;
  return `check-evidence: branch ${dst} has open PR #${openNonDraft.number}: a pushed fix is a done claim — re-run \`make check-local\` on this exact tree first (DoD §K).`;
}

function currentBranch(gitPrefix) {
  const r = spawnSync("git", [...gitPrefix, "rev-parse", "--abbrev-ref", "HEAD"], { encoding: "utf8" });
  return r.status === 0 ? r.stdout.trim() : null;
}

function checkGit(rest, state) {
  if (rest[0]?.text !== "push") return null;
  const flags = rest.slice(1);
  if (flags.some((t) => t.text === "--dry-run" || t.text === "-n")) return null;
  const { refspecs, deleteFlag } = parsePush(flags);
  const specs = refspecs.length > 0 ? refspecs : [null]; // no refspec: current branch
  for (const spec of specs) {
    const target =
      spec === null ? { src: "HEAD", dst: currentBranch(state.gitPrefix) } : refspecTarget(spec, deleteFlag);
    if (!target || !target.dst) continue; // deletion, or current branch unresolvable
    const reason = checkPushTarget(target.src, target.dst, state.cwd, state.gitPrefix);
    if (reason) return reason;
  }
  return null;
}

function stripHeredocs(cmd) {
  const lines = cmd.split("\n");
  const out = [];
  let terminator = null;
  for (const line of lines) {
    if (terminator !== null) {
      if (line.trim() === terminator) terminator = null;
      continue;
    }
    out.push(line);
    const m = line.match(/<<-?\s*(['"]?)([A-Za-z_][A-Za-z0-9_]*)\1/);
    if (m) terminator = m[2];
  }
  return out.join("\n");
}

export function evaluateCommand(cmd, startCwd) {
  const state = { cwd: startCwd };
  return evaluateInner(stripHeredocs(cmd), state);
}

let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (c) => (input += c));
process.stdin.on("end", () => {
  let payload;
  try {
    payload = JSON.parse(input);
  } catch {
    process.exit(0);
  }
  if (payload?.tool_name !== "Bash") process.exit(0);
  if (process.env.BRAWLER_CHECK_EVIDENCE_OFF === "1") process.exit(0);
  const cmd = payload?.tool_input?.command ?? "";
  const startCwd = typeof payload?.cwd === "string" && payload.cwd !== "" ? payload.cwd : process.cwd();
  let reason;
  try {
    reason = evaluateCommand(cmd, startCwd);
  } catch {
    process.exit(0); // fail-open on any unexpected parse/git error
  }
  if (!reason) process.exit(0);
  process.stdout.write(
    JSON.stringify({
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason: reason,
      },
    }),
  );
  process.exit(0);
});
