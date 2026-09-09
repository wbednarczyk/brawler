#!/usr/bin/env node
// check-stamp (S2, hard-gates closing wave): certifies "this exact
// Git-normalized working tree passed <target>" so check-evidence.mjs can
// require a fresh green run before `gh pr create`/a push to an open PR (DoD
// §K). The fingerprint is `git write-tree` computed against a scratch copy
// of the index after `git add -A` — i.e. Git-normalized content (clean
// filters, CRLF normalization), never the raw tested bytes; treat it as
// evidence of "this tree", not proof of the exact bytes a test process read.
// Refuses outright (before fingerprinting) when any index entry is marked
// assume-unchanged or skip-worktree: `git add -A` leaves such entries at
// their stale cached content, so the fingerprint would certify a tree that
// was never actually re-tested. A hand-edited `.artifacts/*.json` stamp is
// outside this script's own reach entirely — local bookkeeping, never
// authenticated evidence.
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, renameSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";

function run(args, opts = {}) {
  const r = spawnSync("git", args, { encoding: "utf8", ...opts });
  if (r.status !== 0) throw new Error(`git ${args.join(" ")} failed: ${r.stderr || r.stdout}`);
  return r.stdout.trim();
}

/** Git-normalized content fingerprint of `cwd`'s working tree, as a tree hash. Throws on an
 * unresolved merge conflict or a dirty submodule — this stamp only ever certifies a clean tree. */
export function fingerprint(cwd) {
  const toplevel = run(["rev-parse", "--show-toplevel"], { cwd });
  const staleFlag = run(["ls-files", "-v"], { cwd })
    .split("\n")
    .find((l) => l[0] === "h" || l[0] === "S" || l[0] === "s");
  if (staleFlag) {
    const flagChar = staleFlag[0];
    const filePath = staleFlag.slice(2);
    const [flagName, fix] =
      flagChar === "h"
        ? ["assume-unchanged", "--no-assume-unchanged"]
        : flagChar === "s"
          ? ["skip-worktree + assume-unchanged", "--no-skip-worktree --no-assume-unchanged"]
          : ["skip-worktree", "--no-skip-worktree"];
    throw new Error(
      `check-stamp: ${filePath} is marked ${flagName} — \`git add -A\` leaves it at stale cached content; ` +
        `run \`git update-index ${fix} ${filePath}\` before stamping`,
    );
  }
  const conflicts = run(["diff", "--name-only", "--diff-filter=U"], { cwd });
  if (conflicts) throw new Error(`check-stamp: unresolved merge conflict(s):\n${conflicts}`);
  const submoduleStatus = run(["submodule", "status", "--recursive"], { cwd });
  if (submoduleStatus.split("\n").some((l) => /^[+\-U]/.test(l))) {
    throw new Error(`check-stamp: dirty submodule(s):\n${submoduleStatus}`);
  }
  const indexPath = run(["rev-parse", "--path-format=absolute", "--git-path", "index"], { cwd });
  if (!existsSync(indexPath)) throw new Error(`check-stamp: index file not found at ${indexPath}`);
  const tmpIndex = `${indexPath}.check-stamp-${process.pid}`;
  copyFileSync(indexPath, tmpIndex);
  try {
    const env = { ...process.env, GIT_INDEX_FILE: tmpIndex };
    run(["add", "-A"], { cwd: toplevel, env });
    return run(["write-tree"], { cwd: toplevel, env });
  } finally {
    rmSync(tmpIndex, { force: true });
  }
}

function atomicWriteJson(outPath, data) {
  mkdirSync(path.dirname(outPath), { recursive: true });
  const tmp = `${outPath}.tmp-${process.pid}`;
  writeFileSync(tmp, `${JSON.stringify(data, null, 2)}\n`);
  renameSync(tmp, outPath);
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === new URL(import.meta.url).pathname;
if (isMain) {
  const [, , cmd, ...rest] = process.argv;
  const cwd = process.cwd();
  if (cmd === "fingerprint") {
    console.log(fingerprint(cwd));
  } else if (cmd === "write") {
    const [target, beforeTree] = rest;
    if (!target || !beforeTree) {
      console.error("Usage: node scripts/check/check-stamp.mjs write <target> <before-tree>");
      process.exit(64);
    }
    const after = fingerprint(cwd);
    if (after !== beforeTree) {
      console.error(`✖ check-stamp: the working tree changed while ${target} ran — re-run ${target}`);
      process.exit(1);
    }
    const toplevel = run(["rev-parse", "--show-toplevel"], { cwd });
    const outPath = path.join(toplevel, ".artifacts", `${target}.json`);
    atomicWriteJson(outPath, { target, tree: after, toplevel, at: new Date().toISOString() });
    console.log(`check-stamp: wrote ${outPath} (tree ${after}).`);
  } else {
    console.error("Usage: node scripts/check/check-stamp.mjs fingerprint | write <target> <before-tree>");
    process.exit(64);
  }
}
