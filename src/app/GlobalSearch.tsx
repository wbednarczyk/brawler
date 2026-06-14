import { Fragment, useEffect, useRef, useState } from "react";
import { Search, X } from "lucide-react";
import {
  search as runSearch,
  type SearchContentType,
  type SearchMatch,
  type SearchResults,
} from "../api/search";
import { makeTranslator, type LocaleKey } from "../shared/locale";
import type { AppLocale } from "../api/types";

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
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResults | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [isOpen, setIsOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const trimmed = query.trim();
    if (trimmed === "") {
      setResults(null);
      setIsSearching(false);
      return;
    }

    let cancelled = false;
    setIsSearching(true);
    const handle = window.setTimeout(() => {
      runSearch({ query: trimmed })
        .then((found) => {
          if (!cancelled) {
            setResults(found);
            setIsSearching(false);
          }
        })
        .catch(() => {
          if (!cancelled) {
            setResults({ groups: [] });
            setIsSearching(false);
          }
        });
    }, SEARCH_DEBOUNCE_MS);

    return () => {
      cancelled = true;
      window.clearTimeout(handle);
    };
  }, [query]);

  useEffect(() => {
    function handlePointerDown(event: PointerEvent) {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    }

    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, []);

  function selectMatch(match: SearchMatch) {
    onNavigate(match);
    setIsOpen(false);
    setQuery("");
    setResults(null);
  }

  function clearQuery() {
    setQuery("");
    setResults(null);
    inputRef.current?.focus();
  }

  const orderedGroups = results
    ? GROUP_ORDER.map((contentType) =>
        results.groups.find((group) => group.contentType === contentType),
      ).filter((group): group is NonNullable<typeof group> => Boolean(group))
    : [];

  const showPanel = isOpen && query.trim() !== "";

  return (
    <div className="global-search" ref={containerRef} data-global-search>
      <Search size={14} aria-hidden="true" className="global-search-icon" />
      <input
        ref={inputRef}
        data-global-search-input
        className="global-search-input"
        type="search"
        value={query}
        placeholder={t("globalSearch.placeholder")}
        aria-label={t("globalSearch.ariaLabel")}
        onChange={(event) => {
          setQuery(event.target.value);
          setIsOpen(true);
        }}
        onFocus={() => setIsOpen(true)}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            setIsOpen(false);
          }
        }}
      />
      {query !== "" ? (
        <button
          type="button"
          className="global-search-clear field-clear-button"
          aria-label={t("globalSearch.clear")}
          onClick={clearQuery}
        >
          <X size={14} aria-hidden="true" />
        </button>
      ) : null}
      {showPanel ? (
        <div className="global-search-results" role="listbox" aria-label={t("globalSearch.ariaLabel")}>
          {isSearching ? (
            <div className="global-search-status">{t("globalSearch.searching")}</div>
          ) : orderedGroups.length === 0 ? (
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
                    role="option"
                    aria-selected={false}
                    className="global-search-result"
                    key={`${match.contentType}:${match.sourceId}`}
                    onClick={() => selectMatch(match)}
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
