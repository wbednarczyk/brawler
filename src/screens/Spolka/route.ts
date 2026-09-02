import type { NotebookDraftOrigin } from "../../api/types";
import type { NotebookForm } from "../../shared/types/notebook";

// A prefilled-but-unsaved note (F4c S2, ADR 0108 amendment, sol re-review):
// the `notatnik` tool's `draft` param carries an origin-attributed composer
// seed (e.g. an Inbox feed item) — never persisted until the user saves it.
export type NotebookDraft = {
  form: NotebookForm;
  origins: NotebookDraftOrigin[];
};

// Company-scoped route/tool vocabulary (F3a S1, ADR 0107). `Tool` is the
// closed set of workshop-bar destinations the Spółka screen can request via
// `onOpenTool`; S1 only wires the callback (the tool host itself is S2).
export type Tool =
  | { t: "tezy"; claimId?: string }
  | { t: "feedItem"; feedItemId: string }
  | { t: "dokumenty"; documentId?: string }
  | { t: "feed" }
  // F4c S2 (ADR 0108 amendment): the Notebooks-global screen retired — every
  // deep link that used to land there opens this tool instead, either
  // highlighting an existing entry (`entryId`) or prefilling the composer
  // with an origin-attributed draft (`draft`, e.g. from an Inbox feed item).
  | { t: "notatnik"; entryId?: string; draft?: NotebookDraft }
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

// The `notatnik` tool's payload without its discriminant — the shape every
// cross-screen deep-link caller (Inbox, research evidence, global search,
// transcript) builds and hands to `navigateToCompanyNotebook` (AppStateRoot,
// sol re-review: the ONE landing point, never `spolkaTool.openTool`).
export type NotebookToolIntent = Omit<Extract<Tool, { t: "notatnik" }>, "t">;

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
