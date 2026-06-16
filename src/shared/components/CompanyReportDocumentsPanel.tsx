import { useEffect, useState } from "react";
import { ExternalLink, FileText } from "lucide-react";
import { listReportDocuments } from "../../api/reportDocuments";
import type { ReportDocument } from "../../api/reportDocumentsTypes";
import { useLocale } from "../locale";
import { EmptyState, ErrorText, ListRow, SectionHeader, StatusChip } from "../../ui";

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
      <SectionHeader title={text("Report documents")} meta={documents.length} />
      {error ? <ErrorText>{error}</ErrorText> : null}
      {documents.length === 0 && !error ? (
        <EmptyState>{text("No report documents stored yet.")}</EmptyState>
      ) : (
        <ul className="ui-list-rows">
          {documents.map((document) => {
            const fullName = document.title?.trim() || document.url;
            return (
              <ListRow
                key={document.id}
                icon={<FileText size={14} aria-hidden="true" />}
                href={document.url}
                title={fullName}
                titleAttr={fullName}
                adornment={<ExternalLink size={12} aria-hidden="true" />}
                meta={document.attribution ?? text("Unknown")}
                trailing={
                  <StatusChip tone={document.fetchStatus === "fetched" ? "accent" : "neutral"}>
                    {statusLabel(document.fetchStatus)}
                  </StatusChip>
                }
              />
            );
          })}
        </ul>
      )}
    </section>
  );
}
