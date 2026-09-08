import { Fragment, useEffect, useRef, useState } from "react";
import {
  search as runSearch,
  type SearchContentType,
  type SearchMatch,
  type SearchResults,
} from "../api/search";
import { makeTranslator, type LocaleKey } from "../shared/locale";
import type { AppLocale } from "../api/types";
import { SearchField, useComboboxListbox, type ComboboxEscapeAction, type ComboboxEscapeState } from "../ui";

const GROUP_ORDER: SearchContentType[] = [
  "company",
  "watchlist",
  "feed_item",
  "notebook_entry",
  "transcript_segment",
  "event",
  "research_brief",
  "digest",
];

// Highlight markers emitted by the storage snippet() call (search.rs) are the
// STX/ETX control characters, not HTML, so snippets render as plain text.
const HIGHLIGHT_START = String.fromCharCode(2);
const HIGHLIGHT_END = String.fromCharCode(3);
const SEARCH_DEBOUNCE_MS = 180;

type GlobalSearchProps = {
  locale: AppLocale;
  onNavigate: (match: SearchMatch) => void;
};

// A `SearchMatch` carries no globally unique id of its own (`sourceId` is
// only unique within its `contentType`) — the composite is the controller's
// stable option identity.
function matchId(match: SearchMatch): string {
  return `${match.contentType}:${match.sourceId}`;
}

// Escape (ADR 0107 policy, shared with the Spółka company picker): open list
// → close; closed + a query typed → clear; closed + empty → bubble (this
// widget has no dirty seam of its own to guard, so bubbling just leaves the
// event for whatever ancestor wants it).
function globalSearchEscapePolicy({ query, isOpen }: ComboboxEscapeState): ComboboxEscapeAction {
  if (isOpen) return "close-list";
  if (query.trim() !== "") return "clear";
  return "bubble";
}

// Render a snippet whose highlighted ranges are wrapped in the control-character
// markers. Text is rendered as React text nodes (never HTML), so untrusted source
// content cannot inject markup.
function renderSnippet(snippet: string) {
  const segments = snippet.split(HIGHLIGHT_START);
  return segments.map((segment, index) => {
    if (index === 0) {
      return <Fragment key={index}>{segment}</Fragment>;
    }
    const [highlighted, ...rest] = segment.split(HIGHLIGHT_END);
    return (
      <Fragment key={index}>
        <mark>{highlighted}</mark>
        {rest.join(HIGHLIGHT_END)}
      </Fragment>
    );
  });
}

export function GlobalSearch({ locale, onNavigate }: GlobalSearchProps) {
  const t = makeTranslator(locale);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  // Results are keyed by the query that produced them: a keystroke makes the
  // previous set stale in the same render (no window where old rows are the
  // controller's options for the new query).
  const [resultsState, setResultsState] = useState<{ query: string; results: SearchResults } | null>(null);

  function selectMatch(match: SearchMatch) {
    onNavigate(match);
    controller.reset();
  }

  // Rows for a query are the settled results keyed by that exact query —
  // anything else (pending, stale) is an empty option set.
  function groupsFor(query: string) {
    const settled = resultsState && resultsState.query === query.trim() ? resultsState.results : null;
    return settled
      ? GROUP_ORDER.map((contentType) => settled.groups.find((group) => group.contentType === contentType)).filter(
          (group): group is NonNullable<typeof group> => Boolean(group),
        )
      : null;
  }

  const controller = useComboboxListbox({
    options: (query) => groupsFor(query)?.flatMap((group) => group.matches) ?? [],
    getId: matchId,
    filter: () => true,
    onSelect: selectMatch,
    escapePolicy: globalSearchEscapePolicy,
  });

  const trimmedQuery = controller.query.trim();
  const orderedGroups = groupsFor(trimmedQuery);
  const isSearching = trimmedQuery !== "" && orderedGroups === null;

  useEffect(() => {
    const trimmed = controller.query.trim();
    if (trimmed === "") {
      setResultsState(null);
      return;
    }
    let cancelled = false;
    const handle = window.setTimeout(() => {
      runSearch({ query: trimmed })
        .then((found) => {
          if (!cancelled) setResultsState({ query: trimmed, results: found });
        })
        .catch(() => {
          if (!cancelled) setResultsState({ query: trimmed, results: { groups: [] } });
        });
    }, SEARCH_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(handle);
    };
  }, [controller.query]);

  useEffect(() => {
    function handlePointerDown(event: PointerEvent) {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        controller.close();
      }
    }

    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `controller.close` reads current state at call time
  }, []);

  function clearQuery() {
    controller.reset();
    inputRef.current?.focus();
  }

  const showPanel = controller.isOpen && trimmedQuery !== "";

  return (
    <div className="global-search" ref={containerRef} data-global-search>
      <SearchField
        className="search-box"
        ariaLabel={t("globalSearch.ariaLabel")}
        placeholder={t("globalSearch.placeholder")}
        value={controller.query}
        // A blank query (whitespace included) never opens the controller: an
        // invisible-open state would consume the first Escape.
        onChange={(value) => (value.trim() === "" ? controller.reset(value) : controller.setQuery(value))}
        onClear={clearQuery}
        clearLabel={t("globalSearch.clear")}
        inputProps={{
          ref: inputRef,
          "data-global-search-input": true,
          role: controller.inputProps.role,
          // The exposed state follows the VISIBLE panel: an empty query never
          // reports an expanded combobox with no listbox in the document.
          "aria-expanded": showPanel,
          "aria-autocomplete": controller.inputProps["aria-autocomplete"],
          "aria-controls": showPanel ? controller.inputProps["aria-controls"] : undefined,
          "aria-activedescendant": controller.inputProps["aria-activedescendant"],
          onKeyDown: controller.inputProps.onKeyDown,
          onFocus: () => {
            if (controller.query.trim() !== "") controller.open();
          },
        }}
      />
      {showPanel ? (
        <div {...controller.listboxProps} className="global-search-results" aria-label={t("globalSearch.ariaLabel")}>
          {isSearching ? (
            <div className="global-search-status">{t("globalSearch.searching")}</div>
          ) : !orderedGroups || orderedGroups.length === 0 ? (
            <div className="global-search-status">{t("globalSearch.noResults")}</div>
          ) : (
            orderedGroups.map((group) => (
              <div className="global-search-group" key={group.contentType}>
                <div className="global-search-group-title">
                  {t(`globalSearch.group.${group.contentType}` as LocaleKey)}
                </div>
                {group.matches.map((match) => (
                  <button
                    type="button"
                    className="global-search-result"
                    key={matchId(match)}
                    {...controller.optionProps(match)}
                  >
                    <span className="global-search-result-title">{match.title}</span>
                    <span className="global-search-result-snippet">{renderSnippet(match.snippet)}</span>
                  </button>
                ))}
              </div>
            ))
          )}
        </div>
      ) : null}
    </div>
  );
}
