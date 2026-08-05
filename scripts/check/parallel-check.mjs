#!/usr/bin/env node
// Staged concurrent check orchestrator (ADR 0048, decision 5).
//
// Stage 1 (fast-fail): cheap static checks run concurrently; if any fails we
// stop before paying for the heavy stage. Stage 2: the heavy independent
// suites run concurrently — the win is overlapping the Rust compile (whose
// link/codegen phases leave cores idle) with the JS suites that need no Rust
// build. Output is captured per task and printed grouped; the first failing
// stage hard-stops (its siblings finish so you see every result, then we exit).
//
// This is opt-in (`make check-local` / `npm run check:parallel`); the canonical
// sequential `make check` stays the release-gate parity path until a measured
// before/after win promotes this to the default. Run it inside `nix develop`
// so npm/cargo resolve to the pinned toolchain.
//
// Worker-cap guardrail: the heavy suites each parallelize internally, so we cap
// them (nextest test-threads, vitest pool) so their sum does not oversubscribe
// the CPU and cause false-timeout flakiness — see ADR 0048.

import { spawn } from "node:child_process";
import os from "node:os";

const cores = os.cpus().length;
// Split the cores across the three heavy suites so they don't each assume all.
const rustThreads = Math.max(2, Math.floor(cores / 2));
const vitestThreads = Math.max(2, Math.floor(cores / 4));

const stages = [
  {
    title: "Stage 1 — static (fast-fail)",
    tasks: [
      { name: "typecheck", cmd: "npm", args: ["run", "typecheck"] },
      // Guardrail (ADR 0045): every dependency engines.node floor must be
      // satisfied by the Node this shell runs — see engines-floor.mjs.
      { name: "engines-floor", cmd: "node", args: ["scripts/check/engines-floor.mjs"] },
      { name: "lint", cmd: "npm", args: ["run", "lint"] },
      { name: "stylelint", cmd: "npm", args: ["run", "stylelint"] },
      { name: "rust-fmt", cmd: "cargo", args: ["fmt", "--check"], cwd: "src-tauri" },
    ],
  },
  {
    title: "Stage 2 — heavy suites (concurrent)",
    tasks: [
      {
        name: "rust (clippy+nextest+doc)",
        cmd: "bash",
        args: [
          "-c",
          `cargo clippy --all-targets -- -D warnings && cargo nextest run --test-threads ${rustThreads} && cargo test --doc`,
        ],
        cwd: "src-tauri",
      },
      {
        name: "vitest",
        cmd: "npx",
        args: ["vitest", "run", "src", "--pool=threads", `--maxWorkers=${vitestThreads}`],
      },
      { name: "build", cmd: "npm", args: ["run", "build"] },
    ],
  },
];

function runTask(task) {
  const start = Date.now();
  return new Promise((resolve) => {
    const child = spawn(task.cmd, task.args, {
      cwd: task.cwd ?? process.cwd(),
      env: process.env,
    });
    let output = "";
    child.stdout.on("data", (chunk) => (output += chunk));
    child.stderr.on("data", (chunk) => (output += chunk));
    child.on("close", (code) => {
      resolve({ name: task.name, code: code ?? 1, output, ms: Date.now() - start });
    });
    child.on("error", (err) => {
      resolve({ name: task.name, code: 1, output: String(err), ms: Date.now() - start });
    });
  });
}

async function main() {
  const overall = Date.now();
  for (const stage of stages) {
    process.stdout.write(`\n▶ ${stage.title}\n`);
    const results = await Promise.all(stage.tasks.map(runTask));
    for (const r of results) {
      const status = r.code === 0 ? "PASS" : "FAIL";
      process.stdout.write(`  ${status} ${r.name} (${(r.ms / 1000).toFixed(1)}s)\n`);
    }
    const failed = results.filter((r) => r.code !== 0);
    if (failed.length > 0) {
      for (const r of failed) {
        process.stdout.write(`\n──── ${r.name} output ────────────────────────────\n`);
        process.stdout.write(r.output.trimEnd() + "\n");
      }
      process.stdout.write(`\n✖ ${stage.title}: ${failed.length} task(s) failed — stopping.\n`);
      process.exit(1);
    }
  }
  process.stdout.write(`\n✓ All stages passed in ${((Date.now() - overall) / 1000).toFixed(1)}s.\n`);
}

main();
