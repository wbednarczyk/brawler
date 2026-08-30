// The palette verb dictionary (ADR 0104 dec. 3, F3a S3): every palette command
// label starts with one of these verbs. Enforced by the copy gate
// (src/app/paletteCopy.test.ts). See docs/ui-authoring.md § i18n.
//
// F4a S1 (ADR 0104 dec. 3 amendment, 2026-08-28): five verbs added for the
// Library screens' action inventories (`ActionButton`, F4a contract) —
// `create`/`rename`/`pause`/`resume`, plus `remove` as the ONLY
// collection-removal verb (the legacy screen-copy key `Delete` is retired).
// F4b S1 (ADR 0104 dec. 3 amendment, 2026-08-30): `edit` (enter edit mode of
// an existing record, persisted by a later `save`), `confirm`/`reject`
// (accept/decline a proposed record — proposals only) join the dictionary.
export type Verb =
  | "open"
  | "apply"
  | "save"
  | "fetch"
  | "read"
  | "refresh"
  | "markAs"
  | "add"
  | "remove"
  | "create"
  | "rename"
  | "pause"
  | "resume"
  | "edit"
  | "confirm"
  | "reject";

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
  create: { en: "Create", pl: "Utwórz" },
  rename: { en: "Rename", pl: "Zmień nazwę" },
  pause: { en: "Pause", pl: "Wstrzymaj" },
  resume: { en: "Resume", pl: "Wznów" },
  edit: { en: "Edit", pl: "Zmień" },
  confirm: { en: "Confirm", pl: "Potwierdź" },
  reject: { en: "Reject", pl: "Odrzuć" },
};
