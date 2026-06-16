import { CheckCircle2, FileSearch, Link2, RefreshCw, Table2, XCircle } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { listCompanies } from "../../api/companies";
import {
  confirmKpiProposal,
  listKpiExtraction,
  rejectKpiProposal,
  startKpiExtraction,
  type KpiExtractionJob,
  type KpiExtractionProposal,
} from "../../api/kpiExtraction";
import { resolveIrReport, type IrReportCandidate } from "../../api/ir";
import { captureReportDocument, listReportDocuments } from "../../api/reportDocuments";
import type { ReportDocument } from "../../api/reportDocumentsTypes";
import type { FeedItem } from "../../api/types";
import { Button } from "./Button";
import { StatusPill } from "./StatusPill";
import { Checkbox, DetailSection, Modal, TextField } from "../../ui";
import { useLocale } from "../locale";
import { formatFinancialValue } from "../format/financialValue";

export type FeedKpiExtractionPanelProps = {
  feedItem: FeedItem;
  providerConfigured: boolean;
};

function fileNameFromUrl(url: string): string {
  try {
    const path = new URL(url).pathname;
    const name = path.split("/").filter(Boolean).pop();
    return name ? decodeURIComponent(name) : url;
  } catch {
    return url;
  }
}

function jobStatusTone(status: string) {
  if (status === "succeeded") return "ok";
  if (status === "failed") return "danger";
  if (status === "queued" || status === "running") return "warn";
  return "neutral";
}

function isActive(status: string) {
  return status === "queued" || status === "running";
}

export function FeedKpiExtractionPanel({ feedItem, providerConfigured }: FeedKpiExtractionPanelProps) {
  const { text, locale } = useLocale();
  const [companyId, setCompanyId] = useState<string | null>(null);
  const [documents, setDocuments] = useState<ReportDocument[]>([]);
  const [job, setJob] = useState<KpiExtractionJob | null>(null);
  const [candidates, setCandidates] = useState<IrReportCandidate[] | null>(null);
  const [pasteUrl, setPasteUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [edits, setEdits] = useState<Record<string, string>>({});
  const [acceptNew, setAcceptNew] = useState<Record<string, boolean>>({});
  const [fiscalYear, setFiscalYear] = useState("");
  const [periodType, setPeriodType] = useState("");
  const [flowOpen, setFlowOpen] = useState(false);

  const pdfAttachments = useMemo(
    () => feedItem.attachments.filter((attachment) => /\.pdf($|\?)/i.test(attachment.url)),
    [feedItem.attachments]
  );

  // Resolve the tracked company for this feed item (feed items carry a qualified ticker).
  useEffect(() => {
    let cancelled = false;
    void listCompanies()
      .then((companies) => {
        if (cancelled) return;
        const match = companies.find((company) => company.qualifiedTicker === feedItem.company);
        setCompanyId(match?.id ?? null);
      })
      .catch(() => {
        if (!cancelled) setCompanyId(null);
      });
    return () => {
      cancelled = true;
    };
  }, [feedItem.company]);

  const refreshDocuments = useCallback(async () => {
    if (!companyId) return;
    const docs = await listReportDocuments(companyId);
    setDocuments(docs);
  }, [companyId]);

  useEffect(() => {
    void refreshDocuments();
  }, [refreshDocuments]);

  // Keep period overrides in sync with the detected period.
  useEffect(() => {
    if (job?.detectedFiscalYear != null) setFiscalYear(String(job.detectedFiscalYear));
    if (job?.detectedPeriodType) setPeriodType(job.detectedPeriodType);
  }, [job?.detectedFiscalYear, job?.detectedPeriodType]);

  // Poll while an extraction job is in flight.
  useEffect(() => {
    if (!job || !isActive(job.status)) return;
    const documentId = job.reportDocumentId;
    const timer = setTimeout(() => {
      void listKpiExtraction(documentId)
        .then((jobs) => {
          const next = jobs.find((candidate) => candidate.id === job.id) ?? jobs[0] ?? null;
          if (next) setJob(next);
        })
        .catch((cause) => setError(String(cause)));
    }, 1500);
    return () => clearTimeout(timer);
  }, [job]);

  const guard = useCallback(
    async (action: () => Promise<void>) => {
      setBusy(true);
      setError(null);
      try {
        await action();
      } catch (cause) {
        setError(String(cause));
      } finally {
        setBusy(false);
      }
    },
    []
  );

  const extractDocument = useCallback(
    (reportDocumentId: string) =>
      guard(async () => {
        setCandidates(null);
        const started = await startKpiExtraction({
          reportDocumentId,
          periodHint: feedItem.title,
        });
        setJob(started);
      }),
    [feedItem.title, guard]
  );

  const captureAndExtract = useCallback(
    (url: string, sourceType: string) =>
      guard(async () => {
        if (!companyId) return;
        const result = await captureReportDocument({
          companyId,
          sourceType,
          url,
          originRef: feedItem.id,
          title: feedItem.title,
        });
        await refreshDocuments();
        if (result.success) {
          await extractDocument(result.documentId);
        } else {
          setError(result.error ?? text("Failed to fetch the report document."));
        }
      }),
    [companyId, extractDocument, feedItem.id, feedItem.title, guard, refreshDocuments, text]
  );

  const resolveFromIr = useCallback(
    () =>
      guard(async () => {
        if (!companyId) return;
        const resolution = await resolveIrReport({
          companyId,
          periodHint: feedItem.title,
          reportType: feedItem.type,
          publishedAt: feedItem.publishedAt,
        });
        await refreshDocuments();
        if (resolution.document) {
          await extractDocument(resolution.document.id);
        } else {
          setCandidates(resolution.candidates);
        }
      }),
    [companyId, extractDocument, feedItem.publishedAt, feedItem.title, feedItem.type, guard, refreshDocuments]
  );

  const refreshJob = useCallback(async () => {
    if (!job) return;
    const jobs = await listKpiExtraction(job.reportDocumentId);
    const next = jobs.find((candidate) => candidate.id === job.id) ?? jobs[0] ?? null;
    if (next) setJob(next);
  }, [job]);

  const confirm = useCallback(
    (proposal: KpiExtractionProposal) =>
      guard(async () => {
        const editedValue = edits[proposal.id];
        await confirmKpiProposal({
          proposalId: proposal.id,
          valueNumeric: editedValue && editedValue !== proposal.valueNumeric ? editedValue : undefined,
          fiscalYear: fiscalYear ? Number(fiscalYear) : undefined,
          periodType: periodType || undefined,
          acceptAsNewKpi: proposal.isProposedKpi ? acceptNew[proposal.id] ?? false : false,
        });
        await refreshJob();
      }),
    [acceptNew, edits, fiscalYear, guard, periodType, refreshJob]
  );

  const reject = useCallback(
    (proposal: KpiExtractionProposal) =>
      guard(async () => {
        await rejectKpiProposal(proposal.id);
        await refreshJob();
      }),
    [guard, refreshJob]
  );

  // Bulk confirm a set of pending proposals, collecting per-metric failures so a
  // single unresolved metric key does not abort the whole batch or close the modal.
  const confirmMany = useCallback(
    (selector: (proposal: KpiExtractionProposal) => boolean, acceptAsNewKpi: boolean) =>
      guard(async () => {
        const targets = (job?.proposals ?? []).filter(
          (proposal) => proposal.status === "pending" && selector(proposal)
        );
        const failures: string[] = [];
        for (const proposal of targets) {
          try {
            const editedValue = edits[proposal.id];
            await confirmKpiProposal({
              proposalId: proposal.id,
              valueNumeric:
                editedValue && editedValue !== proposal.valueNumeric ? editedValue : undefined,
              fiscalYear: fiscalYear ? Number(fiscalYear) : undefined,
              periodType: periodType || undefined,
              acceptAsNewKpi,
            });
          } catch (cause) {
            failures.push(`${proposal.label}: ${String(cause)}`);
          }
        }
        await refreshJob();
        if (failures.length > 0) {
          setError(failures.join("; "));
        }
      }),
    [edits, fiscalYear, guard, job?.proposals, periodType, refreshJob]
  );

  const confirmAllKnown = useCallback(
    () => confirmMany((proposal) => !proposal.isProposedKpi, false),
    [confirmMany]
  );

  const acceptAllSuggestions = useCallback(
    () => confirmMany((proposal) => proposal.isProposedKpi, true),
    [confirmMany]
  );

  const fetchedDocuments = documents.filter((document) => document.fetchStatus === "fetched");
  const proposals = job?.proposals ?? [];
  const hasPendingProposals = proposals.some((proposal) => proposal.status === "pending");
  // Split pending proposals by kind so each bulk action is only enabled when it
  // has something to do — otherwise "Accept all suggestions" appears to blink
  // (busy toggles, refresh runs) with no visible effect when every proposal is a
  // known taxonomy KPI and nothing was suggested as new.
  const hasPendingKnown = proposals.some((p) => p.status === "pending" && !p.isProposedKpi);
  const hasPendingSuggested = proposals.some((p) => p.status === "pending" && p.isProposedKpi);
  const canExtract = providerConfigured && companyId != null;

  // Surface the modal automatically once a job finishes with proposals to handle,
  // so the user lands directly on the review without a second click.
  useEffect(() => {
    if (job?.status === "succeeded" && hasPendingProposals) setFlowOpen(true);
  }, [job?.status, hasPendingProposals]);

  return (
    <DetailSection
      ariaLabel={text("AI KPI extraction")}
      aside={job ? <StatusPill tone={jobStatusTone(job.status)}>{text(job.status)}</StatusPill> : null}
      className="kpi-extraction-launcher"
      description={text("Extract reported KPIs from this report; confirm each value before it is saved.")}
      icon={<Table2 size={15} />}
      title={text("AI KPI extraction")}
    >
      {!providerConfigured ? (
        <p className="ai-analysis-empty">
          {text("Configure a general AI provider in Settings before running analysis.")}
        </p>
      ) : companyId == null ? (
        <p className="ai-analysis-empty">{text("Track this company to extract KPIs from its reports.")}</p>
      ) : (
        <div className="detail-launcher">
          <span className="detail-launcher-status">
            {job?.status === "succeeded"
              ? `${proposals.length} ${text("KPI values extracted")}${
                  hasPendingProposals
                    ? ` · ${proposals.filter((p) => p.status === "pending").length} ${text("to review")}`
                    : ""
                }`
              : text("Open the extractor to pick a report source and review proposed KPIs.")}
          </span>
          <Button
            className="compact-button"
            disabled={busy}
            onClick={() => setFlowOpen(true)}
            variant="primary"
          >
            <Table2 size={15} />
            {job?.status === "succeeded" ? text("Review extracted KPIs") : text("Extract KPIs")}
          </Button>
        </div>
      )}

      {error && !flowOpen ? (
        <p className="error-text">{text("KPI extraction failed.")} {error}</p>
      ) : null}

      <Modal
        ariaLabel={text("AI KPI extraction")}
        onClose={() => setFlowOpen(false)}
        open={flowOpen}
        title={text("AI KPI extraction")}
        footer={
          hasPendingProposals ? (
            <>
              <Button
                className="compact-button"
                disabled={busy || !hasPendingKnown}
                onClick={() => void confirmAllKnown()}
                variant="primary"
              >
                <CheckCircle2 size={15} />
                {text("Confirm all known")}
              </Button>
              <Button
                className="compact-button"
                disabled={busy || !hasPendingSuggested}
                onClick={() => void acceptAllSuggestions()}
              >
                {text("Accept all suggestions")}
              </Button>
              <Button className="compact-button" disabled={busy} onClick={() => void refreshJob()}>
                <RefreshCw size={15} />
                {text("Refresh")}
              </Button>
            </>
          ) : (
            <Button className="compact-button" onClick={() => setFlowOpen(false)}>
              {text("Close")}
            </Button>
          )
        }
      >
        <div className="kpi-extraction-review">
          {job ? (
            <div className="kpi-extraction-status" aria-label={text("Extraction status")}>
              <StatusPill tone={jobStatusTone(job.status)}>{text(job.status)}</StatusPill>
              {job.status === "succeeded" ? (
                <span>
                  {proposals.length} {text("KPI values extracted")}
                  {hasPendingProposals
                    ? ` · ${proposals.filter((p) => p.status === "pending").length} ${text("to review")}`
                    : ""}
                </span>
              ) : null}
            </div>
          ) : null}
          {!canExtract ? null : (
            <>
              <div className="kpi-extraction-sources" aria-label={text("Report document sources")}>
                {pdfAttachments.map((attachment) => {
                  const name = attachment.label || fileNameFromUrl(attachment.url);
                  return (
                    <Button
                      className="compact-button kpi-extraction-source"
                      disabled={busy}
                      key={attachment.id}
                      onClick={() => void captureAndExtract(attachment.url, "espi_attachment")}
                      title={name}
                    >
                      <FileSearch size={15} />
                      <span className="kpi-extraction-source-text">
                        <span className="kpi-extraction-source-label">{text("Extract from attachment")}</span>
                        <span className="kpi-extraction-source-name">{name}</span>
                      </span>
                    </Button>
                  );
                })}
                <Button
                  className="compact-button kpi-extraction-source kpi-extraction-source-action"
                  disabled={busy}
                  onClick={() => void resolveFromIr()}
                >
                  <Link2 size={15} />
                  <span className="kpi-extraction-source-text">{text("Fetch report from IR page")}</span>
                </Button>
              </div>

              <div className="kpi-extraction-paste">
                <TextField
                  aria-label={text("Report PDF URL")}
                  onChange={(event) => setPasteUrl(event.target.value)}
                  placeholder={text("Paste a report PDF URL")}
                  value={pasteUrl}
                />
                <Button
                  className="compact-button"
                  disabled={busy || !pasteUrl.trim()}
                  onClick={() => void captureAndExtract(pasteUrl.trim(), "user_url")}
                >
                  {text("Capture & extract")}
                </Button>
              </div>

              {fetchedDocuments.length > 0 ? (
                <ul className="kpi-extraction-documents" aria-label={text("Stored report documents")}>
                  {fetchedDocuments.map((document) => (
                    <li key={document.id}>
                      <span title={document.url}>
                        {document.title ?? fileNameFromUrl(document.url)}{" "}
                        <small>({fileNameFromUrl(document.url)})</small>
                      </span>
                      <Button className="compact-button" disabled={busy} onClick={() => void extractDocument(document.id)}>
                        {text("Extract KPIs")}
                      </Button>
                    </li>
                  ))}
                </ul>
              ) : null}

              {candidates ? (
                <div className="kpi-extraction-candidates" aria-label={text("IR page report candidates")}>
                  <p>{text("Pick the report on the IR page:")}</p>
                  {candidates.length === 0 ? (
                    <p className="ai-analysis-empty">{text("No report links found on the IR page.")}</p>
                  ) : (
                    <ul>
                      {candidates.map((candidate) => (
                        <li key={candidate.url}>
                          <span>{candidate.label || candidate.url}</span>
                          <Button
                            className="compact-button"
                            disabled={busy}
                            onClick={() => void captureAndExtract(candidate.url, "ir_page")}
                          >
                            {text("Use this")}
                          </Button>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              ) : null}
            </>
          )}

          {job?.status === "failed" ? (
            <div className="ai-analysis-error">
              <p>{job.error ?? text("KPI extraction failed.")}</p>
            </div>
          ) : null}

          <div className="kpi-extraction-period">
            <TextField
              aria-label={text("Fiscal year")}
              label={text("Fiscal year")}
              onChange={(event) => setFiscalYear(event.target.value)}
              value={fiscalYear}
            />
            <TextField
              aria-label={text("Period")}
              label={text("Period")}
              onChange={(event) => setPeriodType(event.target.value)}
              value={periodType}
            />
          </div>
          {proposals.length === 0 ? (
            <p className="ai-analysis-empty">{text("No KPI values were proposed.")}</p>
          ) : (
            <ul>
              {proposals.map((proposal) => (
                <li className="kpi-extraction-proposal" key={proposal.id}>
                  <div className="kpi-extraction-proposal-head">
                    <strong>{proposal.label}</strong>
                    <span className="kpi-extraction-proposal-tags">
                      {proposal.isProposedKpi ? (
                        <StatusPill tone="neutral">{text("Suggested KPI")}</StatusPill>
                      ) : null}
                      {proposal.confidence ? (
                        <StatusPill tone="neutral">{text(proposal.confidence)}</StatusPill>
                      ) : null}
                      <StatusPill tone={proposal.status === "confirmed" ? "ok" : proposal.status === "rejected" ? "danger" : "warn"}>
                        {text(proposal.status)}
                      </StatusPill>
                    </span>
                  </div>
                  <p className="kpi-extraction-asreported">
                    <span>{proposal.asReportedValue ? text("As reported") : text("Value")}</span>
                    <strong>
                      {formatFinancialValue(
                        {
                          valueNumeric: edits[proposal.id] ?? proposal.valueNumeric,
                          currency: proposal.currency,
                          asReportedValue: proposal.asReportedValue,
                          asReportedScale: proposal.asReportedScale,
                          unit: proposal.unit,
                        },
                        locale,
                      )}
                    </strong>
                  </p>
                  <TextField
                    aria-label={`${proposal.label} ${text("value")}`}
                    disabled={proposal.status !== "pending"}
                    onChange={(event) => setEdits((current) => ({ ...current, [proposal.id]: event.target.value }))}
                    value={edits[proposal.id] ?? proposal.valueNumeric}
                  />
                  {proposal.sourceSnippet ? (
                    <p className="kpi-extraction-snippet">“{proposal.sourceSnippet}”</p>
                  ) : null}
                  {proposal.status === "pending" ? (
                    <div className="kpi-extraction-proposal-actions">
                      {proposal.isProposedKpi ? (
                        <Checkbox
                          className="kpi-extraction-accept-new"
                          aria-label={`${proposal.label} ${text("track as new KPI")}`}
                          checked={acceptNew[proposal.id] ?? false}
                          onChange={(event) =>
                            setAcceptNew((current) => ({ ...current, [proposal.id]: event.target.checked }))
                          }
                          label={text("Track as new KPI")}
                        />
                      ) : null}
                      <Button
                        className="compact-button"
                        disabled={busy || (proposal.isProposedKpi && !(acceptNew[proposal.id] ?? false))}
                        onClick={() => void confirm(proposal)}
                        variant="primary"
                      >
                        <CheckCircle2 size={15} />
                        {text("Confirm")}
                      </Button>
                      <Button className="compact-button" disabled={busy} onClick={() => void reject(proposal)}>
                        <XCircle size={15} />
                        {text("Reject")}
                      </Button>
                    </div>
                  ) : null}
                </li>
              ))}
            </ul>
          )}
          {error ? <p className="error-text">{error}</p> : null}
        </div>
      </Modal>
    </DetailSection>
  );
}
