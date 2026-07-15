import { useEffect, useState } from "react";
import { Pencil, X } from "lucide-react";

import { getCompanyBasicInfo, type CompanyBasicInfo } from "../../api/companyBasicInfo";
import { formatFinancialValue } from "../format/financialValue";
import { useLocale } from "../locale";
import { Button, ErrorText, InfoGrid, SectionHeader, Skeleton, StatusChip } from "../../ui";
import { CompanyIrReportsUrlField } from "./CompanyIrReportsUrlField";
import { CompanySectorField } from "./CompanySectorField";
import { TickerLabel } from "./TickerLabel";

type CompanyBasicInfoPanelProps = {
  companyId: string;
};

const DASH = "—";

/// "Basic info" cockpit panel (owner request 2026-07-14, mockup
/// docs/mockups/basic-info-panel.html): identity facts (name, ticker, ISIN),
/// sector with provenance, latest recorded shares_outstanding — read-only by
/// default. Edit affordances (sector override, IR reports URL) stay hidden
/// behind ONE panel-level Edit toggle, never per-fact buttons. A GLOBAL
/// edit-mode pattern is a separate analysis task; this local toggle bridges
/// until it lands.
export function CompanyBasicInfoPanel({ companyId }: CompanyBasicInfoPanelProps) {
  const { text, locale } = useLocale();
  const [info, setInfo] = useState<CompanyBasicInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  // Bump to re-fetch after the edit fields save (sector changes provenance).
  const [revision, setRevision] = useState(0);

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
    setEditing(false);
    setInfo(null);
  }, [companyId]);

  const sectorSourceLabel =
    info?.sectorSource === "manual"
      ? text("manual override")
      : info?.sectorSource === "registry"
        ? text("from the registry")
        : null;

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
          ]}
        />
      ) : null}
      {editing ? (
        <div className="basic-info-edit">
          <CompanySectorField companyId={companyId} />
          <CompanyIrReportsUrlField companyId={companyId} />
        </div>
      ) : null}
    </div>
  );
}
