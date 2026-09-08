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
// hook's env — it does not disable anything).
//
// Known approximations — a review, not this hook's job: a script invoked by
// path (`bash scripts/x.sh`) is not opened and inspected; a GraphQL/REST body
// read from a `--input`/`-F @file` file is not read; a `||` chain is
// flattened the same as `;`/`&&` — every segment is evaluated as if it always
// ran, since real shell exit-status propagation isn't tracked; conditional
// chains fall back to the live branch — a `||` anywhere in the command
// disables the chain's own checkout/switch-assumed branch, so branch rules
// are judged against the real `rev-parse` branch instead of an assumption
// that may never have run. Nobody should mistake
// a green run here for a proof — it is defence-in-depth for the written
// rule (CLAUDE.md), not a substitute for it.
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

/** Whether any token is a bare `-<letter>`, or a bundle of single-letter short flags
 * containing `<letter>` (`-fu`, `-an`, `-SW`). Only matches a single-dash, all-letters
 * token, so a long option (`--force`) or an operand value (`HEAD~1`, a quoted string)
 * never false-positives through this path. */
function hasBundledShortFlag(toks, letter) {
  const re = new RegExp(`^-[a-zA-Z]*${letter}[a-zA-Z]*$`);
  return toks.some((t) => re.test(t.text));
}

// Options that consume the NEXT token as a value, per subcommand — that value must never be
// re-read as a flag by hasExact/hasBundledShortFlag (a `-m "--no-verify"` commit message is
// text, not the flag `--no-verify`). Only options relevant to this hook's own flag checks are
// listed; an inline `--opt=value` form needs no entry since its value never becomes its own
// token. checkout/switch keep their own branch-operand handling (they must see the value, to
// track the branch target) and are intentionally not covered here.
const VALUE_FLAGS = {
  restore: new Set(["-s", "--source"]),
  clean: new Set(["-e", "--exclude"]),
  push: new Set(["-o", "--push-option", "--repo", "--receive-pack"]),
  commit: new Set(["-m", "--message", "-F", "--file", "-c", "-C", "--author", "--date"]),
};

/** Drop the operand token that follows a value-consuming option (per `VALUE_FLAGS`) — it is
 * never a flag, however much it looks like one — before any exact/bundle flag check runs. */
function stripValueOperands(rest, valueFlags) {
  const out = [];
  for (let i = 0; i < rest.length; i++) {
    const t = rest[i];
    out.push(t);
    const name = t.text.includes("=") ? t.text.slice(0, t.text.indexOf("=")) : t.text;
    if (valueFlags.has(name) && !t.text.includes("=") && i + 1 < rest.length) i++; // skip its value
  }
  return out;
}

/** Live branch of `cwd` via `rev-parse`; null when unknown (unknown never denies). */
function liveBranch(cwd) {
  const r = spawnSync("git", ["-C", cwd, "rev-parse", "--abbrev-ref", "HEAD"], { encoding: "utf8" });
  if (r.status !== 0) return null;
  return r.stdout.trim() || null;
}

/** Whether a master-sensitive mutation may run on `master`: every REACHABLE branch state counts.
 * A checkout/switch earlier in the same chain (same cwd) sets the assumed branch; in a chain with
 * an unquoted `||` the shell may have skipped that checkout, so the live branch stays reachable
 * too — a mutation is denied when ANY reachable state is master (never "fall back to one"). */
function mayBeOnMaster(state) {
  if (state.chainBranch === "master") return true;
  if (state.chainBranch !== null && !state.conditional) return false;
  return liveBranch(state.cwd) === "master";
}

/** Whether `cmd` contains an unquoted `||` anywhere (top-level or inside a nested eval/$()/
 * backtick span, since those are literal substrings of the same raw text) — a real shell only
 * runs the right side conditionally, which the segment-flattening in this file cannot track. */
function hasUnquotedOr(cmd) {
  let quote = null;
  for (let i = 0; i < cmd.length; i++) {
    const c = cmd[i];
    if (c === "\\" && quote !== "'" && i + 1 < cmd.length) {
      i++;
      continue;
    }
    if (quote) {
      if (c === quote) quote = null;
      continue;
    }
    if (c === '"' || c === "'") {
      quote = c;
      continue;
    }
    if (c === "|" && cmd[i + 1] === "|") return true;
  }
  return false;
}

// --- git subcommand rules (each returns a reason string to deny, or null to allow) ---

function checkCheckout(rest, state) {
  if (hasExact(rest, ["--"])) return "git checkout with `--` restores tracked files from the index/branch";
  if (hasExact(rest, ["-f", "--force", "--discard-changes"]) || hasBundledShortFlag(rest, "f")) {
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
  if (hasExact(rest, ["-f", "--force", "--discard-changes"]) || hasBundledShortFlag(rest, "f")) {
    return "git switch --force/-f/--discard-changes discards tracked changes";
  }
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
  const flags = stripValueOperands(rest, VALUE_FLAGS.restore);
  const staged = hasExact(flags, ["--staged"]) || hasBundledShortFlag(flags, "S");
  const worktree = hasExact(flags, ["--worktree"]) || hasBundledShortFlag(flags, "W");
  if (staged && !worktree) return null;
  return "git restore without `--staged` (or combined with `--worktree`) can discard working-tree changes";
}

function checkStash(rest) {
  const sub = rest.find((t) => !t.text.startsWith("-"))?.text;
  if (sub === "list" || sub === "show") return null;
  return "git stash (any subcommand but list/show, including the implicit push) can discard uncommitted work";
}

function checkReset(rest, state) {
  if (hasExact(rest, ["--hard", "--merge", "--keep"])) {
    return "git reset --hard/--merge/--keep discards working-tree/index changes";
  }
  // `git reset [<mode>] [<commit>]` moves HEAD; `git reset [<commit>] -- <paths>` or a plain
  // `git reset <existing-path>` only unstages and never moves HEAD. Only a history-rewrite on
  // `master` (an actual commit-ish operand, HEAD moving) is this rule's concern — an ordinary
  // unstage (`git reset`, `git reset <path>`) stays allowed on any branch.
  if (!mayBeOnMaster(state)) return null;
  // Anything after `--` is a path list (git's own syntax): `git reset <commit> -- <paths>` never
  // moves HEAD regardless of what precedes `--` — even a literal `HEAD` there is just the
  // tree-ish paths are reset from, not a HEAD move. So `--` alone settles this as the path form.
  if (rest.some((t) => t.text === "--")) return null;
  const positionals = rest.filter((t) => !t.text.startsWith("-"));
  if (positionals.length === 0) return null;
  if (positionals.length === 1 && existingPath(state.cwd, positionals[0].text)) return null;
  return "git reset with a commit-ish operand moves HEAD directly on `master`, rewriting its history";
}

function checkClean(rest) {
  const flags = stripValueOperands(rest, VALUE_FLAGS.clean);
  if (hasExact(flags, ["-n", "--dry-run"]) || hasBundledShortFlag(flags, "n")) return null;
  return "git clean without -n/--dry-run deletes untracked files";
}

// commit's value-taking short letters (git's own list: -m/-F/-c/-C each take a value, attached
// to the same token — `-mmsg` — or as the next token — `-m msg`). A bundle of boolean short
// flags may end in one of these (`-am msg`, `-amsg`) per normal getopt-style bundling.
const COMMIT_VALUE_SHORT_LETTERS = "mFcC";

/** Split each single-dash all-letter commit token at its first value-taking letter (if any):
 * keep only the boolean-flag letters before it (dropping the value letter itself and anything
 * after), and consume a separate-token value when the letter carries none attached. Must run
 * BEFORE any bundled-flag scan (`-n`/no-verify) — otherwise a message attached to `-m` (e.g.
 * `-mnonverify`) gets misread as a bundled `-n` flag hiding inside the "value". */
const COMMIT_VALUE_LONG = new Set([
  "--message", "--file", "--author", "--date", "--reuse-message", "--reedit-message", "--fixup",
  "--squash", "--trailer", "--cleanup", "--template", "--pathspec-from-file",
]);

/** ONE pass over commit arguments, consuming every option value exactly once: a long value option
 * without `=` eats the next token; a short bundle keeps its boolean letters up to the first
 * value letter (`-am x` → `-a`, value `x`; `-mnonverify` → nothing, attached value). Only the
 * survivors are flag-checked, so a forbidden flag can never be eaten as a "value" and a value can
 * never be misread as a flag (astra r4 #1 / r3 #24). */
function commitFlagTokens(rest) {
  const out = [];
  for (let i = 0; i < rest.length; i++) {
    const t = rest[i];
    if (t.text === "--") break; // pathspecs follow
    if (t.text.startsWith("--")) {
      if (COMMIT_VALUE_LONG.has(t.text) && i + 1 < rest.length) i++; // separate value
      else out.push(t); // `--opt=value` carries its own value; boolean long flags stay
      continue;
    }
    if (/^-[a-zA-Z]+/.test(t.text)) {
      const letters = t.text.slice(1);
      const valueIdx = [...letters].findIndex((ch) => COMMIT_VALUE_SHORT_LETTERS.includes(ch));
      if (valueIdx === -1) {
        out.push(t);
        continue;
      }
      const boolPrefix = letters.slice(0, valueIdx);
      if (boolPrefix) out.push({ ...t, text: `-${boolPrefix}` });
      const attachedValue = letters.slice(valueIdx + 1);
      if (!attachedValue && i + 1 < rest.length) i++; // no attached value: next token is it
      continue;
    }
    out.push(t);
  }
  return out;
}

function checkCommit(rest, state) {
  const flags = commitFlagTokens(rest);
  if (hasExact(flags, ["--no-verify"]) || hasBundledShortFlag(flags, "n")) {
    return "git commit --no-verify/-n (incl. combined short flags) skips the commit-msg gate";
  }
  if (mayBeOnMaster(state)) return "git commit directly on `master` bypasses the PR/CI gate";
  return null;
}

function checkPush(rest, state) {
  const flags = stripValueOperands(rest, VALUE_FLAGS.push);
  // -n is always a dry run, even bundled with other short flags (e.g. `-fn`) — nothing mutates.
  if (hasExact(flags, ["--dry-run"]) || hasBundledShortFlag(flags, "n")) return null;
  if (hasExact(flags, ["--no-verify"])) return "git push --no-verify skips the commit-msg gate";
  if (
    hasExact(flags, ["--force", "--force-if-includes", "--all", "--mirror", "--delete"]) ||
    hasBundledShortFlag(flags, "f")
  ) {
    return "git push --force/--force-if-includes/--all/--mirror/--delete (incl. combined short flags) rewrites or wipes remote refs";
  }
  if (hasPrefix(flags, ["--force-with-lease"])) return "git push --force-with-lease rewrites remote refs";
  // `--repo`/`--receive-pack`/etc already had their value tokens stripped above (`flags`); use
  // it (not the raw `rest`) for positionals so a `--repo`'s value never gets mistaken for one. A
  // `--repo`/`--repo=` option supplies the remote itself, so EVERY remaining positional is a
  // refspec — only without it does the first positional double as the remote.
  const hasRepoOption = flags.some((t) => t.text === "--repo" || t.text.startsWith("--repo="));
  const positionals = flags.filter((t) => !t.text.startsWith("-"));
  const refspecs = hasRepoOption ? positionals : positionals.slice(1); // git push [<repository>] [<refspec>...]
  for (const r of refspecs) {
    // A lone `:` is the empty/"matching" refspec — historically used to push every branch whose
    // name matches on both ends. It can update `master` regardless of the current branch.
    if (r.text === ":") return "git push refspec `:` (matching refspec) can push to any ref, including `master`";
    if (r.text.startsWith("+")) return `git push refspec \`${r.text}\` forces an update (leading +)`;
    const dest = r.text.includes(":") ? r.text.split(":").slice(1).join(":") : r.text;
    const normalized = dest.replace(/^refs\/heads\//, "");
    if (normalized === "master" || normalized === "*") return `git push refspec \`${r.text}\` targets \`master\``;
    // A bare `HEAD` (or an empty destination, `HEAD:`) pushes the current branch under its own name.
    if ((normalized === "HEAD" || normalized === "") && mayBeOnMaster(state)) {
      return `git push refspec \`${r.text}\` pushes the current branch (\`HEAD\`) while on \`master\``;
    }
  }
  if (refspecs.length === 0) {
    if (mayBeOnMaster(state)) return "git push with no refspec while on `master` pushes master";
    // Off master a bare `git push`/`git push <remote>` still resolves to a real destination via
    // the branch's own push/upstream tracking — ask git directly rather than guessing at
    // push.default from static analysis.
    const dest = resolvePushDestination(state.cwd);
    if (dest && /\/master$/.test(dest)) {
      return `git push with no refspec resolves to \`${dest}\` (via @{push}/@{upstream}) — pushes master`;
    }
  }
  return null;
}

/** The real destination a bare `git push`/`git push <remote>` would resolve to, as
 * `<remote>/<branch>` — `@{push}` first (what push.default actually uses), falling back to
 * `@{upstream}` (e.g. push.default=simple can't resolve `@{push}` when the branch name and its
 * upstream's differ, even though the push would still go to that upstream). Unresolvable
 * (detached HEAD, no tracking configured) returns null — this hook never guesses. */
function resolvePushDestination(cwd) {
  for (const ref of ["@{push}", "@{upstream}"]) {
    const r = spawnSync("git", ["-C", cwd, "rev-parse", "--abbrev-ref", "--symbolic-full-name", ref], {
      encoding: "utf8",
    });
    if (r.status === 0 && r.stdout.trim()) return r.stdout.trim();
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

/** Git global options consumed before the subcommand: -C (cwd), -c (hooksPath detection), --git-dir=, --work-tree=, --no-pager/-p.
 * `cwd` is the chain's current directory; `-C <dir>` only scopes THIS git invocation (git's own
 * semantics — it never changes the shell's cwd), so the resolved cwd is returned rather than
 * mutating anything, and the caller must not let it leak into the next chain segment. */
function parseGitGlobals(args, cwd) {
  let i = 0;
  let hooksPathValue;
  while (i < args.length) {
    const t = args[i].text;
    if (t === "-C") {
      cwd = resolve(cwd, args[i + 1]?.text ?? ".");
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
    if (t === "--namespace" || t === "--super-prefix" || t === "--config-env" || t === "--exec-path") {
      i += 2; // value in the next token
      continue;
    }
    // Any other dash token before the subcommand is a boolean git global (`--no-pager`,
    // `--no-optional-locks`, `--literal-pathspecs`, `--bare`, `--exec-path=…`, …): consume it,
    // never mistake it for the subcommand (astra r5: `git --no-optional-locks reset --hard`).
    if (t.startsWith("-")) {
      i += 1;
      continue;
    }
    break;
  }
  return { args: args.slice(i), hooksPathValue, cwd };
}

function checkGit(rawArgs, state) {
  const { args, hooksPathValue, cwd: invocationCwd } = parseGitGlobals(rawArgs, state.cwd);
  if (hooksPathValue !== undefined) {
    return `git -c core.hooksPath=${hooksPathValue} bypasses this repo's hooks`;
  }
  // A local view scoped to this one invocation: `-C` changes ITS cwd only (never the chain's —
  // git itself never touches the shell's directory), while a branch a checkCheckout/checkSwitch
  // assumes here must still carry forward into the next chain segment — but only when this
  // invocation's cwd IS the chain's cwd. A `-C ../other` checkout is a different worktree/repo;
  // its assumed branch must never leak into a later segment that runs in the chain's own cwd.
  const localState = {
    cwd: invocationCwd,
    // A `-C ../other` invocation is another worktree: it neither inherits nor exports the
    // chain's assumed branch — its own live branch is resolved instead.
    chainBranch: invocationCwd === state.cwd ? state.chainBranch : null,
    conditional: state.conditional,
  };
  const subcommand = args[0]?.text;
  const rest = args.slice(1);
  let reason;
  switch (subcommand) {
    case "checkout":
      reason = checkCheckout(rest, localState);
      break;
    case "switch":
      reason = checkSwitch(rest, localState);
      break;
    case "restore":
      reason = checkRestore(rest);
      break;
    case "stash":
      reason = checkStash(rest);
      break;
    case "reset":
      reason = checkReset(rest, localState);
      break;
    case "clean":
      reason = checkClean(rest);
      break;
    case "commit":
      reason = checkCommit(rest, localState);
      break;
    case "push":
      reason = checkPush(rest, localState);
      break;
    case "worktree":
      reason = checkWorktree(rest);
      break;
    default:
      reason =
        MASTER_ONLY_DENY_SUBS.has(subcommand) && mayBeOnMaster(localState)
          ? `git ${subcommand} directly on \`master\` bypasses the PR/CI gate`
          : null;
  }
  // Only copy the assumed branch back when this invocation ran in the chain's own cwd — a
  // `-C ../other` invocation's branch belongs to that other worktree, never this one.
  if (invocationCwd === state.cwd) state.chainBranch = localState.chainBranch;
  return reason;
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
    // Options that take a separate value token — never the endpoint operand. `-f`/`-F`/`--field`/
    // `--raw-field`/`--input` also imply an unwritten `POST` (gh's own default) when no explicit
    // `-X`/`--method` is given, so their presence is tracked too.
    const VALUE_OPTIONS = new Set([
      "-f", "--field", "-F", "--raw-field", "-H", "--header", "-q", "--jq", "-t", "--template",
      "--input", "--hostname", "--cache",
    ]);
    const FIELD_OPTIONS = new Set(["-f", "--field", "-F", "--raw-field", "--input"]);
    let method;
    let hasFieldFlag = false;
    let pathArg;
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
      const eq = t.indexOf("=");
      const name = eq === -1 ? t : t.slice(0, eq);
      if (VALUE_OPTIONS.has(name)) {
        if (FIELD_OPTIONS.has(name)) hasFieldFlag = true;
        if (eq === -1) i++; // consume the separate-token value; the `name=value` form has none
        continue;
      }
      if (t.startsWith("-")) continue; // a boolean flag (-i, -p, --silent, …)
      if (pathArg === undefined) pathArg = t; // first non-option, non-option-value token
    }
    pathArg = pathArg ?? "";
    const effectiveMethod = method ?? (hasFieldFlag ? "POST" : undefined);
    if (pathArg === "graphql") {
      if (rest.some((t) => /mergePullRequest|updateRepository|updateBranchProtectionRule/.test(t.text))) {
        return "gh api graphql mutation (mergePullRequest/updateRepository/updateBranchProtectionRule) — owner-only";
      }
      return null;
    }
    if (/pulls\/\d+\/merge$/.test(pathArg)) {
      return `gh api ${pathArg} merges a PR — owner-only`;
    }
    if (effectiveMethod && /^(POST|PUT|PATCH|DELETE)$/i.test(effectiveMethod) && /\/releases(\/|$)/.test(pathArg)) {
      return `gh api ${effectiveMethod} ${pathArg} mutates a release — owner-only`;
    }
    if (effectiveMethod && /^(PUT|PATCH|DELETE)$/i.test(effectiveMethod)) {
      if (/^repos\/[^/]+\/[^/]+$/.test(pathArg) || pathArg.includes("/rulesets") || /branches\/[^/]+\/protection/.test(pathArg)) {
        return `gh api ${effectiveMethod} ${pathArg} mutates repo settings/branch protection — owner-only`;
      }
    }
  }
  return null;
}

/** Extract the inner text of every `$( … )` (paren-depth-aware) and `` ` … ` `` span, tracking
 * single- vs double-quote state (not otherwise quote-aware — good enough to stop a command
 * hidden in a substitution from being invisible to the segment-based analysis above). A span
 * inside SINGLE quotes is inert in bash (never expanded) and is skipped; one inside double
 * quotes or unquoted still executes and is still extracted. */
function extractSubstitutions(cmd) {
  const spans = [];
  let quote = null; // null | "'" | '"'
  for (let i = 0; i < cmd.length; i++) {
    const c = cmd[i];
    if (c === "\\" && quote !== "'" && i + 1 < cmd.length) {
      i++;
      continue;
    }
    if (quote === null && (c === "'" || c === '"')) {
      quote = c;
      continue;
    }
    if (quote === c) {
      quote = null;
      continue;
    }
    if (quote === "'") continue; // single-quoted text is inert: $()/`` never expand inside it
    if (c === "$" && cmd[i + 1] === "(") {
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
    if (c === "`") {
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

const SHELL_WRAPPERS = new Set(["bash", "sh", "zsh", "dash"]);

/** Strip a leading `(`/`{` from the segment's first token and a trailing `)`/`}`/`;` from its
 * last (subshell/group syntax splitSegments never splits on: `(git reset --hard)`, `{ git
 * stash; }`) — but never touch a QUOTED token, so a message ending in a literal `)` is untouched. */
function stripSubshellDelims(raw) {
  if (raw.length === 0) return raw;
  const out = raw.map((t) => ({ ...t }));
  const first = out[0];
  if (!first.quoted) first.text = first.text.replace(/^[({]+/, "");
  const last = out[out.length - 1];
  if (!last.quoted) last.text = last.text.replace(/[)};]+$/, "");
  return out.filter((t) => t.text !== "" || t.quoted);
}

/** If the token at `idx` is a shell wrapper (`bash`/`sh`/`zsh`/`dash`, incl. by absolute path)
 * invoked with `-c`/`--command`, return its inline script body text (or null if the flag is
 * present with no body); otherwise undefined ("not a wrapper here"). Deliberately independent of
 * one-heavy-build-classify.mjs's own `wrappedScript`, which only strips ITS OWN prefix list
 * (missing `command`) — this runs after THIS hook's full prefix strip below, so `command bash -c
 * "…"` unwraps correctly regardless of what the imported helper recognizes upstream. */
function shellWrapperBody(raw, idx) {
  const head = raw[idx]?.text;
  if (!head) return undefined;
  if (!SHELL_WRAPPERS.has(head.replace(/^.*\//, ""))) return undefined;
  for (let j = idx + 1; j < raw.length; j++) {
    const t = raw[j].text;
    if (/^-[a-zA-Z]*c[a-zA-Z]*$/.test(t)) return raw[j + 1] ? raw[j + 1].text : null;
    if (!t.startsWith("-")) return undefined; // `bash script.sh` — not an inline body
  }
  return undefined;
}

/** Strip transparent prefixes (leading VAR=val…, command/env/rtk/nix -c|--command/timeout/nice/time),
 * tracking `cd <dir>` as cwd state; unwraps a trailing shell-wrapper body and `eval <rest>`;
 * dispatches to git/gh. */
function processLeafSegment(segmentText, state, depth) {
  const raw = stripSubshellDelims(tokenizeRaw(segmentText));
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
    state.chainBranch = null; // branch knowledge from before the `cd` is for a different worktree
    return null;
  }
  const wrapperBody = shellWrapperBody(raw, i);
  if (wrapperBody !== undefined) {
    return wrapperBody ? evaluateInner(wrapperBody, state, depth + 1) : null;
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
  const state = { cwd: startCwd, chainBranch: null, conditional: hasUnquotedOr(cmd) };
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
  if (process.env.BRAWLER_GIT_BOUNDARIES_OFF === "1") {
    process.stderr.write("git-boundaries: disabled by BRAWLER_GIT_BOUNDARIES_OFF=1\n");
    process.exit(0);
  }
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
