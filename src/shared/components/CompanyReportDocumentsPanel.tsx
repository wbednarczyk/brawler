import { useEffect, useState } from "react";
import { ExternalLink, FileDown, FileText } from "lucide-react";
import { listReportDocuments } from "../../api/reportDocuments";
import { extractReportDocumentData } from "../../api/fundamentalsExtraction";
import type { ReportDocument } from "../../api/reportDocumentsTypes";
import { useLocale } from "../locale";
import { formatDetailTimestamp } from "../format/datetime";
import { Button, EmptyState, ErrorText, ListRow, SectionHeader, StatusChip, useToast } from "../../ui";

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

/// Lists the report documents stored for a company (ADR 0036): ESPI/EBI attachments and
/// user-supplied report URLs, each linked back to its source with durable attribution. Periodic
/// reports keep the full file ("Stored"); other filings keep the link only ("Link only").
///
/// Density (ADR 0076 D6): the row shows filename (middle-ellipsis) + date at every tier; the
/// type/status chips appear from M up, and the source-attribution preview column from L up. A
/// short pane drops both back to the bare list. Tiering is pure CSS (container queries on the
/// hosting `pane`); nothing folds behind a disclosure here.
export function CompanyReportDocumentsPanel({
  companyId,
  reloadKey = 0,
  onExtracted,
}: CompanyReportDocumentsPanelProps) {
  const { text } = useLocale();
  const toast = useToast();
  const [documents, setDocuments] = useState<ReportDocument[]>([]);
  const [error, setError] = useState<string | null>(null);
  // The report document currently being extracted (its row's action is busy).
  const [extractingId, setExtractingId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    listReportDocuments(companyId)
      .then((rows) => {
        if (!cancelled) setDocuments(rows);
      })
      .catch((reason) => {
        if (!cancelled) setError(reason instanceof Error ? reason.message : String(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [companyId, reloadKey]);

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
  // autopilot pipeline requires (a local file to parse). The server derives the
  // reporting period and rejects a document whose period can't be classified, so
  // a per-document "have data" indicator stays deferred (already carded).
  const isExtractEligible = (document: ReportDocument) => document.fetchStatus === "fetched";

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

  // Product-facing type label (avoid the raw implementation code, e.g. `ir_page`).
  const typeLabel = (sourceType: string) => {
    switch (sourceType) {
      case "espi":
        return "ESPI";
      case "espi_attachment":
        return "ESPI";
      case "ebi":
        return "EBI";
      case "ebi_attachment":
        return "EBI";
      case "ir_page":
        return text("IR page");
      case "user_url":
        return text("User link");
      default:
        return sourceType;
    }
  };

  // Provenance bundle (document kind + source attribution). The source stays
  // visible and durable, but off the row's chip line: the repeated type and
  // source chips were K-class noise (identical on every row). They live in the
  // L-tier preview column and its tooltip instead of being deleted (ADR 0076 D6).
  const provenanceLabel = (document: ReportDocument) =>
    `${typeLabel(document.sourceType)} · ${document.attribution?.trim() || text("Unknown")}`;

  return (
    <section className="company-report-documents" aria-label={text("Report documents")}>
      <SectionHeader paneLead title={text("Report documents")} meta={documents.length} />
      {error ? <ErrorText>{error}</ErrorText> : null}
      {documents.length === 0 && !error ? (
        <EmptyState>{text("No report documents stored yet.")}</EmptyState>
      ) : (
        <ul className="ui-list-rows">
          {documents.map((document) => {
            const fullName = document.title?.trim() || document.url;
            const dateLabel = formatDetailTimestamp(document.fetchedAt ?? document.createdAt, "");
            return (
              <ListRow
                key={document.id}
                icon={<FileText size={14} aria-hidden="true" />}
                href={document.url}
                title={middleTruncate(fullName)}
                titleAttr={fullName}
                adornment={<ExternalLink size={12} aria-hidden="true" />}
                trailing={
                  <span className="doc-trailing">
                    {dateLabel ? <span className="doc-date">{dateLabel}</span> : null}
                    <span className="doc-chips">
                      {/* Only the differentiating storage state — "Stored" vs
                          "Link only". Kind + source moved to the preview column. */}
                      <StatusChip tone={document.fetchStatus === "fetched" ? "ok" : "neutral"}>
                        {statusLabel(document.fetchStatus)}
                      </StatusChip>
                    </span>
                    <span className="doc-preview" title={provenanceLabel(document)}>
                      {provenanceLabel(document)}
                    </span>
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
                }
              />
            );
          })}
        </ul>
      )}
    </section>
  );
}
