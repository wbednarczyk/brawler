/**
 * Splits a stored report-document title into a human statement and a leading
 * filename token. Source `report_documents.title`
 * values are sometimes a filename fused directly onto the human title
 * (e.g. "Y24_25_Sprawozdanie jednostkowe.xhtmlJednostkowe Sprawozdanie Finansowe
 * AB S.A.") or a raw filename alone ("2410_Passus_2023_PSSF_MSSF_skrócone_PL-sig.pdf").
 * A filename is metadata, not prose: it belongs on a quiet secondary link line,
 * not in the row's statement.
 *
 * Model: `[FILENAME][HUMAN]` — the filename leads (it is the raw artifact the
 * pipeline saw first). Detection keys on a known document extension acting as the
 * filename's boundary; everything up to and including it is the filename, whatever
 * follows (trimmed of separators / camel-case glue) is the human statement.
 *
 * - Glued → clean split (filename + statement).
 * - Filename-only → `statement: null` (caller substitutes its generic copy),
 *   filename surfaced.
 * - Human-only (no extension) → passes through unchanged as the statement.
 * - A title that merely contains dots (no known extension) never splits.
 */

// Ordered longest-first so ".xhtml" wins over its ".html"/".htm" substrings, and
// ".html" over ".htm". Case-insensitive. The extension need not be followed by a
// separator — it may be glued directly to the human title (…".xhtmlJednostkowe…").
//
// EXPORTED as the single source for this pattern: the anti-filename CI gate
// (`streamCopy.test.tsx`) and the live specificity probe
// (`ux-checkpoint.live.spec.ts`) reuse THIS regex to assert no rendered row
// statement ever carries a document extension — a statement is prose, a filename
// is metadata. Never `/g` (would make `.test()`/`.exec()` stateful); `/i` only.
export const FILENAME_EXTENSION = /\.(?:xhtml|html|htm|pdf|zip)/i;

/**
 * A stricter GLUE probe: a document extension
 * immediately fused to a following word character (`"…jednostkowe.xhtmlJednostkowe…"`)
 * — the exact class that leaked a filename into a row statement. Reused by the
 * anti-filename gate so the glue case reddens on its own, independent of the
 * broader "any extension present" check.
 */
export const FILENAME_EXTENSION_GLUE = /\.(?:xhtml|html|htm|pdf|zip)\w/i;

// Separators / glue trimmed from the FRONT of the human remainder after the split:
// whitespace, dashes/en–em dashes, colons, semicolons, commas, and leading dots.
const LEADING_SEPARATORS = /^[\s–—:;,.-]+/;

export type SplitDocumentTitle = {
  /** The human-readable statement, or `null` when the source was filename-only. */
  statement: string | null;
  /** The leading filename token, or `null` when the title carries no filename. */
  filename: string | null;
};

export function splitDocumentTitle(raw: string | null | undefined): SplitDocumentTitle {
  const trimmed = (raw ?? "").trim();
  if (!trimmed) return { statement: null, filename: null };

  const match = FILENAME_EXTENSION.exec(trimmed);
  if (!match) return { statement: trimmed, filename: null };

  const splitAt = match.index + match[0].length;
  const filename = trimmed.slice(0, splitAt).trim();
  const remainder = trimmed.slice(splitAt).replace(LEADING_SEPARATORS, "").trim();
  return { statement: remainder.length > 0 ? remainder : null, filename: filename || null };
}
