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

// The workshop bar's destination entries (F3a, moved here in F3c S1 so the
// bar, the keyboard cycle (`useSpolkaKeyboard`) and `workshopIndexOf` share
// ONE order). Destination labels are nouns, not verbs (ADR 0104 dec. 3
// amendment) — see `SPOLKA_TOOL_COMMANDS` in `SpolkaScreen.tsx` for the
// palette's verb-prefixed twin.
export const WORKSHOP_TOOLS: Array<{ tool: Tool; label: string }> = [
  { tool: { t: "fundamenty" }, label: "Fundamentals" },
  { tool: { t: "feed" }, label: "Feed" },
  { tool: { t: "pokrycie" }, label: "Coverage" },
  { tool: { t: "rekomendacje" }, label: "Recommendations" },
  { tool: { t: "tezy" }, label: "Claims" },
  { tool: { t: "notatnik" }, label: "Notebook" },
  { tool: { t: "dziennik" }, label: "Decision journal" },
  { tool: { t: "jakosc" }, label: "Quality" },
  { tool: { t: "diff" }, label: "Report diff" },
  { tool: { t: "research" }, label: "Research" },
  { tool: { t: "akcjonariat" }, label: "Ownership" },
  { tool: { t: "sygnaly" }, label: "Signals" },
  { tool: { t: "dokumenty" }, label: "Documents" },
  { tool: { t: "wydarzenia" }, label: "Events" },
];

// The workshop bar's roving-tabindex/selection index for a committed tool
// (F3c S1, plan § Design 1): Overview = 0, WORKSHOP_TOOLS = 1..14. `feedItem`
// has no bar entry of its own (opened only from the Inbox) — it maps to
// Feed's index, so closing it returns focus to a real, visible entry. Accepts
// either the full `Tool` or just its discriminant (`focus.closedKind` on
// `SpolkaToolHostApi` carries only the kind, not the whole tool).
export function workshopIndexOf(toolOrKind: Tool | Tool["t"] | null): number {
  if (toolOrKind === null) return 0;
  const kind = typeof toolOrKind === "string" ? toolOrKind : toolOrKind.t;
  const effectiveKind = kind === "feedItem" ? "feed" : kind;
  const index = WORKSHOP_TOOLS.findIndex((entry) => entry.tool.t === effectiveKind);
  return index === -1 ? 0 : index + 1;
}
