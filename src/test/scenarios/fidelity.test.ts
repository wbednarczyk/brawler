import { describe, expect, it } from "vitest";

import corpus from "./fidelity-corpus.json";
import { createMockRuntime } from "./runtime";

// Dual-execution mock-fidelity contract — TS mock-runtime side (ADR 0049, T6).
//
// Replays the SHARED journey corpus against the mock runtime (createMockRuntime).
// The Rust side (src-tauri/src/storage/tests/mock_fidelity.rs) replays the SAME
// corpus against the real AppState/storage layer. Both must satisfy every
// assertion, so the hand-written mock cannot silently drift from real backend
// behavior. ts-rs already guarantees DTO shapes match; this guarantees behavior.

interface Step {
  command: string;
  input?: unknown;
  capture?: string;
  expectField?: Record<string, unknown>;
  expectContains?: Record<string, unknown>;
  expectAbsent?: Record<string, unknown>;
  /**
   * When the result is an OBJECT (not itself an array), name the array-valued
   * field `expectContains`/`expectAbsent` scan (e.g. `"active"` or `"queued"`
   * on an `ActivityView`). Omitted when the result IS the array (the default,
   * unchanged behavior).
   */
  expectContainsField?: string;
  /**
   * Dotted-path deep-equality pins (e.g. `"kpi.rows.0.yoyPct"`), for nested
   * fields `expectField`'s shallow `===` can't reach. Read by BOTH replayers
   * (mock_fidelity.rs implements the same dotted-path lookup), so a deep pin
   * is a dual-execution assertion like `expectField`.
   */
  expectDeep?: Record<string, unknown>;
}
interface Journey {
  name: string;
  steps: Step[];
}

/** Replace any `"$name"` leaf with the captured value of `name`. */
function substitute(value: unknown, caps: Record<string, unknown>): unknown {
  if (typeof value === "string" && value.startsWith("$")) return caps[value.slice(1)];
  if (Array.isArray(value)) return value.map((entry) => substitute(entry, caps));
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([key, entry]) => [
        key,
        substitute(entry, caps),
      ]),
    );
  }
  return value;
}

/** Resolve a dotted path (numeric segments index into arrays) against `value`. */
function getByPath(value: unknown, path: string): unknown {
  return path.split(".").reduce<unknown>((node, key) => {
    if (node === null || typeof node !== "object") return undefined;
    return (node as Record<string, unknown>)[key];
  }, value);
}

/** Structural equality (JSON-shaped values only — the DTOs this corpus checks
 * carry no functions/undefined), mirroring the Rust replayer's
 * `serde_json::Value` `==`, which is already deep for objects/arrays. Lets
 * `isSuperset` match nested-object subset values (e.g. `target: { kind: "…" }`),
 * not just primitives. */
function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (Array.isArray(a) && Array.isArray(b)) {
    return a.length === b.length && a.every((v, i) => deepEqual(v, b[i]));
  }
  if (a && b && typeof a === "object" && typeof b === "object") {
    const aKeys = Object.keys(a as Record<string, unknown>);
    const bKeys = Object.keys(b as Record<string, unknown>);
    if (aKeys.length !== bKeys.length) return false;
    return aKeys.every((key) =>
      deepEqual(
        (a as Record<string, unknown>)[key],
        (b as Record<string, unknown>)[key],
      ),
    );
  }
  return false;
}

/** True when `actual` is an object containing every key/value pair in `subset`. */
function isSuperset(actual: unknown, subset: Record<string, unknown>): boolean {
  if (!actual || typeof actual !== "object") return false;
  const object = actual as Record<string, unknown>;
  return Object.entries(subset).every(([key, value]) => deepEqual(object[key], value));
}

describe("mock-fidelity corpus — TS mock runtime side (ADR 0049 T6)", () => {
  const journeys = (corpus as { journeys: Journey[] }).journeys;
  expect(journeys.length).toBeGreaterThan(0);

  for (const journey of journeys) {
    it(`mock runtime satisfies: ${journey.name}`, async () => {
      const runtime = createMockRuntime("minimal");
      const caps: Record<string, unknown> = {};

      for (const step of journey.steps) {
        const input = substitute(step.input ?? {}, caps) as Record<string, unknown>;
        const result = await runtime.invoke(step.command, input);

        if (step.capture) caps[step.capture] = (result as { id: unknown }).id;
        if (step.expectField) {
          expect(isSuperset(result, step.expectField), `${step.command} expectField`).toBe(true);
        }
        if (step.expectContains) {
          const subset = step.expectContains;
          const array = step.expectContainsField
            ? (result as Record<string, unknown>)[step.expectContainsField]
            : result;
          expect(Array.isArray(array), `${step.command} returns array`).toBe(true);
          expect(
            (array as unknown[]).some((item) => isSuperset(item, subset)),
            `${step.command} expectContains`,
          ).toBe(true);
        }
        if (step.expectAbsent) {
          const subset = step.expectAbsent;
          const array = step.expectContainsField
            ? (result as Record<string, unknown>)[step.expectContainsField]
            : result;
          expect(Array.isArray(array), `${step.command} returns array`).toBe(true);
          expect(
            (array as unknown[]).every((item) => !isSuperset(item, subset)),
            `${step.command} expectAbsent`,
          ).toBe(true);
        }
        if (step.expectDeep) {
          for (const [path, expected] of Object.entries(step.expectDeep)) {
            expect(getByPath(result, path), `${step.command} expectDeep ${path}`).toEqual(expected);
          }
        }
      }
    });
  }
});
