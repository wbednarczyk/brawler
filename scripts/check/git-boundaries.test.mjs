// Guard (hard gate G1, tests audit wave 2, owner 2026-09-08): git-boundaries.sh
// must deny a Bash git/gh invocation that discards tracked work or lands a
// mutation on `master`/the forge outside CLAUDE.md § Working Rules (T1), and
// must never flag a legitimate feature-branch commit/push/merge. Drives the
// hook exactly as Claude Code's PreToolUse machinery does: JSON on stdin,
// read permissionDecision back — same harness shape as one-heavy-build.test.mjs.
import { test, after } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(fileURLToPath(new URL("../..", import.meta.url)));
const HOOK_PATH = path.join(REPO_ROOT, ".claude/hooks/git-boundaries.sh");

function git(args, cwd) {
  const r = spawnSync("git", args, { cwd, encoding: "utf8" });
  if (r.status !== 0) throw new Error(`git ${args.join(" ")} failed in ${cwd}: ${r.stderr}`);
  return r;
}

/** A tmp repo with one committed file (`tracked.txt`), on `master`, then optionally switched to `branch`. */
function makeRepo(branch) {
  const dir = mkdtempSync(path.join(tmpdir(), "git-boundaries-"));
  git(["init", "-q"], dir);
  git(["symbolic-ref", "HEAD", "refs/heads/master"], dir);
  git(["config", "user.email", "t@example.com"], dir);
  git(["config", "user.name", "test"], dir);
  writeFileSync(path.join(dir, "tracked.txt"), "x");
  git(["add", "tracked.txt"], dir);
  git(["commit", "-q", "-m", "init"], dir);
  if (branch !== "master") git(["checkout", "-q", "-b", branch], dir);
  return dir;
}

const MASTER_REPO = makeRepo("master");
const FEATURE_REPO = makeRepo("wave2g/s1-fixture");

/** Item 5's exact recipe: a bare local remote, `master` pushed to it, then a feature branch
 * whose upstream is set to `origin/master` — so a bare `git push`/`git push origin` off master
 * still resolves (via `@{push}`/`@{upstream}`) to a destination that IS master. */
function makePushTrackingRepo() {
  const remoteDir = mkdtempSync(path.join(tmpdir(), "git-boundaries-remote-"));
  git(["init", "-q", "--bare"], remoteDir);
  const dir = makeRepo("master");
  git(["remote", "add", "origin", remoteDir], dir);
  git(["push", "-q", "origin", "master"], dir);
  git(["checkout", "-q", "-b", "feat"], dir);
  git(["branch", "--set-upstream-to=origin/master", "feat"], dir);
  return { dir, remoteDir };
}
const PUSH_TRACKING = makePushTrackingRepo();

after(() => {
  rmSync(MASTER_REPO, { recursive: true, force: true });
  rmSync(FEATURE_REPO, { recursive: true, force: true });
  rmSync(PUSH_TRACKING.dir, { recursive: true, force: true });
  rmSync(PUSH_TRACKING.remoteDir, { recursive: true, force: true });
});

function runHook(command, { cwd = FEATURE_REPO, env = {}, payloadCwd } = {}) {
  // `payloadCwd` mimics the PreToolUse payload's own `cwd` field (item 6): the hook must use
  // it over the process's spawn cwd — distinct so a mismatch actually proves which one wins.
  const input = JSON.stringify({ tool_name: "Bash", tool_input: { command }, ...(payloadCwd ? { cwd: payloadCwd } : {}) });
  const result = spawnSync("bash", [HOOK_PATH], {
    input,
    encoding: "utf8",
    cwd,
    env: {
      ...process.env,
      BRAWLER_GIT_BOUNDARIES_OFF: "",
      ...env,
    },
  });
  const stdout = result.stdout.trim();
  if (!stdout) return "allow";
  return JSON.parse(stdout).hookSpecificOutput?.permissionDecision ?? "allow";
}

// Deny cases that don't depend on which branch is checked out (evaluated on the feature repo).
const DENY_BRANCH_INDEPENDENT = [
  // astra r5: a boolean git global before the subcommand must never be read AS the subcommand
  "git --no-optional-locks reset --hard",
  "git --literal-pathspecs --no-pager stash",
  "git --exec-path=/tmp clean -f",
  // astra r4 #1: a forbidden flag after a consumed value must survive the operand parse
  'git commit -m "fix: x" --no-verify',
  'git commit -m "fix: x" -n',
  'git commit -am "fix: x" --no-verify',
  'git commit --message "fix: x" -n',
  "git checkout -- tracked.txt",
  "git checkout tracked.txt",
  "git checkout .",
  "git checkout -f",
  "git checkout --force",
  "git checkout --discard-changes",
  "git restore tracked.txt",
  "git restore --worktree tracked.txt",
  "git restore --staged --worktree tracked.txt",
  "git stash",
  "git stash -u",
  "git stash pop",
  "git stash drop",
  "git stash clear",
  "git reset --hard HEAD~1",
  "git reset --merge",
  "git reset --keep",
  "git clean -fd",
  "git clean",
  "git commit --no-verify -m x",
  "git commit -n -m x",
  "git push --force",
  "git push -f origin feat/x",
  "git push --force-if-includes",
  "git push --all",
  "git push --mirror",
  "git push origin --delete feat/x",
  "git push origin +feat:master",
  "git push origin x:refs/heads/master",
  "git push origin \"HEAD:master\"",
  "git push --force-with-lease=master",
  "git worktree remove ../x --force",
  "git worktree remove ../x -f",
  "gh pr merge 123",
  "gh repo delete foo/bar",
  "gh repo edit --description x",
  "gh repo rename x",
  "gh repo archive",
  "gh release create v1.0.0",
  "gh release delete v1.0.0",
  "gh release edit v1.0.0",
  "gh release upload v1.0.0 file.zip",
  "gh api -X DELETE repos/o/r",
  "gh api -X PUT repos/o/r/rulesets/1",
  "gh api -X PATCH repos/o/r/branches/main/protection",
  'gh api graphql -f query="mutation { mergePullRequest(x: 1) }"',
  // prefix stripping / cwd tracking / wrapper mechanics
  "rtk git stash",
  "bash -c \"git stash\"",
  "command git stash",
  "/usr/bin/git reset --hard",
  "env -u X git clean -f",
  "git -c core.hooksPath=/dev/null commit -m x",
  // review findings (bypasses), fixed below — one deny case each
  "FOO=1 git stash", // 1. leading assignment(s) not stripped
  "nix develop -c git stash", // 2. `nix … -c` wrapper not unwrapped
  'nix develop .#x --command git stash', // 2. `--command` spelling
  'eval "git stash"', // 3. `eval` hid the inner command
  "echo $(git stash)", // 3. a `$( … )` command substitution hid it
  "echo `git stash`", // 3. a backtick substitution hid it
  "git push -fu origin x", // 4. combined short flag `-fu` not matched as force
  "git commit -an -m x", // 4. combined short flag `-an` not matched as --no-verify
  "gh api -X PUT repos/o/r/pulls/12/merge", // 5. PR-merge via the REST API
  "gh api -X POST repos/o/r/releases", // 5. release creation via the REST API
  "git checkout HEAD -- tracked.txt", // 7. explicit deny case named by the review
  // adversarial review round 2 (owner 2026-09-08), lettered A-J below
  "git switch --discard-changes feat/x", // A. checkSwitch had no discard check at all
  "git switch -f feat/x", // A. same, short form
  "git switch --force feat/x", // A. same, long form
  "git checkout -fq feat/x", // B. bundled short flag `-fq` on checkout (letters generalized)
  "git switch -fc feat/x", // B. bundled short flag on switch
  "git restore --staged -SW tracked.txt", // B. bundled `-SW` on restore — W (worktree) hides in the bundle
  "(git reset --hard)", // D. subshell parens hid the leading/trailing tokens
  "{ git stash; }", // D. brace-group syntax, same class
  'command bash -c "git reset --hard"', // E. `command` prefix defeated wrapper detection
  "git push origin :", // H. lone `:` (matching refspec) can update master
  'git push origin ":"', // H. same, quoted
  "gh api repos/o/r/releases -f tag_name=v1", // I. implicit POST via -f, no explicit -X
  "gh api -f tag_name=v1 repos/o/r/releases", // I. same, endpoint operand after the flag
  'echo "unsafe: $(git push --force)"', // G. double-quoted $() still executes — must still deny
  // hard-gates wave 2 review (owner 2026-09-08), items 1-3 below
  'git clean -f -e "-n"', // 1. `-n` here is `-e`'s value, not clean's own dry-run flag
  "git push --repo=origin HEAD:master", // 2. `--repo=` value must not be read as the remote positional
  "git push --repo origin HEAD:master", // 2. same, space-separated `--repo` value form
];

// Deny cases that require a specific starting branch or a chain transition.
const DENY_BRANCH_DEPENDENT = [
  // astra r4 #2: a `||` chain keeps every reachable branch state; master reachable → deny
  { cmd: "git checkout master && git commit -m x || true", cwd: FEATURE_REPO },
  // astra r4 #3: a -C invocation into a master worktree never inherits the chain's assumed branch
  { cmd: `git checkout -b feat/review-example && git -C ${MASTER_REPO} commit -m x`, cwd: FEATURE_REPO },
  { cmd: "git commit -m x", cwd: MASTER_REPO },
  { cmd: "git push", cwd: MASTER_REPO },
  { cmd: "git merge feature", cwd: MASTER_REPO },
  { cmd: "git rebase main", cwd: MASTER_REPO },
  { cmd: "git cherry-pick abc123", cwd: MASTER_REPO },
  { cmd: "git revert HEAD", cwd: MASTER_REPO },
  { cmd: "git am patch.mbox", cwd: MASTER_REPO },
  { cmd: "git apply patch.diff", cwd: MASTER_REPO },
  // chain-aware branch transition: assumed branch becomes master mid-chain
  { cmd: "git checkout master && git commit -m x", cwd: FEATURE_REPO },
  // cwd tracking via `cd`: the stash rule fires regardless, proving the cwd/chain plumbing ran
  { cmd: "cd ../wt && git stash", cwd: FEATURE_REPO },
  // 4. `git push origin HEAD` has a refspec, so the old code skipped the no-refspec/master check
  { cmd: "git push origin HEAD", cwd: MASTER_REPO },
  { cmd: "git push origin HEAD:", cwd: MASTER_REPO },
  // C. a `reset` with a commit-ish operand moves HEAD — a history rewrite when done on master
  { cmd: "git reset --soft HEAD~1", cwd: MASTER_REPO },
  { cmd: "git reset HEAD~1", cwd: MASTER_REPO },
  // F. `-C` must scope only the ONE git invocation it's attached to, never leak into the next
  // chain segment's cwd — so this commit is still judged against the real (master) cwd.
  { cmd: "git -C ../feature status && git commit -m x", cwd: MASTER_REPO },
  // hard-gates wave 2 review (owner 2026-09-08), item 4 below
  // 4a. a `-C ../feature` checkout's assumed branch must never leak into a LATER segment that
  // runs in the chain's own (unscoped) cwd — the commit is still judged against master.
  { cmd: "git -C ../feature checkout feat/x && git commit -m x", cwd: MASTER_REPO },
  // 4b. a `||` anywhere in the raw command disables chain-branch trust entirely: the checkout
  // may never have run (its `||` guard succeeded), so the commit is judged on the live branch.
  { cmd: "true || git checkout feat/x; git commit -m x", cwd: MASTER_REPO },
];

const ALLOW_BRANCH_INDEPENDENT = [
  "git --no-optional-locks status",
  "git --namespace x log",
  'git commit --message="--no-verify"',
  'git commit -m "fix: x" --amend --no-edit',
  'git commit -F notes.txt -q',
  "git checkout -b feat/x",
  "git checkout -B feat/x",
  "git checkout --orphan feat/x",
  "git checkout feat/x",
  "git switch -c feat/x",
  "git switch feat/x",
  "git restore --staged tracked.txt",
  "git stash list",
  "git stash show",
  "git reset",
  "git reset --soft HEAD~1",
  "git reset --mixed",
  "git clean -n",
  "git clean --dry-run",
  "git commit -m \"no-verify in the message\"",
  "git commit -m x",
  "git push origin feat/x",
  "git push",
  "git push -n",
  "git push --dry-run",
  "git fetch",
  "git pull --ff-only",
  "git log",
  "git status",
  "git diff",
  "git branch -D wave2/s1",
  "git worktree add ../x",
  "git worktree remove ../x",
  "gh issue edit 480 --add-label x",
  "gh project item-edit 1 --field-id x --text y",
  "gh release list",
  "gh release view v1.0.0",
  "gh api -X POST repos/o/r/issues/1/sub_issues",
  // review findings — matching allow cases, one per item
  "FOO=1 BAR=2 git log", // 1. stripped assignments don't break an ordinary read
  "nix develop -c git log", // 2. same for the nix -c wrapper
  'eval "git log"', // 3. eval of a harmless command
  "echo $(git log)", // 3. a substitution around a harmless command
  "git push -u origin x", // 4. `-u` alone (no `f`) is not force
  "git commit -am x", // 4. `-am` (no `n`) is not --no-verify
  "gh api repos/o/r/releases", // 5. a GET on /releases is a read, not a mutation
  "git checkout -q feat/x", // 7. explicit allow case named by the review
  // adversarial review round 2 (owner 2026-09-08), lettered A-J below
  "git commit -m \"--no-verify\"", // G. a message that only LOOKS like the flag must not deny
  "git commit -m \"fix -n\"", // G. same, a bundled-looking short flag inside the message
  'git commit --message="--no-verify"', // G. inline `--opt=value` form of the same trap
  "echo 'inert: $(git push --force)'", // G. single-quoted $() is inert in bash — must not deny
  // hard-gates wave 2 review (owner 2026-09-08), items 3 below
  "git commit -mnonverify", // 3. attached `-m` value that CONTAINS "n" must not read as bundled -n
  "git commit -amsg", // 3. bundle `-a` + attached `-m` value, same trap
  "git commit -am msg", // 3. bundle `-a` + separate-token `-m` value
];

const ALLOW_BRANCH_DEPENDENT = [
  // local merge into a feature branch stays allowed (assumption 3)
  { cmd: "git merge --no-ff wave2/s1", cwd: FEATURE_REPO },
  { cmd: "git checkout master && git pull --ff-only", cwd: FEATURE_REPO },
  // 4. `git push origin HEAD` is fine off master
  { cmd: "git push origin HEAD", cwd: FEATURE_REPO },
  // C. `git reset` with no commit operand (or an existing-path operand) never moves HEAD —
  // stays an ordinary unstage even directly on master
  { cmd: "git reset", cwd: MASTER_REPO },
  { cmd: "git reset --mixed", cwd: MASTER_REPO },
  { cmd: "git reset tracked.txt", cwd: MASTER_REPO },
  // F. `cd` must reset chainBranch — branch knowledge from before the `cd` is for a different
  // worktree. Starting on MASTER_REPO, `checkout master` sets chainBranch="master"; `cd` into
  // FEATURE_REPO (a real repo on a non-master branch) must drop that stale assumption, so the
  // commit is judged by FEATURE_REPO's real (non-master) branch and allowed. Denied here would
  // prove the bug: a stale "master" chainBranch surviving the `cd`.
  { cmd: `git checkout master && cd ${FEATURE_REPO} && git commit -m x`, cwd: MASTER_REPO },
  // hard-gates wave 2 review (owner 2026-09-08), item 3 below
  // 3. `--` settles a `reset` as the path form regardless of what precedes it (even `HEAD`) —
  // it never moves HEAD, so it stays allowed on master.
  { cmd: "git reset HEAD -- tracked.txt", cwd: MASTER_REPO },
  { cmd: "git reset -- tracked.txt", cwd: MASTER_REPO },
];

test("git-boundaries denies the write matrix, allows everything else (branch-independent)", () => {
  for (const cmd of DENY_BRANCH_INDEPENDENT) {
    assert.equal(runHook(cmd, { cwd: FEATURE_REPO }), "deny", `expected deny for: ${cmd}`);
  }
  for (const cmd of ALLOW_BRANCH_INDEPENDENT) {
    assert.equal(runHook(cmd, { cwd: FEATURE_REPO }), "allow", `expected allow for: ${cmd}`);
  }
});

test("git-boundaries master-branch and chain-transition rules", () => {
  for (const { cmd, cwd } of DENY_BRANCH_DEPENDENT) {
    assert.equal(runHook(cmd, { cwd }), "deny", `expected deny for: ${cmd}`);
  }
  for (const { cmd, cwd } of ALLOW_BRANCH_DEPENDENT) {
    assert.equal(runHook(cmd, { cwd }), "allow", `expected allow for: ${cmd}`);
  }
});

test("the escape hatch, malformed input, non-Bash tools, and the un-flagged override all allow", () => {
  // Escape hatch: BRAWLER_GIT_BOUNDARIES_OFF=1 in the hook's own env allows a command that would otherwise deny.
  assert.equal(runHook("git push --force", { cwd: MASTER_REPO, env: { BRAWLER_GIT_BOUNDARIES_OFF: "1" } }), "allow");

  // J. the escape hatch is diagnosed on stderr — stdout (the actual permission decision) stays empty (allow).
  const offRun = spawnSync("bash", [HOOK_PATH], {
    input: JSON.stringify({ tool_name: "Bash", tool_input: { command: "git push --force" } }),
    encoding: "utf8",
    cwd: MASTER_REPO,
    env: { ...process.env, BRAWLER_GIT_BOUNDARIES_OFF: "1" },
  });
  assert.equal(offRun.stdout.trim(), "");
  assert.match(offRun.stderr, /git-boundaries: disabled by BRAWLER_GIT_BOUNDARIES_OFF=1/);

  // Malformed JSON on stdin never blocks.
  const malformed = spawnSync("bash", [HOOK_PATH], { input: "not json", encoding: "utf8", cwd: FEATURE_REPO });
  assert.equal(malformed.stdout.trim(), "");
  assert.equal(malformed.status, 0);

  // A non-Bash tool is never inspected.
  const nonBash = spawnSync("bash", [HOOK_PATH], {
    input: JSON.stringify({ tool_name: "Edit", tool_input: { command: "git push --force" } }),
    encoding: "utf8",
    cwd: FEATURE_REPO,
  });
  assert.equal(nonBash.stdout.trim(), "");
});

test("5. bare push off master denies when @{push}/@{upstream} resolves to <remote>/master", () => {
  // PUSH_TRACKING.dir is on `feat` (not master), with @{upstream} set to origin/master — a bare
  // push still lands on master, so both the fully-bare and remote-only forms must deny.
  assert.equal(runHook("git push", { cwd: PUSH_TRACKING.dir }), "deny");
  assert.equal(runHook("git push origin", { cwd: PUSH_TRACKING.dir }), "deny");
  // An explicit refspec off the same repo is unaffected by the tracking check — normal deny/allow
  // rules (from checkPush's refspec loop) still apply on their own terms.
  assert.equal(runHook("git push origin feat:feat", { cwd: PUSH_TRACKING.dir }), "allow");
  // FEATURE_REPO has no remote/tracking at all — @{push}/@{upstream} are unresolvable, so a bare
  // push stays allowed (unresolvable never denies).
  assert.equal(runHook("git push", { cwd: FEATURE_REPO }), "allow");
});

test("6. payload.cwd (not the hook process's own spawn cwd) governs branch resolution", () => {
  // Spawned from FEATURE_REPO (not master) but the PreToolUse payload names MASTER_REPO —
  // the payload's cwd must win, so a plain commit is denied.
  assert.equal(
    runHook("git commit -m x", { cwd: FEATURE_REPO, payloadCwd: MASTER_REPO }),
    "deny",
    "payload.cwd must override the hook's own spawn cwd",
  );
  // And the reverse: spawned from MASTER_REPO but the payload names FEATURE_REPO — allowed.
  assert.equal(
    runHook("git commit -m x", { cwd: MASTER_REPO, payloadCwd: FEATURE_REPO }),
    "allow",
    "payload.cwd must override the hook's own spawn cwd, in both directions",
  );
});
