import { useCallback, useEffect, useState } from "react";
import { Pencil, X } from "lucide-react";

import { getCompanyBasicInfo, type CompanyBasicInfo } from "../../api/companyBasicInfo";
import {
  backfillOwnershipExtraction,
  confirmOwnershipHolderTypeProposal,
  confirmOwnershipOcrProposal,
  getOwnershipOverview,
  rejectOwnershipHolderTypeProposal,
  rejectOwnershipOcrProposal,
  runCompanyOwnershipOcr,
  runOwnershipClassification,
  setOwnershipHolderType,
  type OwnershipOverview,
} from "../../api/ownership";
import { getInsiderOverview, type InsiderOverview } from "../../api/insider";
import { formatFinancialValue } from "../format/financialValue";
import { useLocale } from "../locale";
import { Button, ErrorText, InfoGrid, SectionHeader, Skeleton, StatusChip, useToast } from "../../ui";
import { CompanyIrReportsUrlField } from "./CompanyIrReportsUrlField";
import { CompanySectorField } from "./CompanySectorField";
import { InsiderBlock } from "./InsiderBlock";
import { OwnershipSection } from "./OwnershipSection";
import { TickerLabel } from "./TickerLabel";

type CompanyBasicInfoPanelProps = {
  companyId: string;
};

const DASH = "—";

/// "Basic info" cockpit panel (owner request 2026-07-14, mockup
/// docs/mockups/basic-info-panel.html): identity facts (name, ticker, ISIN),
/// sector with provenance, latest recorded shares_outstanding — read-only by
/// default. Edit affordances (sector override, IR reports URL) stay hidden
/// behind ONE panel-level Edit toggle, never per-fact buttons. Below the
/// identity facts sits the Ownership section (v0.56 T6, ADR 0072): the panel
/// owns its IPC fetch + mutations and passes callbacks down.
export function CompanyBasicInfoPanel({ companyId }: CompanyBasicInfoPanelProps) {
  const { text, locale } = useLocale();
  const toast = useToast();
  const [info, setInfo] = useState<CompanyBasicInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  // Bump to re-fetch after the edit fields save (sector changes provenance).
  const [revision, setRevision] = useState(0);

  const [ownership, setOwnership] = useState<OwnershipOverview | null>(null);
  const [ownershipError, setOwnershipError] = useState<string | null>(null);
  const [ownershipBusy, setOwnershipBusy] = useState(false);

  const [insider, setInsider] = useState<InsiderOverview | null>(null);
  const [insiderError, setInsiderError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setError(null);
    getCompanyBasicInfo(companyId)
      .then((result) => {
        if (!cancelled) setInfo(result);
      })
      .catch((cause) => {
        if (!cancelled) setError(String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [companyId, revision]);

  useEffect(() => {
    let cancelled = false;
    setOwnershipError(null);
    getOwnershipOverview(companyId)
      .then((result) => {
        if (!cancelled) setOwnership(result);
      })
      .catch((cause) => {
        if (!cancelled) setOwnershipError(String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [companyId]);

  useEffect(() => {
    let cancelled = false;
    setInsiderError(null);
    getInsiderOverview(companyId)
      .then((result) => {
        if (!cancelled) setInsider(result);
      })
      .catch((cause) => {
        if (!cancelled) setInsiderError(String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [companyId]);

  useEffect(() => {
    setEditing(false);
    setInfo(null);
    setOwnership(null);
    setInsider(null);
  }, [companyId]);

  const handleBackfill = useCallback(() => {
    setOwnershipBusy(true);
    backfillOwnershipExtraction(companyId)
      .then((count) => {
        toast.show({
          message: `${text("Started reading ownership from")} ${count} ${text("reports")}`,
          tone: "positive",
        });
        return getOwnershipOverview(companyId);
      })
      .then(setOwnership)
      .catch((cause) => setOwnershipError(String(cause)))
      .finally(() => setOwnershipBusy(false));
  }, [companyId, text, toast]);

  const runMutation = useCallback((promise: Promise<OwnershipOverview>) => {
    setOwnershipBusy(true);
    promise
      .then(setOwnership)
      .catch((cause) => setOwnershipError(String(cause)))
      .finally(() => setOwnershipBusy(false));
  }, []);

  const handleSetHolderType = useCallback(
    (holderKey: string, holderType: string | null) =>
      runMutation(setOwnershipHolderType(companyId, holderKey, holderType)),
    [companyId, runMutation],
  );

  // ADR 0072 §3: the AI pass only ever produces proposals (confirm-before-apply);
  // this explicit trigger keeps AI spend user-initiated.
  const handleRunClassification = useCallback(() => {
    setOwnershipBusy(true);
    runOwnershipClassification()
      .then((result) => {
        toast.show({
          message: `${text("Classification proposals ready:")} ${result.proposed}`,
          tone: "positive",
        });
        return getOwnershipOverview(companyId);
      })
      .then(setOwnership)
      .catch((cause) => setOwnershipError(String(cause)))
      .finally(() => setOwnershipBusy(false));
  }, [companyId, text, toast]);
  const handleConfirmProposal = useCallback(
    (proposalId: string) =>
      runMutation(confirmOwnershipHolderTypeProposal(companyId, proposalId)),
    [companyId, runMutation],
  );
  const handleRejectProposal = useCallback(
    (proposalId: string) => runMutation(rejectOwnershipHolderTypeProposal(companyId, proposalId)),
    [companyId, runMutation],
  );

  // Tier-4 OCR over the residual documents (T8, ADR 0077). The per-company run
  // returns the refreshed overview so any new proposal renders in one round-trip;
  // a no-vision-provider state comes back as a clean no-op (no new proposal), so
  // surface that honestly rather than as an error.
  const handleRunOcr = useCallback(() => {
    setOwnershipBusy(true);
    runCompanyOwnershipOcr(companyId)
      .then((overview) => {
        setOwnership(overview);
        if (overview.ocrProposals.length === 0) {
          toast.show({
            message: text("No OCR provider configured, or OCR found no shareholder table."),
            tone: "neutral",
          });
        }
      })
      .catch((cause) => setOwnershipError(String(cause)))
      .finally(() => setOwnershipBusy(false));
  }, [companyId, text, toast]);
  const handleConfirmOcrProposal = useCallback(
    (reportDocumentId: string) =>
      runMutation(confirmOwnershipOcrProposal(companyId, reportDocumentId)),
    [companyId, runMutation],
  );
  const handleRejectOcrProposal = useCallback(
    (reportDocumentId: string) =>
      runMutation(rejectOwnershipOcrProposal(companyId, reportDocumentId)),
    [companyId, runMutation],
  );

  const sectorSourceLabel =
    info?.sectorSource === "manual"
      ? text("manual override")
      : info?.sectorSource === "registry"
        ? text("from the registry")
        : null;

  const showFreeFloat = (ownership?.holders.length ?? 0) > 0;

  return (
    <div className="company-tab-panel basic-info-panel" aria-label={text("Basic info")}>
      <SectionHeader
        level="h4"
        title={text("Basic info")}
        meta={
          <Button
            className="compact-button"
            onClick={() => {
              if (editing) setRevision((value) => value + 1);
              setEditing((value) => !value);
            }}
          >
            {editing ? <X size={15} /> : <Pencil size={15} />}
            {editing ? text("Done editing") : text("Edit")}
          </Button>
        }
      />
      {error ? <ErrorText>{text("Failed to load basic info")}: {error}</ErrorText> : null}
      {!info && !error ? <Skeleton variant="list-row" count={5} label={text("Loading…")} /> : null}
      {info ? (
        <InfoGrid
          ariaLabel={text("Basic info")}
          className="basic-info-grid"
          items={[
            { label: text("Name"), value: info.displayName },
            {
              label: text("Ticker"),
              value: <TickerLabel value={info.qualifiedTicker} />,
              valueAriaLabel: `${text("Ticker")}: ${info.qualifiedTicker}`,
            },
            { label: text("ISIN"), value: info.isin ?? DASH },
            {
              label: text("Sector"),
              value: info.sector ? (
                <>
                  {info.sector}
                  {sectorSourceLabel ? <StatusChip tone="neutral">{sectorSourceLabel}</StatusChip> : null}
                </>
              ) : (
                DASH
              ),
              valueAriaLabel: `${text("Sector")}: ${info.sector ?? DASH}`,
            },
            {
              label: text("Shares outstanding"),
              value: info.sharesOutstanding ? (
                <>
                  {formatFinancialValue(
                    { valueNumeric: info.sharesOutstanding, valueKind: "count" },
                    locale,
                  )}
                  {info.sharesOutstandingPeriod ? (
                    <StatusChip tone="neutral">{info.sharesOutstandingPeriod}</StatusChip>
                  ) : null}
                </>
              ) : (
                DASH
              ),
              valueAriaLabel: `${text("Shares outstanding")}: ${info.sharesOutstanding ?? DASH}`,
            },
            ...(showFreeFloat && ownership
              ? [
                  {
                    label: text("Free float (derived)"),
                    value: formatFinancialValue(
                      { valueNumeric: ownership.freeFloatPct, valueKind: "percentage" },
                      locale,
                    ),
                    valueAriaLabel: `${text("Free float (derived)")}: ${ownership.freeFloatPct}`,
                  },
                ]
              : []),
          ]}
        />
      ) : null}
      {editing ? (
        <div className="basic-info-edit">
          <CompanySectorField companyId={companyId} />
          <CompanyIrReportsUrlField companyId={companyId} />
        </div>
      ) : null}
      <OwnershipSection
        data={ownership}
        error={ownershipError}
        busy={ownershipBusy}
        onBackfill={handleBackfill}
        onRunClassification={handleRunClassification}
        onSetHolderType={handleSetHolderType}
        onConfirmProposal={handleConfirmProposal}
        onRejectProposal={handleRejectProposal}
        onRunOcr={handleRunOcr}
        onConfirmOcrProposal={handleConfirmOcrProposal}
        onRejectOcrProposal={handleRejectOcrProposal}
      />
      <InsiderBlock data={insider} error={insiderError} />
    </div>
  );
}
