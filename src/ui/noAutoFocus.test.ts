import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// F3c (#197, ADR 0076 dec. 9 amendment): a JSX `autoFocus` fires during
// React's commit, BEFORE any effect — inside a `Modal` it becomes the node
// the dialog "restores" focus to (the palette could not return focus to its
// invoker for months); anywhere else it moves focus on mount without an
// explicit intent. Initial focus is always an explicit ref (`Modal`'s
// `initialFocusRef`, a focus intent, `useFocusAfterRemove`). Source scan over
// every component file; comments mentioning the word do not count.

const SRC = join(process.cwd(), "src");

function listTsx(dir: string, out: string[]): string[] {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) listTsx(full, out);
    else if (entry.endsWith(".tsx") && !entry.endsWith(".test.tsx")) out.push(full);
  }
  return out;
}

describe("no JSX autoFocus anywhere in src", () => {
  it("every component sets initial focus through an explicit ref/intent", () => {
    const offenders = listTsx(SRC, [])
      .filter((file) => {
        const source = readFileSync(file, "utf8").replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/.*$/gm, "");
        return /\bautoFocus\b/.test(source);
      })
      .map((file) => file.slice(process.cwd().length + 1));
    expect(offenders).toEqual([]);
  });
});
