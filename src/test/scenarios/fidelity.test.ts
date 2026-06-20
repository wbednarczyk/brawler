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

/** True when `actual` is an object containing every key/value pair in `subset`. */
function isSuperset(actual: unknown, subset: Record<string, unknown>): boolean {
  if (!actual || typeof actual !== "object") return false;
  const object = actual as Record<string, unknown>;
  return Object.entries(subset).every(([key, value]) => object[key] === value);
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
          expect(Array.isArray(result), `${step.command} returns array`).toBe(true);
          expect(
            (result as unknown[]).some((item) => isSuperset(item, subset)),
            `${step.command} expectContains`,
          ).toBe(true);
        }
        if (step.expectAbsent) {
          const subset = step.expectAbsent;
          expect(Array.isArray(result), `${step.command} returns array`).toBe(true);
          expect(
            (result as unknown[]).every((item) => !isSuperset(item, subset)),
            `${step.command} expectAbsent`,
          ).toBe(true);
        }
      }
    });
  }
});
