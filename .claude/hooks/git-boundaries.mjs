#!/usr/bin/env node
// git-boundaries PreToolUse hook (owner 2026-09-08, hard gate G1 of the tests
// audit wave 2; ADR 0038 amendment). Denies a Bash `git`/`gh` invocation that
// would discard tracked work or land a mutation on `master`/the forge outside
// this repo's written rule (CLAUDE.md § Working Rules — T1). The written rule
// stays the rule's home; this hook is defence-in-depth for Claude's Bash tool
// only — Codex peers are outside it.
//
// Protocol matches one-heavy-build.mjs: JSON on stdin, a deny writes
// {hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"deny",
// permissionDecisionReason}} to stdout and exits 0; an allow writes nothing
// and exits 0. Malformed input / a non-Bash tool always allow (fail open).
//
// Parsing reuses splitSegments/wrappedScript (chain + shell-wrapper handling)
// from one-heavy-build-classify.mjs unchanged, but tokenizes with the
// value-preserving tokenizeRaw — stripQuotes would erase a quoted refspec
// like "HEAD:master" or a quoted flag like "--hard", which this hook must
// still see. Prefix stripping tracks `cd <dir>` as cwd state across the
// chain (not just a local strip) and skips `command`, `env [-u X|K=V]…`,
// `rtk [proxy]`, `timeout`, `nice`, `time`, and an absolute git/gh path.
//
// Escape: BRAWLER_GIT_BOUNDARIES_OFF=1 read from THIS process's own env only
// (an inline `VAR=1 git …` prefix is stripped as a command token, never this
// hook's env — it does not disable anything). Test seam: BRAWLER_GIT_BRANCH_
// OVERRIDE is honoured only together with BRAWLER_GIT_BOUNDARIES_TEST=1.
//
// Out of reach — a review, not this hook's job: a script invoked by path
// (`bash scripts/x.sh`) is not opened and inspected; `push.default`/upstream
// config that maps a bare `git push` to `master` from a feature branch is
// invisible to static analysis; a GraphQL/REST body read from a `--input`/
// `-F @file` file is not read. Nobody should mistake a green run here for a
// proof — it is defence-in-depth for the written rule (CLAUDE.md), not a
// substitute for it.
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { splitSegments, wrappedScript, tokenizeRaw } from "./one-heavy-build-classify.mjs";

const RULE_HOME =
  "CLAUDE.md § Working Rules / engineering-workflow.md § Local Developer Commands (hard gate G1, owner 2026-09-08)";

const ASSIGNMENT_RE = /^[A-Za-z_][A-Za-z0-9_]*=/;

/** Flatten a command into leaf segments in execution order, unwrapping `bash -c "…"`-style wrappers recursively. */
function collectLeafSegments(cmd) {
  const out = [];
  for (const segment of splitSegments(cmd)) {
    const inner = wrappedScript(segment);
    if (inner !== null) out.push(...collectLeafSegments(inner));
    else out.push(segment);
  }
  return out;
}

function existingPath(cwd, token) {
  try {
    return existsSync(resolve(cwd, token));
  } catch {
    return false;
  }
}

const hasExact = (toks, names) => toks.some((t) => names.includes(t.text));
const hasPrefix = (toks, prefixes) => toks.some((t) => prefixes.some((p) => t.text.startsWith(p)));

/** Current branch: a checkout/switch earlier in the same chain wins; else a live `rev-parse`; unknown never denies. */
function resolveBranch(state) {
  if (state.chainBranch !== null) return state.chainBranch;
  if (process.env.BRAWLER_GIT_BOUNDARIES_TEST === "1" && Object.hasOwn(process.env, "BRAWLER_GIT_BRANCH_OVERRIDE")) {
    return process.env.BRAWLER_GIT_BRANCH_OVERRIDE;
  }
  const r = spawnSync("git", ["-C", state.cwd, "rev-parse", "--abbrev-ref", "HEAD"], { encoding: "utf8" });
  if (r.status !== 0) return null;
  return r.stdout.trim() || null;
}

// --- git subcommand rules (each returns a reason string to deny, or null to allow) ---

function checkCheckout(rest, state) {
  if (hasExact(rest, ["--"])) return "git checkout with `--` restores tracked files from the index/branch";
  if (hasExact(rest, ["-f", "--force", "--discard-changes"])) {
    return "git checkout --force/-f/--discard-changes discards tracked changes";
  }
  let branchOperandIdx = -1;
  for (let i = 0; i < rest.length; i++) {
    if (["-b", "-B", "--orphan"].includes(rest[i].text)) {
      branchOperandIdx = i + 1;
      break;
    }
  }
  let target = null;
  for (let i = 0; i < rest.length; i++) {
    if (i === branchOperandIdx) {
      target = rest[i].text;
      continue;
    }
    const t = rest[i].text;
    if (t.startsWith("-")) continue;
    if (t === "." || existingPath(state.cwd, t)) {
      return `git checkout \`${t}\` is an existing path under cwd — restores a tracked file outside \`--\``;
    }
    if (target === null) target = t;
  }
  if (target) state.chainBranch = target;
  return null;
}

function checkSwitch(rest, state) {
  let branchOperandIdx = -1;
  for (let i = 0; i < rest.length; i++) {
    if (["-c", "-C"].includes(rest[i].text)) {
      branchOperandIdx = i + 1;
      break;
    }
  }
  let target = null;
  for (let i = 0; i < rest.length; i++) {
    if (i === branchOperandIdx) {
      target = rest[i].text;
      continue;
    }
    const t = rest[i].text;
    if (t.startsWith("-")) continue;
    if (target === null) target = t;
  }
  if (target) state.chainBranch = target;
  return null;
}

function checkRestore(rest) {
  const staged = hasExact(rest, ["--staged", "-S"]);
  const worktree = hasExact(rest, ["--worktree", "-W"]);
  if (staged && !worktree) return null;
  return "git restore without `--staged` (or combined with `--worktree`) can discard working-tree changes";
}

function checkStash(rest) {
  const sub = rest.find((t) => !t.text.startsWith("-"))?.text;
  if (sub === "list" || sub === "show") return null;
  return "git stash (any subcommand but list/show, including the implicit push) can discard uncommitted work";
}

function checkReset(rest) {
  if (hasExact(rest, ["--hard", "--merge", "--keep"])) {
    return "git reset --hard/--merge/--keep discards working-tree/index changes";
  }
  return null;
}

function checkClean(rest) {
  if (hasExact(rest, ["-n", "--dry-run"])) return null;
  return "git clean without -n/--dry-run deletes untracked files";
}

// Combined short flags (`-an`, `-nm`, `-fu`) bundle boolean options into one token — a bare
// exact-match list misses them. Matched only against single-dash tokens (`^-[a-zA-Z]*`), so
// long options (`--no-verify`, `--force-with-lease`) never false-positive through this path.
const COMMIT_NO_VERIFY_SHORT_RE = /^-[a-zA-Z]*n[a-zA-Z]*$/;
const PUSH_DRY_RUN_SHORT_RE = /^-[a-zA-Z]*n[a-zA-Z]*$/;
const PUSH_FORCE_SHORT_RE = /^-[a-zA-Z]*f[a-zA-Z]*$/;

function checkCommit(rest, state) {
  if (hasExact(rest, ["--no-verify"]) || rest.some((t) => COMMIT_NO_VERIFY_SHORT_RE.test(t.text))) {
    return "git commit --no-verify/-n (incl. combined short flags) skips the commit-msg gate";
  }
  if (resolveBranch(state) === "master") return "git commit directly on `master` bypasses the PR/CI gate";
  return null;
}

function checkPush(rest, state) {
  // -n is always a dry run, even bundled with other short flags (e.g. `-fn`) — nothing mutates.
  if (hasExact(rest, ["--dry-run"]) || rest.some((t) => PUSH_DRY_RUN_SHORT_RE.test(t.text))) return null;
  if (hasExact(rest, ["--no-verify"])) return "git push --no-verify skips the commit-msg gate";
  if (
    hasExact(rest, ["--force", "--force-if-includes", "--all", "--mirror", "--delete"]) ||
    rest.some((t) => PUSH_FORCE_SHORT_RE.test(t.text))
  ) {
    return "git push --force/--force-if-includes/--all/--mirror/--delete (incl. combined short flags) rewrites or wipes remote refs";
  }
  if (hasPrefix(rest, ["--force-with-lease"])) return "git push --force-with-lease rewrites remote refs";
  const positionals = rest.filter((t) => !t.text.startsWith("-"));
  const refspecs = positionals.slice(1); // git push [<repository>] [<refspec>...] — first positional is the remote
  for (const r of refspecs) {
    if (r.text.startsWith("+")) return `git push refspec \`${r.text}\` forces an update (leading +)`;
    const dest = r.text.includes(":") ? r.text.split(":").slice(1).join(":") : r.text;
    const normalized = dest.replace(/^refs\/heads\//, "");
    if (normalized === "master" || normalized === "*") return `git push refspec \`${r.text}\` targets \`master\``;
    // A bare `HEAD` (or an empty destination, `HEAD:`) pushes the current branch under its own name.
    if ((normalized === "HEAD" || normalized === "") && resolveBranch(state) === "master") {
      return `git push refspec \`${r.text}\` pushes the current branch (\`HEAD\`) while on \`master\``;
    }
  }
  if (refspecs.length === 0 && resolveBranch(state) === "master") {
    return "git push with no refspec while on `master` pushes master";
  }
  return null;
}

function checkWorktree(rest) {
  if (rest[0]?.text === "remove" && hasExact(rest.slice(1), ["-f", "--force"])) {
    return "git worktree remove --force/-f can discard a worktree with uncommitted changes";
  }
  return null;
}

const MASTER_ONLY_DENY_SUBS = new Set(["merge", "rebase", "cherry-pick", "revert", "am", "apply"]);

/** Git global options consumed before the subcommand: -C (cwd), -c (hooksPath detection), --git-dir=, --work-tree=, --no-pager/-p. */
function parseGitGlobals(args, state) {
  let i = 0;
  let hooksPathValue;
  while (i < args.length) {
    const t = args[i].text;
    if (t === "-C") {
      state.cwd = resolve(state.cwd, args[i + 1]?.text ?? ".");
      i += 2;
      continue;
    }
    if (t === "-c") {
      const kv = args[i + 1]?.text ?? "";
      if (kv.split("=")[0] === "core.hooksPath") hooksPathValue = kv.split("=").slice(1).join("=");
      i += 2;
      continue;
    }
    if (t.startsWith("-c") && t.length > 2 && t.includes("=")) {
      const kv = t.slice(2);
      if (kv.split("=")[0] === "core.hooksPath") hooksPathValue = kv.split("=").slice(1).join("=");
      i += 1;
      continue;
    }
    if (t === "--git-dir") {
      i += 2;
      continue;
    }
    if (t.startsWith("--git-dir=")) {
      i += 1;
      continue;
    }
    if (t === "--work-tree") {
      i += 2;
      continue;
    }
    if (t.startsWith("--work-tree=")) {
      i += 1;
      continue;
    }
    if (t === "--no-pager" || t === "-p" || t === "--paginate") {
      i += 1;
      continue;
    }
    break;
  }
  return { args: args.slice(i), hooksPathValue };
}

function checkGit(rawArgs, state) {
  const { args, hooksPathValue } = parseGitGlobals(rawArgs, state);
  if (hooksPathValue !== undefined) {
    return `git -c core.hooksPath=${hooksPathValue} bypasses this repo's hooks`;
  }
  const subcommand = args[0]?.text;
  const rest = args.slice(1);
  switch (subcommand) {
    case "checkout":
      return checkCheckout(rest, state);
    case "switch":
      return checkSwitch(rest, state);
    case "restore":
      return checkRestore(rest);
    case "stash":
      return checkStash(rest);
    case "reset":
      return checkReset(rest);
    case "clean":
      return checkClean(rest);
    case "commit":
      return checkCommit(rest, state);
    case "push":
      return checkPush(rest, state);
    case "worktree":
      return checkWorktree(rest);
    default:
      if (MASTER_ONLY_DENY_SUBS.has(subcommand) && resolveBranch(state) === "master") {
        return `git ${subcommand} directly on \`master\` bypasses the PR/CI gate`;
      }
      return null;
  }
}

function checkGh(args) {
  const sub = args[0]?.text;
  const sub2 = args[1]?.text;
  if (sub === "pr" && sub2 === "merge") return "gh pr merge merges a PR into its base branch — owner-only";
  if (sub === "repo" && ["edit", "delete", "rename", "archive"].includes(sub2)) {
    return `gh repo ${sub2} mutates repository settings — owner-only`;
  }
  if (sub === "release" && ["create", "delete", "edit", "upload"].includes(sub2)) {
    return `gh release ${sub2} mutates a release — owner-only`;
  }
  if (sub === "api") {
    const rest = args.slice(1);
    let method;
    const filtered = [];
    for (let i = 0; i < rest.length; i++) {
      const t = rest[i].text;
      if (t === "-X" || t === "--method") {
        method = rest[i + 1]?.text;
        i++;
        continue;
      }
      if (t.startsWith("--method=")) {
        method = t.slice("--method=".length);
        continue;
      }
      filtered.push(rest[i]);
    }
    const pathArg = filtered.find((t) => !t.text.startsWith("-"))?.text ?? "";
    if (pathArg === "graphql") {
      if (rest.some((t) => /mergePullRequest|updateRepository|updateBranchProtectionRule/.test(t.text))) {
        return "gh api graphql mutation (mergePullRequest/updateRepository/updateBranchProtectionRule) — owner-only";
      }
      return null;
    }
    if (/pulls\/\d+\/merge$/.test(pathArg)) {
      return `gh api ${pathArg} merges a PR — owner-only`;
    }
    if (method && /^(POST|PUT|PATCH|DELETE)$/i.test(method) && /\/releases(\/|$)/.test(pathArg)) {
      return `gh api ${method} ${pathArg} mutates a release — owner-only`;
    }
    if (method && /^(PUT|PATCH|DELETE)$/i.test(method)) {
      if (/^repos\/[^/]+\/[^/]+$/.test(pathArg) || pathArg.includes("/rulesets") || /branches\/[^/]+\/protection/.test(pathArg)) {
        return `gh api ${method} ${pathArg} mutates repo settings/branch protection — owner-only`;
      }
    }
  }
  return null;
}

/** Extract the inner text of every `$( … )` (paren-depth-aware) and `` ` … ` `` span — simple
 * matching, not quote-aware; good enough to stop a command hidden in a substitution from
 * being invisible to the segment-based analysis above. */
function extractSubstitutions(cmd) {
  const spans = [];
  for (let i = 0; i < cmd.length; i++) {
    if (cmd[i] === "$" && cmd[i + 1] === "(") {
      let depth = 1;
      let j = i + 2;
      while (j < cmd.length && depth > 0) {
        if (cmd[j] === "(") depth++;
        else if (cmd[j] === ")") depth--;
        j++;
      }
      spans.push(cmd.slice(i + 2, depth === 0 ? j - 1 : j));
      i = j - 1;
      continue;
    }
    if (cmd[i] === "`") {
      const end = cmd.indexOf("`", i + 1);
      const stop = end === -1 ? cmd.length : end;
      spans.push(cmd.slice(i + 1, stop));
      i = stop;
    }
  }
  return spans;
}

// Recursion guard for eval/$()/backtick nesting — a realistic command nests a handful of
// levels deep; beyond this it is not worth chasing further (ponytail: raise if a legitimate
// case needs more).
const MAX_EVAL_DEPTH = 8;

/** Strip transparent prefixes (leading VAR=val…, command/env/rtk/nix -c|--command/timeout/nice/time),
 * tracking `cd <dir>` as cwd state; unwraps `eval <rest>`; dispatches to git/gh. */
function processLeafSegment(segmentText, state, depth) {
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
    if (t === "nix") {
      const cIdx = raw.findIndex((tok, idx) => idx > i && (tok.text === "-c" || tok.text === "--command"));
      if (cIdx === -1) break; // not a wrapper this hook understands — `nix` itself is not git/gh anyway
      i = cIdx + 1;
      continue;
    }
    if (t === "timeout") {
      i++;
      while (raw[i] && raw[i].text.startsWith("-")) {
        if (/^(-s|-k|--signal|--kill-after)$/.test(raw[i].text)) i++;
        i++;
      }
      i++; // the duration
      continue;
    }
    if (t === "nice") {
      i++;
      if (raw[i]?.text === "-n") i += 2;
      else if (/^-\d+$/.test(raw[i]?.text ?? "")) i++;
      continue;
    }
    if (t === "time") {
      i++;
      continue;
    }
    break;
  }
  if (raw[i]?.text === "cd" && raw[i + 1]) {
    state.cwd = resolve(state.cwd, raw[i + 1].text);
    return null;
  }
  const head = raw[i]?.text;
  if (!head) return null;
  if (head === "eval") {
    const restText = raw
      .slice(i + 1)
      .map((tok) => tok.text)
      .join(" ");
    return restText ? evaluateInner(restText, state, depth + 1) : null;
  }
  const base = head.replace(/^.*\//, "");
  if (base === "git") return checkGit(raw.slice(i + 1), state);
  if (base === "gh") return checkGh(raw.slice(i + 1));
  return null;
}

/** Evaluate a command string against the given state: its own chain of segments, plus every
 * `eval`/`$()`/backtick span found at this level or nested inside them (depth-capped). */
function evaluateInner(cmd, state, depth) {
  if (depth > MAX_EVAL_DEPTH) return null;
  for (const segment of collectLeafSegments(cmd)) {
    const reason = processLeafSegment(segment, state, depth);
    if (reason) return reason;
  }
  for (const span of extractSubstitutions(cmd)) {
    const reason = evaluateInner(span, state, depth + 1);
    if (reason) return reason;
  }
  return null;
}

/** Evaluate a whole Bash command line; returns a deny reason, or null to allow. cwd/branch state threads across the chain. */
export function evaluateCommand(cmd, startCwd) {
  const state = { cwd: startCwd, chainBranch: null };
  return evaluateInner(cmd, state, 0);
}

let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (c) => (input += c));
process.stdin.on("end", () => {
  let payload;
  try {
    payload = JSON.parse(input);
  } catch {
    process.exit(0); // malformed input: never block
  }
  if (payload?.tool_name !== "Bash") process.exit(0);
  if (process.env.BRAWLER_GIT_BOUNDARIES_OFF === "1") process.exit(0);
  const cmd = payload?.tool_input?.command ?? "";
  const startCwd = typeof payload?.cwd === "string" && payload.cwd !== "" ? payload.cwd : process.cwd();
  const reason = evaluateCommand(cmd, startCwd);
  if (!reason) process.exit(0);
  process.stdout.write(
    JSON.stringify({
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason: `git-boundaries: ${reason} — ${RULE_HOME}`,
      },
    }),
  );
  process.exit(0);
});
