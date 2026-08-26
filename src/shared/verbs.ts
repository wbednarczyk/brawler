// The palette verb dictionary (ADR 0104 dec. 3, F3a S3): every palette command
// label starts with one of these verbs. Enforced by the copy gate
// (src/app/paletteCopy.test.ts). See docs/ui-authoring.md § i18n.
export type Verb =
  | "open"
  | "apply"
  | "save"
  | "fetch"
  | "read"
  | "refresh"
  | "markAs"
  | "add"
  | "remove";

export const VERB_LABELS: Record<Verb, { en: string; pl: string }> = {
  open: { en: "Open", pl: "Otwórz" },
  apply: { en: "Apply", pl: "Zastosuj" },
  save: { en: "Save", pl: "Zapisz" },
  fetch: { en: "Fetch", pl: "Pobierz" },
  read: { en: "Read", pl: "Przeczytaj" },
  refresh: { en: "Refresh", pl: "Odśwież" },
  markAs: { en: "Mark as", pl: "Oznacz jako" },
  add: { en: "Add", pl: "Dodaj" },
  remove: { en: "Remove", pl: "Usuń" },
};
