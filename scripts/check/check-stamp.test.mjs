// Guard (S2, hard-gates closing wave): check-stamp's fingerprint must equal
// HEAD^{tree} on a clean tree, move when the tree actually changes, and stay
// put across a commit of identical content; its `write` CLI must refuse when
// the tree moved between the `before` and `after` fingerprint.
import { test, after as afterAll } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { fingerprint } from "./check-stamp.mjs";

const REPO_ROOT = path.resolve(fileURLToPath(new URL("../..", import.meta.url)));
const CLI_PATH = path.join(REPO_ROOT, "scripts/check/check-stamp.mjs");

function git(args, cwd) {
  const r = spawnSync("git", args, { cwd, encoding: "utf8" });
  if (r.status !== 0) throw new Error(`git ${args.join(" ")} failed in ${cwd}: ${r.stderr}`);
  return r.stdout.trim();
}

function makeRepo() {
  const dir = mkdtempSync(path.join(tmpdir(), "check-stamp-"));
  git(["init", "-q"], dir);
  git(["symbolic-ref", "HEAD", "refs/heads/master"], dir);
  git(["config", "user.email", "t@example.com"], dir);
  git(["config", "user.name", "test"], dir);
  writeFileSync(path.join(dir, "a.txt"), "hello\n");
  git(["add", "a.txt"], dir);
  git(["commit", "-q", "-m", "init"], dir);
  return dir;
}

const dirs = [];
afterAll(() => {
  for (const d of dirs) rmSync(d, { recursive: true, force: true });
});

test("fingerprint equals HEAD^{tree} on a clean committed repo", () => {
  const dir = makeRepo();
  dirs.push(dir);
  const headTree = git(["rev-parse", "HEAD^{tree}"], dir);
  assert.equal(fingerprint(dir), headTree);
});

test("fingerprint changes when an untracked file appears", () => {
  const dir = makeRepo();
  dirs.push(dir);
  const before = fingerprint(dir);
  writeFileSync(path.join(dir, "b.txt"), "new\n");
  const after = fingerprint(dir);
  assert.notEqual(before, after);
});

test("fingerprint is unchanged by a commit of identical content", () => {
  const dir = makeRepo();
  dirs.push(dir);
  const before = fingerprint(dir);
  writeFileSync(path.join(dir, "a.txt"), "hello\n"); // identical content, new mtime
  git(["add", "-A"], dir);
  git(["commit", "-q", "--allow-empty", "-m", "noop"], dir);
  const after = fingerprint(dir);
  assert.equal(before, after);
});

test("CLI write refuses when the tree changed between before and after", () => {
  const dir = makeRepo();
  dirs.push(dir);
  const before = fingerprint(dir);
  writeFileSync(path.join(dir, "c.txt"), "changed\n");
  const r = spawnSync("node", [CLI_PATH, "write", "check-local", before], { cwd: dir, encoding: "utf8" });
  assert.equal(r.status, 1);
  assert.match(r.stderr, /the working tree changed while check-local ran/);
  assert.equal(existsSync(path.join(dir, ".artifacts/check-local.json")), false);
});

test("CLI write succeeds and writes .artifacts/<target>.json when before == after", () => {
  const dir = makeRepo();
  dirs.push(dir);
  const before = fingerprint(dir);
  const r = spawnSync("node", [CLI_PATH, "write", "check-local", before], { cwd: dir, encoding: "utf8" });
  assert.equal(r.status, 0);
  const written = JSON.parse(readFileSync(path.join(dir, ".artifacts/check-local.json"), "utf8"));
  assert.equal(written.tree, before);
  assert.equal(written.target, "check-local");
  assert.equal(written.toplevel, dir);
});

test("fingerprint refuses when a file is marked assume-unchanged (stale index content survives git add -A)", () => {
  const dir = makeRepo();
  dirs.push(dir);
  git(["update-index", "--assume-unchanged", "a.txt"], dir);
  writeFileSync(path.join(dir, "a.txt"), "changed but hidden from the index\n");
  assert.throws(() => fingerprint(dir), /assume-unchanged.*a\.txt|a\.txt.*assume-unchanged/s);
  git(["update-index", "--no-assume-unchanged", "a.txt"], dir);
});

test("fingerprint refuses when a file is marked skip-worktree", () => {
  const dir = makeRepo();
  dirs.push(dir);
  git(["update-index", "--skip-worktree", "a.txt"], dir);
  assert.throws(() => fingerprint(dir), /skip-worktree.*a\.txt|a\.txt.*skip-worktree/s);
  git(["update-index", "--no-skip-worktree", "a.txt"], dir);
});

test("fingerprint refuses when a file is marked BOTH skip-worktree and assume-unchanged (ls-files -v 's')", () => {
  const dir = makeRepo();
  dirs.push(dir);
  git(["update-index", "--skip-worktree", "a.txt"], dir);
  git(["update-index", "--assume-unchanged", "a.txt"], dir);
  assert.throws(() => fingerprint(dir), /skip-worktree.*a\.txt|a\.txt.*skip-worktree/s);
  git(["update-index", "--no-skip-worktree", "a.txt"], dir);
  git(["update-index", "--no-assume-unchanged", "a.txt"], dir);
});

test("fingerprint refuses on a merge conflict", () => {
  const dir = makeRepo();
  dirs.push(dir);
  git(["checkout", "-q", "-b", "other"], dir);
  writeFileSync(path.join(dir, "a.txt"), "other\n");
  git(["commit", "-q", "-am", "other change"], dir);
  git(["checkout", "-q", "master"], dir);
  writeFileSync(path.join(dir, "a.txt"), "master\n");
  git(["commit", "-q", "-am", "master change"], dir);
  spawnSync("git", ["merge", "-q", "other"], { cwd: dir }); // expected to conflict
  assert.throws(() => fingerprint(dir), /unresolved merge conflict/);
  spawnSync("git", ["merge", "--abort"], { cwd: dir });
});
