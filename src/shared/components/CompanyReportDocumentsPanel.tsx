import { useEffect, useMemo, useState } from "react";
import { ExternalLink, FileDown, FileText, Star } from "lucide-react";
import { getReportDocumentsView, reclassifyReportDocuments } from "../../api/reportDocuments";
import { extractReportDocumentData } from "../../api/fundamentalsExtraction";
import type {
  ReportDocument,
  ReportDocumentCoverageTotals,
  ReportDocumentViewRow,
} from "../../api/reportDocumentsTypes";
import { useLocale } from "../locale";
import { pluralNoun, type PluralForms } from "../locale/plural";
import { formatDetailTimestamp } from "../format/datetime";
import {
  ActionRow,
  Button,
  Checkbox,
  EmptyState,
  ErrorText,
  FilterToolbar,
  SearchField,
  SectionHeader,
  SelectField,
  StatusChip,
  useToast,
} from "../../ui";

// The doc_kind taxonomy (ADR 0077 §1) in display order. `null` (unclassified)
// is not a taxonomy value — it is a filter option and renders no badge.
const DOC_KINDS = [
  "periodic_ssf",
  "periodic_jsf",
  "auditor_opinion",
  "presentation",
  "governance",
  "other",
] as const;

// Filter sentinels kept distinct from taxonomy values.
const KIND_FILTER_ALL = "all";
const KIND_FILTER_UNCLASSIFIED = "unclassified";

// Plural forms for the grouped view's counts (pluralNoun handles Polish's three
// forms; a `n === 1 ? a : b` ternary is wrong there).
const DOCUMENT_FORMS: PluralForms = {
  en: ["document", "documents"],
  pl: ["dokument", "dokumenty", "dokumentów"],
};
const COMPANION_FORMS: PluralForms = {
  en: ["companion file", "companion files"],
  pl: ["plik towarzyszący", "pliki towarzyszące", "plików towarzyszących"],
};

// Plural forms for the coverage-totals summary row's periodic-report chip (#174,
// epic #229 T3) — `periodicCount` counts periodic filings in ANY fetch state.
const PERIODIC_FORMS: PluralForms = {
  en: ["periodic report", "periodic reports"],
  pl: ["raport okresowy", "raporty okresowe", "raportów okresowych"],
};

// Descending period index within a fiscal year (ADR 0077): FY/Q4/H2 newest.
function periodSortIndex(periodType: string): number {
  switch (periodType) {
    case "Q1":
      return 1;
    case "Q2":
    case "H1":
      return 2;
    case "Q3":
    case "9M":
      return 3;
    case "Q4":
    case "H2":
    case "FY":
      return 4;
    default:
      return 0;
  }
}

type CompanyReportDocumentsPanelProps = {
  companyId: string;
  // Bump to force a reload (e.g. after a backfill completes).
  reloadKey?: number;
  // Fired after an extraction that actually produced new facts, so a sibling
  // panel (the cockpit Fundamentals view) can refetch — the report-documents
  // panel and the fundamentals panel are independent cockpit panels with no
  // shared read model, so this is the invalidation signal across them.
  onExtracted?: () => void;
};

// Middle-ellipsize a long, unbreakable filename so the identity portion — the
// head prefix and the extension/distinguishing tail — both stay visible (a CSS
// end-ellipsis would hide the extension). The full name lives in the row's title
// attribute (tooltip). Density contract, ADR 0076 D6 (Report documents).
export function middleTruncate(name: string, max = 44): string {
  const trimmed = name.trim();
  if (trimmed.length <= max) return trimmed;
  const ellipsis = "…";
  const budget = max - ellipsis.length;
  const head = Math.ceil(budget / 2);
  const tail = Math.floor(budget / 2);
  return `${trimmed.slice(0, head)}${ellipsis}${trimmed.slice(trimmed.length - tail)}`;
}

// One period's rows, split into the always-visible set (periodic statements,
// audit reports, and any extract-eligible row) and the folded companion set.
type DocumentGroup = {
  key: string;
  fiscalYear: number;
  periodType: string;
  total: number;
  visible: ReportDocumentViewRow[];
  folded: ReportDocumentViewRow[];
};

/// Lists the report documents stored for a company (ADR 0036, ADR 0077 §1/§2): ESPI/EBI
/// attachments and user-supplied report URLs, each linked back to its source with durable
/// attribution. The redesigned panel (owner feedback 2026-07-09, mockup docs/mockups/f2-coverage-map.html
/// Panel B) groups documents by fiscal period, stars the canonical report of each period (the T1.3
/// selection), promotes the document-kind label ahead of the raw filename, and folds signature/data
/// companions away — with a "No period" group for non-periodic filings and a "Group by period" toggle
/// back to the flat list.
export function CompanyReportDocumentsPanel({
  companyId,
  reloadKey = 0,
  onExtracted,
}: CompanyReportDocumentsPanelProps) {
  const { text, locale } = useLocale();
  const toast = useToast();
  const [rows, setRows] = useState<ReportDocumentViewRow[]>([]);
  // Coverage roll-up (#174, epic #229 T3) — the denominator behind the rows;
  // null while the view hasn't loaded yet.
  const [totals, setTotals] = useState<ReportDocumentCoverageTotals | null>(null);
  const [error, setError] = useState<string | null>(null);
  // The report document currently being extracted (its row's action is busy).
  const [extractingId, setExtractingId] = useState<string | null>(null);
  // Selected document-type filter (a taxonomy value, "all", or "unclassified").
  const [kindFilter, setKindFilter] = useState<string>(KIND_FILTER_ALL);
  // Free-text search across the kind label / title / filename.
  const [query, setQuery] = useState("");
  // Grouped-by-period is the default; unchecking falls back to the flat list.
  const [grouped, setGrouped] = useState(true);
  // Group keys whose companion fold is expanded.
  const [expandedFolds, setExpandedFolds] = useState<Set<string>>(new Set());
  // The collapsed-by-default "No period" group.
  const [noPeriodExpanded, setNoPeriodExpanded] = useState(false);
  // Corpus-wide reclassification in flight (the header action is busy).
  const [reclassifying, setReclassifying] = useState(false);
  // Local reload trigger: bumped after a reclassification so the list refetches
  // the refreshed kinds without a prop change from the parent.
  const [reloadCounter, setReloadCounter] = useState(0);

  useEffect(() => {
    let cancelled = false;
    getReportDocumentsView(companyId)
      .then((view) => {
        if (!cancelled) {
          setRows(view.rows);
          setTotals(view.totals);
        }
      })
      .catch((reason) => {
        if (!cancelled) setError(reason instanceof Error ? reason.message : String(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [companyId, reloadKey, reloadCounter]);

  // Product-facing kind label for a doc_kind value (ADR 0077 §1). Returns null
  // for an unclassified (null) or unknown kind — no badge is rendered then.
  const kindLabel = (docKind: string | null): string | null => {
    switch (docKind) {
      case "periodic_ssf":
        return text("Consolidated report");
      case "periodic_jsf":
        return text("Standalone report");
      case "auditor_opinion":
        return text("Audit report");
      case "presentation":
        return text("Presentation");
      case "governance":
        return text("Governance");
      case "other":
        return text("Other");
      default:
        return null;
    }
  };

  // Short statement code for the accent chip on periodic rows (acronyms, shared
  // across locales — like ESPI/EBI). Non-periodic kinds carry no code chip; the
  // prominent kind label already names them.
  const statementCode = (docKind: string | null): string | null =>
    docKind === "periodic_ssf" ? "SSF" : docKind === "periodic_jsf" ? "JSF" : null;

  // Structured ESEF/iXBRL document → the neutral "ESEF" chip. Heuristic over the
  // content type and the filename/URL (xhtml body, xbri package).
  const isEsef = (document: ReportDocument): boolean => {
    const contentType = (document.contentType ?? "").toLowerCase();
    if (contentType.includes("xhtml") || contentType.includes("xml")) return true;
    const probe = `${document.title ?? ""} ${document.url} ${document.localPath ?? ""}`.toLowerCase();
    return probe.includes(".xhtml") || probe.includes(".xbri") || probe.includes(".xbrl");
  };

  // The cadence word for a period group's header (mockup "raport roczny", etc.).
  const cadenceWord = (periodType: string): string => {
    switch (periodType) {
      case "FY":
      case "Q4":
        return text("Annual report");
      case "H1":
      case "H2":
        return text("Half-year report");
      case "Q1":
      case "Q2":
      case "Q3":
      case "9M":
        return text("Quarterly report");
      default:
        return text("Periodic report");
    }
  };

  const statusLabel = (status: string) => {
    switch (status) {
      case "fetched":
        return text("Stored");
      case "metadata_only":
        return text("Link only");
      case "failed":
        return text("Failed fetch");
      default:
        return text("Pending fetch");
    }
  };

  // Extract-eligible = a stored (fetched) document — the same precondition the
  // autopilot pipeline requires (a local file to parse). Such a row is NEVER
  // folded away, so its actionable button can't hide inside a companion fold.
  const isExtractEligible = (document: ReportDocument) => document.fetchStatus === "fetched";

  const documentName = (row: ReportDocumentViewRow) =>
    row.document.title?.trim() || row.document.url;

  // Rows after the type filter + the free-text search (title / filename / kind).
  const filteredRows = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return rows.filter((row) => {
      const document = row.document;
      if (kindFilter === KIND_FILTER_UNCLASSIFIED) {
        if (document.docKind != null) return false;
      } else if (kindFilter !== KIND_FILTER_ALL && document.docKind !== kindFilter) {
        return false;
      }
      if (needle.length === 0) return true;
      const haystack = `${document.title ?? ""} ${document.url} ${kindLabel(document.docKind) ?? ""}`.toLowerCase();
      return haystack.includes(needle);
    });
    // kindLabel closes over `text`, which is stable per render; rows/kindFilter/query drive it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rows, kindFilter, query]);

  // Group the filtered rows by (fiscalYear, periodType) DESC. Within a group:
  // periodic statements (canonical first) → audit reports → the rest. Everything
  // in the "rest" folds away EXCEPT extract-eligible rows (never hide an action).
  const { groups, noPeriodRows } = useMemo(() => {
    const periodRows = filteredRows.filter(
      (row) => row.fiscalYear != null && row.periodType != null,
    );
    const noPeriod = filteredRows.filter(
      (row) => row.fiscalYear == null || row.periodType == null,
    );
    const byKey = new Map<string, ReportDocumentViewRow[]>();
    for (const row of periodRows) {
      const key = `${row.fiscalYear}|${row.periodType}`;
      const bucket = byKey.get(key);
      if (bucket) bucket.push(row);
      else byKey.set(key, [row]);
    }
    const rank = (row: ReportDocumentViewRow): number => {
      const docKind = row.document.docKind;
      if (docKind === "periodic_ssf" || docKind === "periodic_jsf") return 0;
      if (docKind === "auditor_opinion") return 1;
      return 2;
    };
    const built: DocumentGroup[] = [...byKey.entries()].map(([key, bucket]) => {
      const [fyText, periodType] = key.split("|");
      const sorted = [...bucket].sort((a, b) => {
        const ra = rank(a);
        const rb = rank(b);
        if (ra !== rb) return ra - rb;
        if (a.canonical !== b.canonical) return a.canonical ? -1 : 1;
        return documentName(a).localeCompare(documentName(b));
      });
      const folded = sorted.filter(
        (row) => rank(row) === 2 && !isExtractEligible(row.document),
      );
      const foldedSet = new Set(folded);
      const visible = sorted.filter((row) => !foldedSet.has(row));
      return {
        key,
        fiscalYear: Number(fyText),
        periodType,
        total: sorted.length,
        visible,
        folded,
      };
    });
    built.sort(
      (a, b) =>
        b.fiscalYear - a.fiscalYear ||
        periodSortIndex(b.periodType) - periodSortIndex(a.periodType) ||
        b.periodType.localeCompare(a.periodType),
    );
    return { groups: built, noPeriodRows: noPeriod };
  }, [filteredRows]);

  const toggleFold = (key: string) =>
    setExpandedFolds((current) => {
      const next = new Set(current);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });

  // Recompute the doc_kind taxonomy over every stored document (ADR 0077 §1).
  // Deterministic + idempotent server-side; on success the list reloads so the
  // refreshed kinds show, and the toast reports how many rows changed.
  const handleReclassify = () => {
    setReclassifying(true);
    reclassifyReportDocuments()
      .then((summary) => {
        toast.show({
          message: text("Classified documents: {n} of {total}")
            .replace("{n}", String(summary.updated))
            .replace("{total}", String(summary.total)),
          tone: "positive",
        });
        setReloadCounter((count) => count + 1);
        // Reclassification changes each document's doc_kind, which the coverage
        // read model consumes (canonical-report selection + kind labels). Signal
        // the sibling coverage/fundamentals panels to refetch so they don't show
        // stale kinds until the next remount (bug 3579234, ADR 0077 §1).
        onExtracted?.();
      })
      .catch((reason) => {
        toast.show({
          message: reason instanceof Error ? reason.message : String(reason),
          tone: "negative",
        });
      })
      .finally(() => {
        setReclassifying(false);
      });
  };

  // Run the deterministic structured pipeline over one stored report document
  // (ADR 0061 S5) — the reachable entry point that avoids the Inbox round-trip.
  // The period is derived server-side; feedback lands on the Toast surface.
  const handleExtract = (document: ReportDocument) => {
    setExtractingId(document.id);
    extractReportDocumentData({ companyId, reportDocumentId: document.id })
      .then((summary) => {
        // Report reality, not a blanket "done": the count of facts the run
        // actually produced. 0-new is NOT a success — a re-extracted period
        // re-observes its slots (skipped, T7-F), a flagged set or an
        // unparseable file emit nothing, and a re-observed value that DIFFERS
        // from the stored fact is kept out (never overwritten) and surfaced.
        const produced = summary.producedFactIds.length;
        const skipped = summary.skippedFactIds.length;
        if (produced > 0) {
          toast.show({
            message: text("Extracted new values: {n}").replace("{n}", String(produced)),
            tone: "positive",
          });
          // New facts landed → let the sibling fundamentals panel refetch.
          onExtracted?.();
        } else if (summary.divergentCount > 0) {
          toast.show({
            message: text(
              "Extracted values differ from stored facts: {n} — stored values kept, see Diagnostics",
            ).replace("{n}", String(summary.divergentCount)),
            tone: "caution",
          });
        } else if (skipped > 0) {
          toast.show({
            message: text("No new values — {n} already recorded from this document").replace(
              "{n}",
              String(skipped),
            ),
            tone: "caution",
          });
        } else {
          toast.show({
            message:
              summary.acceptance === "flagged"
                ? text("No new values — the document was flagged for review")
                : text("No new values extracted from this document"),
            tone: "caution",
          });
        }
      })
      .catch((reason) => {
        toast.show({
          message: reason instanceof Error ? reason.message : String(reason),
          tone: "negative",
        });
      })
      .finally(() => {
        setExtractingId((current) => (current === document.id ? null : current));
      });
  };

  const renderRow = (row: ReportDocumentViewRow) => {
    const document = row.document;
    const fullName = documentName(row);
    const dateLabel = formatDetailTimestamp(document.fetchedAt ?? document.createdAt, "");
    const code = statementCode(document.docKind);
    const canonicalTitle = text("Canonical report for this period");
    return (
      <li className="doc-row" key={document.id}>
        <span className="doc-row-icon">
          <FileText size={14} aria-hidden="true" />
        </span>
        <div className="doc-row-main">
          <div className="doc-row-head">
            {row.canonical ? (
              <span className="doc-row-star" role="img" aria-label={canonicalTitle} title={canonicalTitle}>
                <Star size={13} aria-hidden="true" />
              </span>
            ) : null}
            <span className="doc-row-kind">{kindLabel(document.docKind) ?? text("Other")}</span>
            <span className="doc-chips">
              {code ? <StatusChip tone="accent">{code}</StatusChip> : null}
              {isEsef(document) ? <StatusChip tone="neutral">ESEF</StatusChip> : null}
              {row.extraction ? (
                <StatusChip
                  className="doc-extraction-chip"
                  tone={
                    row.extraction.status === "has_data"
                      ? "ok"
                      : row.extraction.status === "flagged"
                        ? "warn"
                        : "neutral"
                  }
                  title={
                    row.extraction.status === "has_data"
                      ? `${text("Extracted facts")}: ${row.extraction.factCount}`
                      : undefined
                  }
                >
                  {row.extraction.status === "has_data"
                    ? text("Financial data")
                    : row.extraction.status === "flagged"
                      ? text("Extraction flagged")
                      : text("No extractable data")}
                </StatusChip>
              ) : null}
            </span>
          </div>
          <a
            className="doc-row-file"
            href={document.url}
            rel="noreferrer"
            target="_blank"
            title={fullName}
          >
            <span className="doc-row-name">{middleTruncate(fullName)}</span>
            {dateLabel ? <span className="doc-date">{dateLabel}</span> : null}
            <ExternalLink size={11} aria-hidden="true" />
          </a>
        </div>
        {/* Fixed trailing slots (owner rule 2026-07-09): the status-chip column
            and the action column are ALWAYS reserved, so a chip never migrates
            horizontally when a row happens to carry no action button. */}
        <div className="doc-side">
          <span className="doc-status">
            <StatusChip tone={document.fetchStatus === "fetched" ? "ok" : "neutral"}>
              {statusLabel(document.fetchStatus)}
            </StatusChip>
          </span>
          <span className="doc-action">
            {isExtractEligible(document) ? (
              <Button
                variant="secondary"
                className="doc-extract"
                icon={<FileDown size={14} aria-hidden="true" />}
                aria-label={text("Extract data")}
                title={text("Extract data")}
                disabled={extractingId === document.id}
                onClick={() => handleExtract(document)}
              >
                <span className="doc-extract-label">{text("Extract data")}</span>
              </Button>
            ) : null}
          </span>
        </div>
      </li>
    );
  };

  const hasDocuments = rows.length > 0;

  return (
    <div role="group" className="company-report-documents" aria-label={text("Report documents")}>
      <SectionHeader
        paneLead
        title={text("Report documents")}
        meta={filteredRows.length}
        actions={
          hasDocuments ? (
            <Button variant="secondary" disabled={reclassifying} onClick={handleReclassify}>
              {text("Refresh classification")}
            </Button>
          ) : null
        }
      />
      {error ? <ErrorText>{error}</ErrorText> : null}
      {/* Coverage-totals summary (#174, epic #229 T3): the denominator behind the
          rows below — documents/fetched/pending/link-only counts plus whether
          any FETCHED periodic report exists at all. `periodicCount` can be > 0
          while `hasPeriodicCoverage` is false (a known periodic filing with no
          stored bytes yet) — the single most important gap the roll-up exists
          to surface, so it gets its own warn-tone chip rather than folding into
          the periodic count. Reuses the Quality panel's scorecard-summary
          pattern (ActionRow + tone-carrying StatusChips). */}
      {totals && hasDocuments ? (
        <ActionRow ariaLabel={text("Document totals")} className="doc-coverage-summary">
          <StatusChip tone="neutral">
            {`${totals.documents} ${pluralNoun(locale, totals.documents, DOCUMENT_FORMS)}`}
          </StatusChip>
          <StatusChip tone="ok">{`${totals.fetched} ${text("Stored")}`}</StatusChip>
          <StatusChip tone="neutral">{`${totals.pending} ${text("Pending fetch")}`}</StatusChip>
          <StatusChip tone="neutral">{`${totals.metadataOnly} ${text("Link only")}`}</StatusChip>
          <StatusChip tone="accent">
            {`${totals.periodicCount} ${pluralNoun(locale, totals.periodicCount, PERIODIC_FORMS)}`}
          </StatusChip>
          {!totals.hasPeriodicCoverage ? (
            <StatusChip tone="warn">{text("No periodic report stored")}</StatusChip>
          ) : null}
        </ActionRow>
      ) : null}
      {hasDocuments ? (
        <FilterToolbar
          ariaLabel={text("Filter documents by type")}
          search={
            <SearchField
              ariaLabel={text("Search documents")}
              placeholder={text("Search titles…")}
              value={query}
              onChange={setQuery}
              onClear={() => setQuery("")}
              clearLabel={text("Clear search")}
            />
          }
        >
          <SelectField
            label={text("Document type")}
            value={kindFilter}
            onChange={(event) => setKindFilter(event.target.value)}
          >
            <option value={KIND_FILTER_ALL}>{text("All types")}</option>
            {DOC_KINDS.map((docKind) => (
              <option key={docKind} value={docKind}>
                {kindLabel(docKind)}
              </option>
            ))}
            <option value={KIND_FILTER_UNCLASSIFIED}>{text("Unclassified")}</option>
          </SelectField>
          <Checkbox
            className="doc-group-toggle"
            checked={grouped}
            onChange={(event) => setGrouped(event.target.checked)}
            label={text("Group by period")}
          />
        </FilterToolbar>
      ) : null}
      {!hasDocuments && !error ? (
        <EmptyState>{text("No report documents stored yet.")}</EmptyState>
      ) : grouped ? (
        <div className="doc-groups">
          {groups.map((group) => {
            const expanded = expandedFolds.has(group.key);
            return (
              <div className="doc-group" key={group.key}>
                <div className="doc-grp">
                  <span className="doc-grp-label">
                    {group.fiscalYear} {group.periodType}
                  </span>
                  <span className="doc-grp-meta">
                    {cadenceWord(group.periodType)} · {group.total}{" "}
                    {pluralNoun(locale, group.total, DOCUMENT_FORMS)}
                  </span>
                </div>
                <ul className="doc-rows">
                  {group.visible.map(renderRow)}
                  {expanded ? group.folded.map(renderRow) : null}
                </ul>
                {group.folded.length > 0 ? (
                  <button
                    type="button"
                    className="doc-fold"
                    aria-expanded={expanded}
                    onClick={() => toggleFold(group.key)}
                  >
                    {expanded ? "▾" : "▸"} {group.folded.length}{" "}
                    {pluralNoun(locale, group.folded.length, COMPANION_FORMS)}
                  </button>
                ) : null}
              </div>
            );
          })}
          {noPeriodRows.length > 0 ? (
            <div className="doc-group" key="__no_period__">
              <div className="doc-grp">
                <span className="doc-grp-label">{text("No period")}</span>
                <span className="doc-grp-meta">
                  {noPeriodRows.length} {pluralNoun(locale, noPeriodRows.length, DOCUMENT_FORMS)}
                </span>
              </div>
              {noPeriodExpanded ? <ul className="doc-rows">{noPeriodRows.map(renderRow)}</ul> : null}
              <button
                type="button"
                className="doc-fold"
                aria-expanded={noPeriodExpanded}
                onClick={() => setNoPeriodExpanded((value) => !value)}
              >
                {noPeriodExpanded ? "▾" : "▸"}{" "}
                {text("Show all ({n})").replace("{n}", String(noPeriodRows.length))}
              </button>
            </div>
          ) : null}
        </div>
      ) : (
        <ul className="doc-rows doc-rows-flat">{filteredRows.map(renderRow)}</ul>
      )}
    </div>
  );
}
