import { useEffect, useState } from "react";
import { ExternalLink, FileText } from "lucide-react";
import { listReportDocuments } from "../../api/reportDocuments";
import type { ReportDocument } from "../../api/reportDocumentsTypes";
import { useLocale } from "../locale";
import { EmptyState, StatusChip } from "../../ui";

type CompanyReportDocumentsPanelProps = {
  companyId: string;
  // Bump to force a reload (e.g. after a backfill completes).
  reloadKey?: number;
};

/// Lists the report documents stored for a company (ADR 0036): ESPI/EBI attachments and
/// user-supplied report URLs, each linked back to its source with durable attribution. Periodic
/// reports keep the full file ("Stored"); other filings keep the link only ("Link only").
export function CompanyReportDocumentsPanel({
  companyId,
  reloadKey = 0,
}: CompanyReportDocumentsPanelProps) {
  const { text } = useLocale();
  const [documents, setDocuments] = useState<ReportDocument[]>([]);
  const [error, setError] = useState<string | null>(null);

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

  return (
    <section className="company-report-documents" aria-label={text("Report documents")}>
      <div className="company-report-documents-head">
        <h4>{text("Report documents")}</h4>
        <span>{documents.length}</span>
      </div>
      {error ? <p className="error-text">{error}</p> : null}
      {documents.length === 0 && !error ? (
        <EmptyState>{text("No report documents stored yet.")}</EmptyState>
      ) : (
        <ul className="company-report-documents-list">
          {documents.map((document) => {
            const fullName = document.title?.trim() || document.url;
            return (
              <li className="company-report-document-row" key={document.id}>
                <FileText size={14} aria-hidden="true" className="company-report-document-icon" />
                <a
                  className="company-report-document-link"
                  href={document.url}
                  rel="noreferrer"
                  target="_blank"
                  title={fullName}
                >
                  <span className="company-report-document-name">{fullName}</span>
                  <ExternalLink size={12} aria-hidden="true" />
                </a>
                <span className="company-report-document-attr">
                  {document.attribution ?? text("Unknown")}
                </span>
                <StatusChip tone={document.fetchStatus === "fetched" ? "accent" : "neutral"}>
                  {statusLabel(document.fetchStatus)}
                </StatusChip>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
