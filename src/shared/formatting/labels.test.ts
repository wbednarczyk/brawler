import { describe, expect, it } from "vitest";
import { JOB_KINDS } from "./jobKinds";
import { formatJobKindDisplayName, JOB_KIND_LABELS } from "./labels";

describe("formatJobKindDisplayName", () => {
  it("has an explicit label for every kind in the checked-in registry list (ADR 0109 parity)", () => {
    for (const kind of JOB_KINDS) {
      expect(JOB_KIND_LABELS[kind], `missing explicit label for job kind "${kind}"`).toBeDefined();
      expect(formatJobKindDisplayName(kind)).toBe(JOB_KIND_LABELS[kind]);
    }
  });
});
