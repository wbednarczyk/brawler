// Guard (S2, hard-gates closing wave, DoD §K): check-evidence must deny
// `gh pr create`/`gh pr ready` (without --undo) and a `git push` onto a
// branch with an open non-draft PR unless a check-stamp certifies HEAD's
// exact tree, and must never flag anything else. `gh` is stubbed via PATH so
// the hook's own child-process call is exercised, not mocked out.
import { test, after as afterAll } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(fileURLToPath(new URL("../..", import.meta.url)));
const HOOK_PATH = path.join(REPO_ROOT, ".claude/hooks/check-evidence.sh");
const STAMP_CLI = path.join(REPO_ROOT, "scripts/check/check-stamp.mjs");

function git(args, cwd) {
  const r = spawnSync("git", args, { cwd, encoding: "utf8" });
  if (r.status !== 0) throw new Error(`git ${args.join(" ")} failed in ${cwd}: ${r.stderr}`);
  return r.stdout.trim();
}

/** A repo with `origin/master` reachable (bare remote, master pushed) and a feature branch
 * checked out — the shape check-evidence's merge-base/docs-only logic needs. */
function makeRepo() {
  const remoteDir = mkdtempSync(path.join(tmpdir(), "check-evidence-remote-"));
  git(["init", "-q", "--bare"], remoteDir);
  const dir = mkdtempSync(path.join(tmpdir(), "check-evidence-"));
  git(["init", "-q"], dir);
  git(["symbolic-ref", "HEAD", "refs/heads/master"], dir);
  git(["config", "user.email", "t@example.com"], dir);
  git(["config", "user.name", "test"], dir);
  mkdirSync(path.join(dir, "docs"), { recursive: true });
  writeFileSync(path.join(dir, "docs/x.md"), "x\n");
  writeFileSync(path.join(dir, "src.rs"), "fn main() {}\n");
  git(["add", "-A"], dir);
  git(["commit", "-q", "-m", "init"], dir);
  git(["remote", "add", "origin", remoteDir], dir);
  git(["push", "-q", "origin", "master"], dir);
  git(["checkout", "-q", "-b", "feat"], dir);
  return { dir, remoteDir };
}

function fakeGhDir(behavior) {
  const dir = mkdtempSync(path.join(tmpdir(), "fake-gh-"));
  const script = path.join(dir, "gh");
  writeFileSync(script, behavior);
  chmodSync(script, 0o755);
  return dir;
}

const NO_OPEN_PR_GH = "#!/usr/bin/env bash\necho '[]'\n";
const OPEN_NON_DRAFT_GH = '#!/usr/bin/env bash\necho \'[{"isDraft":false,"number":7}]\'\n';
const OPEN_DRAFT_GH = '#!/usr/bin/env bash\necho \'[{"isDraft":true,"number":8}]\'\n';
const FAILING_GH = "#!/usr/bin/env bash\nexit 1\n";

function runHook(command, cwd, { env = {}, ghDir } = {}) {
  const input = JSON.stringify({ tool_name: "Bash", tool_input: { command } });
  const PATH = ghDir ? `${ghDir}:${process.env.PATH}` : process.env.PATH;
  const result = spawnSync("bash", [HOOK_PATH], {
    input,
    encoding: "utf8",
    cwd,
    env: { ...process.env, PATH, BRAWLER_CHECK_EVIDENCE_OFF: "", ...env },
  });
  const stdout = result.stdout.trim();
  if (!stdout) return "allow";
  return JSON.parse(stdout).hookSpecificOutput?.permissionDecision ?? "allow";
}

function stampCurrentTree(dir, target) {
  const before = spawnSync("node", [STAMP_CLI, "fingerprint"], { cwd: dir, encoding: "utf8" }).stdout.trim();
  const r = spawnSync("node", [STAMP_CLI, "write", target, before], { cwd: dir, encoding: "utf8" });
  if (r.status !== 0) throw new Error(`stamp write failed: ${r.stderr}`);
}

/** Stamp a LOCAL branch's tree without leaving it checked out afterwards (tmp repo — checkout
 * here is fixture setup, not the agent's own git use). */
function stampBranchTree(dir, target, branch) {
  const original = git(["rev-parse", "--abbrev-ref", "HEAD"], dir);
  git(["checkout", "-q", branch], dir);
  stampCurrentTree(dir, target);
  git(["checkout", "-q", original], dir);
}

/** A fake `gh` that can answer both `pr list --head <branch>` (a non-draft PR only when the
 * queried head matches `prHead`) and `pr view <arg>` (resolves to `viewHead`/`viewOid` only when
 * the arg matches `viewArg`) — lets a test prove the hook looked at the RIGHT branch/commit, not
 * just any. `viewOid` defaults to a sha that exists in no repo, so a test that doesn't care about
 * the oid path still exercises the "commit not present locally" deny. */
function fakeGhAdvanced({ prHead, prNumber = 7, viewArg, viewHead, viewOid = "0".repeat(40) } = {}) {
  const listBlock = prHead
    ? `if [ "$1" = "pr" ] && [ "$2" = "list" ]; then
  head=""
  shift 2
  while [ $# -gt 0 ]; do
    if [ "$1" = "--head" ]; then head="$2"; fi
    shift
  done
  if [ "$head" = "${prHead}" ]; then
    echo '[{"isDraft":false,"number":${prNumber}}]'
  else
    echo '[]'
  fi
  exit 0
fi`
    : `if [ "$1" = "pr" ] && [ "$2" = "list" ]; then echo '[]'; exit 0; fi`;
  const viewBlock = viewArg
    ? `if [ "$1" = "pr" ] && [ "$2" = "view" ] && [ "$3" = "${viewArg}" ]; then
  echo '{"headRefName":"${viewHead}","headRefOid":"${viewOid}","isDraft":false}'
  exit 0
fi
if [ "$1" = "pr" ] && [ "$2" = "view" ]; then exit 1; fi`
    : `if [ "$1" = "pr" ] && [ "$2" = "view" ]; then exit 1; fi`;
  return `#!/usr/bin/env bash\n${listBlock}\n${viewBlock}\necho '[]'\n`;
}

/** A fake `gh` that reports a PR on `prHead` only when invoked with cwd == `expectedDir` — lets a
 * test prove the hook spawned `gh` in the right repository, not just anywhere (a cwd-blind fake
 * would pass whether or not the hook resolved the correct directory). */
function fakeGhCwdGate(expectedDir, prHead, prNumber) {
  return `#!/usr/bin/env bash
if [ "$1" = "pr" ] && [ "$2" = "list" ]; then
  head=""
  shift 2
  while [ $# -gt 0 ]; do
    if [ "$1" = "--head" ]; then head="$2"; fi
    shift
  done
  here="$(pwd -P)"
  want="$(cd "${expectedDir}" && pwd -P)"
  if [ "$head" = "${prHead}" ] && [ "$here" = "$want" ]; then
    echo '[{"isDraft":false,"number":${prNumber}}]'
  else
    echo '[]'
  fi
  exit 0
fi
echo '[]'
`;
}

const repos = [];
const ghDirs = [];
afterAll(() => {
  for (const { dir, remoteDir } of repos) {
    rmSync(dir, { recursive: true, force: true });
    rmSync(remoteDir, { recursive: true, force: true });
  }
  for (const d of ghDirs) rmSync(d, { recursive: true, force: true });
});

function repo() {
  const r = makeRepo();
  repos.push(r);
  return r;
}

/** A repo() whose `origin` remote is a github.com URL for the given `owner/repo` slug — needed to
 * exercise the `gh --repo`-vs-origin identity check, since repo()'s default origin is a local tmp
 * path (get-url still works without network access; it only reads .git/config). */
function repoWithGithubOrigin(slug) {
  const r = repo();
  git(["remote", "set-url", "origin", `https://github.com/${slug}.git`], r.dir);
  return r;
}

function gh(behavior) {
  const d = fakeGhDir(behavior);
  ghDirs.push(d);
  return d;
}

test("gh pr create with no stamp denies", () => {
  const { dir } = repo();
  assert.equal(runHook("gh pr create --title x --body y", dir), "deny");
});

test("gh pr create with a fresh check-local stamp on the committed tree allows", () => {
  const { dir } = repo();
  writeFileSync(path.join(dir, "src.rs"), "fn main() { 1; }\n");
  git(["commit", "-q", "-am", "code change"], dir);
  stampCurrentTree(dir, "check-local");
  assert.equal(runHook("gh pr create", dir), "allow");
});

test("stamp valid, then an uncommitted tracked edit — gh pr create still allows (publishes HEAD, not the working tree)", () => {
  const { dir } = repo();
  writeFileSync(path.join(dir, "src.rs"), "fn main() { 1; }\n");
  git(["commit", "-q", "-am", "code change"], dir);
  stampCurrentTree(dir, "check-local");
  writeFileSync(path.join(dir, "src.rs"), "fn main() { 2; }\n"); // uncommitted after stamping
  assert.equal(runHook("gh pr create", dir), "allow");
});

test("stamp taken with an untracked file that was never committed denies (stamp tree != HEAD tree)", () => {
  const { dir } = repo();
  writeFileSync(path.join(dir, "src.rs"), "fn main() { 1; }\n");
  git(["commit", "-q", "-am", "code change"], dir);
  writeFileSync(path.join(dir, "untracked.txt"), "x\n"); // present when stamped, never committed
  stampCurrentTree(dir, "check-local");
  rmSync(path.join(dir, "untracked.txt"));
  assert.equal(runHook("gh pr create", dir), "deny");
});

test("docs-only change with a check-docs stamp allows", () => {
  const { dir } = repo();
  writeFileSync(path.join(dir, "docs/x.md"), "changed\n");
  git(["commit", "-q", "-am", "docs change"], dir);
  stampCurrentTree(dir, "check-docs");
  assert.equal(runHook("gh pr create", dir), "allow");
});

test("code change with only a check-docs stamp denies", () => {
  const { dir } = repo();
  writeFileSync(path.join(dir, "src.rs"), "fn main() { 1; }\n");
  git(["commit", "-q", "-am", "code change"], dir);
  stampCurrentTree(dir, "check-docs");
  assert.equal(runHook("gh pr create", dir), "deny");
});

test("deleted docs file with a check-docs stamp allows", () => {
  const { dir } = repo();
  rmSync(path.join(dir, "docs/x.md"));
  git(["commit", "-q", "-am", "remove docs"], dir);
  stampCurrentTree(dir, "check-docs");
  assert.equal(runHook("gh pr create", dir), "allow");
});

test("gh pr ready --undo always allows", () => {
  const { dir } = repo();
  assert.equal(runHook("gh pr ready --undo", dir), "allow");
});

test("bare gh pr ready with no PR resolvable for the current branch fails open (allows) rather than trusting local HEAD directly", () => {
  const { dir } = repo(); // origin is a local bare repo, not GitHub — `gh pr view` cannot resolve a PR here
  assert.equal(runHook("gh pr ready", dir), "allow");
});

test("bare gh pr ready resolves the CURRENT branch's PR via gh pr view (not local HEAD directly) and denies when the PR's remote head oid is unstamped", () => {
  const { dir } = repo();
  stampCurrentTree(dir, "check-local"); // local HEAD looks done — must not be trusted without resolving the PR
  git(["branch", "other"], dir);
  git(["checkout", "-q", "other"], dir);
  writeFileSync(path.join(dir, "src.rs"), "fn main() { 321; }\n");
  git(["commit", "-q", "-am", "other change"], dir);
  const otherOid = git(["rev-parse", "HEAD"], dir);
  git(["checkout", "-q", "feat"], dir);
  const ghScript = `#!/usr/bin/env bash
if [ "$1" = "pr" ] && [ "$2" = "view" ] && [ "$3" = "--json" ]; then
  echo '{"headRefName":"other","headRefOid":"${otherOid}","isDraft":false}'
  exit 0
fi
exit 1
`;
  const ghDir = gh(ghScript);
  assert.equal(runHook("gh pr ready", dir, { ghDir }), "deny");
});

test("git push with an open non-draft PR and no stamp denies", () => {
  const { dir } = repo();
  const ghDir = gh(OPEN_NON_DRAFT_GH);
  assert.equal(runHook("git push -u origin feat", dir, { ghDir }), "deny");
});

test("git push with only a draft PR allows", () => {
  const { dir } = repo();
  const ghDir = gh(OPEN_DRAFT_GH);
  assert.equal(runHook("git push -u origin feat", dir, { ghDir }), "allow");
});

test("git push with no open PR allows", () => {
  const { dir } = repo();
  const ghDir = gh(NO_OPEN_PR_GH);
  assert.equal(runHook("git push -u origin feat", dir, { ghDir }), "allow");
});

test("git push when gh fails (offline) allows", () => {
  const { dir } = repo();
  const ghDir = gh(FAILING_GH);
  assert.equal(runHook("git push -u origin feat", dir, { ghDir }), "allow");
});

test("unrelated gh issue list allows", () => {
  const { dir } = repo();
  assert.equal(runHook("gh issue list", dir), "allow");
});

test("unrelated git status allows", () => {
  const { dir } = repo();
  assert.equal(runHook("git status", dir), "allow");
});

test("BRAWLER_CHECK_EVIDENCE_OFF=1 allows even gh pr create with no stamp", () => {
  const { dir } = repo();
  assert.equal(runHook("gh pr create", dir, { env: { BRAWLER_CHECK_EVIDENCE_OFF: "1" } }), "allow");
});

test('wrapped bash -c "git push" behaves the same as bare git push', () => {
  const { dir } = repo();
  const ghDir = gh(OPEN_NON_DRAFT_GH);
  assert.equal(runHook('bash -c "git push -u origin feat"', dir, { ghDir }), "deny");
});

// ---- refspec source/destination and gh pr ready/create head resolution ----

test("git push origin HEAD:reviewed with a non-draft PR on reviewed and no stamp denies (dst drives the PR lookup)", () => {
  const { dir } = repo();
  const ghDir = gh(fakeGhAdvanced({ prHead: "reviewed", prNumber: 21 }));
  assert.equal(runHook("git push origin HEAD:reviewed", dir, { ghDir }), "deny");
});

test("git push origin scratch reviewed evaluates EVERY refspec, not just the first", () => {
  const { dir } = repo();
  const ghDir = gh(fakeGhAdvanced({ prHead: "reviewed", prNumber: 22 }));
  assert.equal(runHook("git push origin scratch reviewed", dir, { ghDir }), "deny");
});

test("git push origin :reviewed (deletion refspec) updates nothing and is never denied", () => {
  const { dir } = repo();
  assert.equal(runHook("git push origin :reviewed", dir), "allow");
});

test("git push -u origin feature (no PR on feature) allows", () => {
  const { dir } = repo();
  const ghDir = gh(NO_OPEN_PR_GH);
  assert.equal(runHook("git push -u origin feature", dir, { ghDir }), "allow");
});

test("gh pr ready 12 resolves PR 12's actual head branch (not the checked-out one) and denies when it is unstamped", () => {
  const { dir } = repo();
  stampCurrentTree(dir, "check-local"); // checked-out branch 'feat' looks done — must not be trusted
  git(["branch", "other"], dir);
  git(["checkout", "-q", "other"], dir);
  writeFileSync(path.join(dir, "src.rs"), "fn main() { 1; }\n"); // 'other' diverges — its tree is NOT the stamped one
  git(["commit", "-q", "-am", "other change"], dir);
  const otherOid = git(["rev-parse", "HEAD"], dir);
  git(["checkout", "-q", "feat"], dir);
  const ghDir = gh(fakeGhAdvanced({ viewArg: "12", viewHead: "other", viewOid: otherOid }));
  assert.equal(runHook("gh pr ready 12", dir, { ghDir }), "deny");
});

test("gh pr ready 12 allows once PR 12's actual head branch carries a fresh stamp", () => {
  const { dir } = repo();
  git(["branch", "other"], dir);
  stampBranchTree(dir, "check-local", "other");
  const otherOid = git(["rev-parse", "other"], dir);
  const ghDir = gh(fakeGhAdvanced({ viewArg: "12", viewHead: "other", viewOid: otherOid }));
  assert.equal(runHook("gh pr ready 12", dir, { ghDir }), "allow");
});

// ---- git/gh global options don't hide the subcommand ----

test("git -C <dir> push recognizes the subcommand and evaluates evidence in the -C target, not the invoking cwd", () => {
  const { dir } = repo();
  const outsideCwd = mkdtempSync(path.join(tmpdir(), "check-evidence-outside-"));
  const ghDir = gh(fakeGhAdvanced({ prHead: "feat", prNumber: 31 }));
  assert.equal(runHook(`git -C ${dir} push origin feat`, outsideCwd, { ghDir }), "deny");
  rmSync(outsideCwd, { recursive: true, force: true });
});

test("git -c k=v push recognizes the subcommand after a -c global option", () => {
  const { dir } = repo();
  const ghDir = gh(fakeGhAdvanced({ prHead: "feat", prNumber: 32 }));
  assert.equal(runHook("git -c foo.bar=baz push origin feat", dir, { ghDir }), "deny");
});

test("git --git-dir=<path> push recognizes the subcommand after --git-dir=", () => {
  const { dir } = repo();
  const ghDir = gh(fakeGhAdvanced({ prHead: "feat", prNumber: 33 }));
  assert.equal(runHook(`git --git-dir=${dir}/.git push origin feat`, dir, { ghDir }), "deny");
});

test("git --no-pager push recognizes the subcommand after --no-pager", () => {
  const { dir } = repo();
  const ghDir = gh(fakeGhAdvanced({ prHead: "feat", prNumber: 34 }));
  assert.equal(runHook("git --no-pager push origin feat", dir, { ghDir }), "deny");
});

test("gh --repo o/r pr create recognizes the subcommand after --repo", () => {
  const { dir } = repo();
  assert.equal(runHook("gh --repo o/r pr create", dir), "deny");
});

test("gh -R o/r pr ready recognizes the subcommand after -R", () => {
  const { dir } = repo();
  assert.equal(runHook("gh -R o/r pr ready", dir), "deny");
});

test("git --git-dir=<dir>/.git push (no --work-tree) reads the STAMP from the resolved toplevel too — a fresh stamp there allows from any cwd", () => {
  const { dir } = repo();
  const outsideCwd = mkdtempSync(path.join(tmpdir(), "check-evidence-outside-"));
  writeFileSync(path.join(dir, "src.rs"), "fn main() { 2; }\n");
  git(["commit", "-q", "-am", "feat change"], dir);
  stampCurrentTree(dir, "check-local");
  const ghDir = gh(fakeGhCwdGate(dir, "feat", 71));
  assert.equal(runHook(`git --git-dir=${dir}/.git push origin feat`, outsideCwd, { ghDir }), "allow");
  rmSync(outsideCwd, { recursive: true, force: true });
});

test("git --git-dir=<dir>/.git push (no --work-tree) runs the PR-list lookup in the resolved toplevel, not the invoking cwd", () => {
  const { dir } = repo();
  const outsideCwd = mkdtempSync(path.join(tmpdir(), "check-evidence-outside-"));
  // gh only reports a PR when invoked with cwd == dir — proves the lookup ran in the resolved
  // toplevel rather than blindly inheriting outsideCwd (a fake gh that ignores cwd can't catch this).
  const ghDir = gh(fakeGhCwdGate(dir, "feat", 71));
  assert.equal(runHook(`git --git-dir=${dir}/.git push origin feat`, outsideCwd, { ghDir }), "deny");
  rmSync(outsideCwd, { recursive: true, force: true });
});

test("git -C <dir> --work-tree <dir> push keeps -C in the git prefix alongside --work-tree, so evidence still resolves to that repo from an unrelated invoking cwd", () => {
  const { dir } = repo();
  writeFileSync(path.join(dir, "src.rs"), "fn main() { 1; }\n");
  git(["commit", "-q", "-am", "code change"], dir);
  stampCurrentTree(dir, "check-local"); // feat's tree IS fresh — dropping -C breaks discovery and would wrongly deny
  const outsideCwd = mkdtempSync(path.join(tmpdir(), "check-evidence-outside-"));
  const ghDir = gh(fakeGhAdvanced({ prHead: "feat", prNumber: 72 }));
  assert.equal(runHook(`git -C ${dir} --work-tree ${dir} push origin feat`, outsideCwd, { ghDir }), "allow");
  rmSync(outsideCwd, { recursive: true, force: true });
});

test("gh pr create --repo other/repo (given AFTER the subcommand) is checked against origin the same as a leading --repo — denies even with a fresh stamp", () => {
  const { dir } = repoWithGithubOrigin("owner/brawler");
  stampCurrentTree(dir, "check-local"); // rules out the generic "no stamp" deny — this must be the repo-identity deny
  assert.equal(runHook("gh pr create --repo other/repo", dir), "deny");
});

test("gh pr create --repo <matching origin slug> (given AFTER the subcommand) allows with a fresh stamp", () => {
  const { dir } = repoWithGithubOrigin("owner/brawler");
  stampCurrentTree(dir, "check-local");
  assert.equal(runHook("gh pr create --repo owner/brawler", dir), "allow");
});

test("gh pr ready --repo other/repo (given AFTER the subcommand) denies (repository targeting applies to ready too)", () => {
  const { dir } = repoWithGithubOrigin("owner/brawler");
  assert.equal(runHook("gh pr ready --repo other/repo", dir), "deny");
});

// ---- docs-only classification inspects both rename endpoints ----

test("a code file renamed into docs/ still counts as code — check-docs stamp denies", () => {
  const { dir } = repo();
  git(["mv", "src.rs", "docs/src.rs"], dir);
  git(["commit", "-q", "-m", "rename code into docs"], dir);
  stampCurrentTree(dir, "check-docs");
  assert.equal(runHook("gh pr create", dir), "deny");
});

test("a docs-only rename allows with a check-docs stamp", () => {
  const { dir } = repo();
  git(["mv", "docs/x.md", "docs/y.md"], dir);
  git(["commit", "-q", "-m", "rename docs file"], dir);
  stampCurrentTree(dir, "check-docs");
  assert.equal(runHook("gh pr create", dir), "allow");
});

// ---- a draft PR is WIP, exempt from evidence ----

test("gh --repo other/repo pr create --draft allows (the draft exemption applies before the repository-mismatch check)", () => {
  const { dir } = repoWithGithubOrigin("owner/brawler");
  assert.equal(runHook("gh --repo other/repo pr create --draft", dir), "allow");
});

test("gh pr create --draft allows without a stamp", () => {
  const { dir } = repo();
  assert.equal(runHook("gh pr create --draft", dir), "allow");
});

test("gh pr create -d allows without a stamp", () => {
  const { dir } = repo();
  assert.equal(runHook("gh pr create -d", dir), "allow");
});

// ---- refspec normalization (+ force marker, refs/heads/ prefix) and --repo doesn't eat the remote ----

test("git push origin +reviewed (force marker) with an open non-draft PR on reviewed and no stamp denies", () => {
  const { dir } = repo();
  const ghDir = gh(fakeGhAdvanced({ prHead: "reviewed", prNumber: 51 }));
  assert.equal(runHook("git push origin +reviewed", dir, { ghDir }), "deny");
});

test("git push origin refs/heads/reviewed with an open non-draft PR on reviewed and no stamp denies", () => {
  const { dir } = repo();
  const ghDir = gh(fakeGhAdvanced({ prHead: "reviewed", prNumber: 52 }));
  assert.equal(runHook("git push origin refs/heads/reviewed", dir, { ghDir }), "deny");
});

test("git push --repo origin reviewed does not consume 'reviewed' as the remote and denies on an open PR", () => {
  const { dir } = repo();
  const ghDir = gh(fakeGhAdvanced({ prHead: "reviewed", prNumber: 53 }));
  assert.equal(runHook("git push --repo origin reviewed", dir, { ghDir }), "deny");
});

test("git push origin refs/heads/feature:feature allows once the BRANCH's tree is stamped, even though a same-named unstamped TAG also exists (source stays qualified)", () => {
  const { dir } = repo();
  git(["branch", "feature"], dir);
  stampBranchTree(dir, "check-local", "feature"); // branch 'feature' tree is stamped
  writeFileSync(path.join(dir, "src.rs"), "fn main() { 555; }\n");
  git(["commit", "-q", "-am", "advance feat past feature"], dir); // 'feat' (checked out) now diverges from 'feature'
  git(["tag", "feature"], dir); // tag 'feature' points at the diverged, unstamped tree
  const ghDir = gh(fakeGhAdvanced({ prHead: "feature", prNumber: 81 }));
  assert.equal(runHook("git push origin refs/heads/feature:feature", dir, { ghDir }), "allow");
});

test("git push origin refs/heads/feature:feature denies when only the same-named TAG's tree is stamped (a bare 'feature' source would wrongly prefer the tag)", () => {
  const { dir } = repo();
  git(["branch", "feature"], dir); // branch 'feature' tree stays unstamped
  writeFileSync(path.join(dir, "src.rs"), "fn main() { 555; }\n");
  git(["commit", "-q", "-am", "advance feat past feature"], dir); // 'feat' (checked out) now diverges from 'feature'
  git(["tag", "feature"], dir); // tag 'feature' points at feat's current (about-to-be-stamped) tree
  stampCurrentTree(dir, "check-local"); // stamps HEAD == the tag's tree, not the branch's
  const ghDir = gh(fakeGhAdvanced({ prHead: "feature", prNumber: 82 }));
  assert.equal(runHook("git push origin refs/heads/feature:feature", dir, { ghDir }), "deny");
});

// ---- gh pr ready resolves the PR's head commit, not just its branch name ----

test("gh pr ready <n> denies naming a git fetch when the PR's head commit is not present locally", () => {
  const { dir } = repo();
  const ghDir = gh(fakeGhAdvanced({ viewArg: "14", viewHead: "other", viewOid: "1".repeat(40) }));
  const result = spawnSync("bash", [path.join(REPO_ROOT, ".claude/hooks/check-evidence.sh")], {
    input: JSON.stringify({ tool_name: "Bash", tool_input: { command: "gh pr ready 14" } }),
    encoding: "utf8",
    cwd: dir,
    env: { ...process.env, PATH: `${ghDir}:${process.env.PATH}`, BRAWLER_CHECK_EVIDENCE_OFF: "" },
  });
  const decision = JSON.parse(result.stdout.trim()).hookSpecificOutput;
  assert.equal(decision.permissionDecision, "deny");
  assert.match(decision.permissionDecisionReason, /git fetch origin other/);
});

test("gh pr ready <n> allows once the PR's head commit is present locally and its tree is freshly stamped", () => {
  const { dir } = repo();
  git(["branch", "other"], dir);
  stampBranchTree(dir, "check-local", "other");
  const otherOid = git(["rev-parse", "other"], dir);
  const ghDir = gh(fakeGhAdvanced({ viewArg: "14", viewHead: "other", viewOid: otherOid }));
  assert.equal(runHook("gh pr ready 14", dir, { ghDir }), "allow");
});

// ---- repository identity: gh --repo vs origin; git --git-dir/--work-tree ----

test("gh --repo other/repo pr create from a repo whose origin is owner/brawler denies even with a fresh stamp (evidence is per-checkout)", () => {
  const { dir } = repoWithGithubOrigin("owner/brawler");
  stampCurrentTree(dir, "check-local"); // rules out the generic "no stamp" deny — this must be the repo-identity deny
  assert.equal(runHook("gh --repo other/repo pr create", dir), "deny");
});

test("gh --repo <matching origin slug> pr create behaves the same as without --repo", () => {
  const { dir } = repoWithGithubOrigin("owner/brawler");
  assert.equal(runHook("gh --repo owner/brawler pr create", dir), "deny"); // no stamp
  writeFileSync(path.join(dir, "src.rs"), "fn main() { 1; }\n");
  git(["commit", "-q", "-am", "code change"], dir);
  stampCurrentTree(dir, "check-local");
  assert.equal(runHook("gh --repo owner/brawler pr create", dir), "allow");
});

for (const [label, url] of [
  ["ssh://", "ssh://git@github.com/owner/brawler.git"],
  ["git+ssh://", "git+ssh://git@github.com/owner/brawler.git"],
  ["https:// with a user@ prefix", "https://user@github.com/owner/brawler"],
  ["a trailing slash", "https://github.com/owner/brawler/"],
  ["a .git suffix plus a trailing slash", "https://github.com/owner/brawler.git/"],
]) {
  test(`gh --repo owner/brawler pr create matches an origin remote written as ${label}`, () => {
    const { dir } = repo();
    git(["remote", "set-url", "origin", url], dir);
    writeFileSync(path.join(dir, "src.rs"), "fn main() { 1; }\n");
    git(["commit", "-q", "-am", "code change"], dir);
    stampCurrentTree(dir, "check-local");
    assert.equal(runHook("gh --repo owner/brawler pr create", dir), "allow");
  });
}

test("git --git-dir/--work-tree push evaluates evidence in that repo, not the invoking cwd's repo (even when cwd is freshly stamped)", () => {
  const { dir: dirA } = repo();
  stampCurrentTree(dirA, "check-local"); // dirA's own 'feat' tree is fresh — must not be trusted for dirB
  const { dir: dirB } = repo();
  writeFileSync(path.join(dirB, "src.rs"), "fn main() { 99; }\n"); // dirB's 'feat' diverges, unstamped
  git(["commit", "-q", "-am", "other repo change"], dirB);
  const ghDir = gh(fakeGhAdvanced({ prHead: "feat", prNumber: 61 }));
  assert.equal(
    runHook(`git --git-dir=${dirB}/.git --work-tree=${dirB} push origin feat`, dirA, { ghDir }),
    "deny",
  );
});

// ---- gh pr create --head resolves directly as the local branch's tree ----

test("gh pr create --head feature allows once feature's own tree carries a fresh stamp", () => {
  const { dir } = repo();
  git(["branch", "feature"], dir);
  stampBranchTree(dir, "check-local", "feature");
  assert.equal(runHook("gh pr create --head feature", dir), "allow");
});

test("gh pr create --head feature denies without a stamp on feature's tree", () => {
  const { dir } = repo();
  git(["branch", "feature"], dir);
  assert.equal(runHook("gh pr create --head feature", dir), "deny");
});

test("gh pr create --head feature ignores the checked-out branch's own stamp (different content, wrong branch)", () => {
  const { dir } = repo();
  stampCurrentTree(dir, "check-local"); // stamps 'feat' (checked out), not 'feature'
  git(["checkout", "-q", "-b", "feature"], dir);
  writeFileSync(path.join(dir, "src.rs"), "fn main() { 7; }\n"); // feature diverges from feat's stamped tree
  git(["commit", "-q", "-am", "feature change"], dir);
  git(["checkout", "-q", "feat"], dir);
  assert.equal(runHook("gh pr create --head feature", dir), "deny");
});

// ---- --draft/-d detection is value-aware (a value never looks like the flag) ----

test('gh pr create --title "--draft" with no stamp denies (the value is not the flag)', () => {
  const { dir } = repo();
  assert.equal(runHook('gh pr create --title "--draft"', dir), "deny");
});

test("gh pr create --draft --title x allows (the real flag still works)", () => {
  const { dir } = repo();
  assert.equal(runHook("gh pr create --draft --title x", dir), "allow");
});
