// Company-scoped route/tool vocabulary (F3a S1, ADR 0107). `Tool` is the
// closed set of workshop-bar destinations the Spółka screen can request via
// `onOpenTool`; S1 only wires the callback (the tool host itself is S2).
export type Tool =
  | { t: "tezy"; claimId?: string }
  | { t: "feedItem"; feedItemId: string }
  | { t: "dokumenty"; documentId?: string }
  | { t: "feed" }
  | { t: "notatnik" }
  | { t: "dziennik" }
  | { t: "jakosc" }
  | { t: "diff" }
  | { t: "research" }
  | { t: "akcjonariat" }
  | { t: "sygnaly" }
  | { t: "fundamenty" }
  | { t: "pokrycie" }
  | { t: "rekomendacje" }
  | { t: "wydarzenia" };

export const TOOL_KINDS = [
  "tezy",
  "feedItem",
  "dokumenty",
  "feed",
  "notatnik",
  "dziennik",
  "jakosc",
  "diff",
  "research",
  "akcjonariat",
  "sygnaly",
  "fundamenty",
  "pokrycie",
  "rekomendacje",
  "wydarzenia",
] as const satisfies readonly Tool["t"][];

