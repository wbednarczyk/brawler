import { useEffect, useMemo, useRef, useState } from "react";

import { getSectorPercentiles } from "../../api/sectorPercentiles";
import type { SectorPercentiles } from "../../api/sectorPercentiles";
import { computeComparativeValuation } from "../../api/valuation";
import type { ComparativeValuation } from "../../api/valuation";
import type { Company } from "../../api/types";
import { groupFormat, formatFixedPercent } from "../../shared/format/financialValue";
import { localizedKpiLabelForKey } from "../../shared/locale/kpiLabels";
import { useLocale, type LocaleCode } from "../../shared/locale";
import type { LocaleKey } from "../../shared/locale/resources/en";
import {
  Button,
  EmptyState,
  ErrorText,
  Hint,
  RangeBarChart,
  SectionHeader,
  SelectField,
  Skeleton,
  StatusChip,
  type RangeBarRow,
} from "../../ui";

// Reactive compute, same idiom as the rest of the Compare screen: the valuation
// re-reads on every change of the scoped company. A short debounce coalesces a
// burst of selector changes into one pair of reads.
const VALUATION_DEBOUNCE_MS = 180;

type LoadState = "idle" | "loading" | "error";

// The three level-1 methods (ADR 0089 dec. 4) → their localized "× median" label.
const METHOD_LABEL: Record<string, LocaleKey> = {
  pe_multiple: "compare.valuation.method.pe",
  ev_ebitda_multiple: "compare.valuation.method.evEbitda",
  pbv_multiple: "compare.valuation.method.pbv",
};

// Typed method-absence reason → its translated phrase (never a raw enum/0-bar).
const ABSENT_LABEL: Record<string, LocaleKey> = {
  no_driver: "compare.valuation.absent.no_driver",
  non_positive_driver: "compare.valuation.absent.non_positive_driver",
  insufficient_peers: "compare.valuation.absent.insufficient_peers",
};

// Market-ratio metric key → its short percentile-chip label. Other keys fall
// back to the canonical KPI label resolver (never a raw English def.label).
const METRIC_LABEL: Record<string, LocaleKey> = {
  pe_ratio: "compare.valuation.metric.pe_ratio",
  ev_ebitda: "compare.valuation.metric.ev_ebitda",
  pbv_ratio: "compare.valuation.metric.pbv_ratio",
  dividend_yield: "compare.valuation.metric.dividend_yield",
  fcf_yield: "compare.valuation.metric.fcf_yield",
};

function fmtMoney(locale: LocaleCode, decimal: string | null): string | null {
  if (decimal === null) return null;
  const value = Number(decimal);
  if (!Number.isFinite(value)) return null;
  return groupFormat(value, locale, 0);
}

/** Grade component in `0..1` → a whole-percent string (raw Intl is gated). */
function fmtComponent(locale: LocaleCode, decimal: string): string {
  const value = Number(decimal);
  if (!Number.isFinite(value)) return decimal;
  return formatFixedPercent(value * 100, locale, 0);
}

export type CompareValuationProps = {
  /** The selected zestaw; the section scopes to ONE of them at a time. */
  companyIds: string[];
  companyById: Map<string, Company>;
};

/**
 * Comparative valuation L1 (ADR 0089 dec. 4, storyboard frame 4 bottom + frame
 * 6). Scoped to one company among the zestaw (default = first): percentile chips
 * with an honest N, the football field (implied range per method + a current-
 * price marker; typed absences named, never omitted), and the confidence grade
 * with an inspectable four-component breakdown. Thin/no-sector states render
 * their explicit reason + threshold — never a silent absence. Decision support
 * only; no cheap/expensive or buy/sell language.
 */
export function CompareValuation({ companyIds, companyById }: CompareValuationProps) {
  const { t, locale } = useLocale();

  const [valuationCompanyId, setValuationCompanyId] = useState<string>(companyIds[0] ?? "");
  const [percentiles, setPercentiles] = useState<SectorPercentiles | null>(null);
  const [valuation, setValuation] = useState<ComparativeValuation | null>(null);
  const [status, setStatus] = useState<LoadState>("idle");
  const [breakdownOpen, setBreakdownOpen] = useState(false);
  const requestSeq = useRef(0);

  // Keep the scoped company valid: default to the first selected, and re-pin when
  // the current subject leaves the zestaw (the ✕-recompute path).
  useEffect(() => {
    setValuationCompanyId((current) =>
      current && companyIds.includes(current) ? current : (companyIds[0] ?? ""),
    );
  }, [companyIds]);

  useEffect(() => {
    if (!valuationCompanyId) return;
    setStatus("loading");
    const handle = setTimeout(() => {
      const seq = (requestSeq.current += 1);
      void (async () => {
        try {
          const [pct, val] = await Promise.all([
            getSectorPercentiles(valuationCompanyId),
            computeComparativeValuation(valuationCompanyId),
          ]);
          if (seq !== requestSeq.current) return;
          setPercentiles(pct);
          setValuation(val);
          setStatus("idle");
        } catch {
          if (seq !== requestSeq.current) return;
          setStatus("error");
        }
      })();
    }, VALUATION_DEBOUNCE_MS);
    return () => clearTimeout(handle);
  }, [valuationCompanyId]);

  const retry = () => {
    if (!valuationCompanyId) return;
    setStatus("loading");
    const id = valuationCompanyId;
    const seq = (requestSeq.current += 1);
    void (async () => {
      try {
        const [pct, val] = await Promise.all([
          getSectorPercentiles(id),
          computeComparativeValuation(id),
        ]);
        if (seq !== requestSeq.current) return;
        setPercentiles(pct);
        setValuation(val);
        setStatus("idle");
      } catch {
        if (seq !== requestSeq.current) return;
        setStatus("error");
      }
    })();
  };

  const currency = t("compare.valuation.currency");

  // Percentile chips: ranked market ratios only, each with its own N (sampleSize).
  const percentileChips = useMemo(() => {
    if (!percentiles) return [];
    return percentiles.metrics
      .filter((metric) => metric.kind === "market_ratio" && metric.percentile !== null)
      .map((metric) => {
        const labelKey = METRIC_LABEL[metric.metricKey];
        const label = labelKey ? t(labelKey) : localizedKpiLabelForKey(metric.metricKey, locale);
        const p = Math.round(Number(metric.percentile));
        return { key: metric.metricKey, label, p, n: metric.sampleSize };
      });
  }, [percentiles, t, locale]);

  // Football-field rows: a drawable implied range or a named typed absence.
  const rows = useMemo<RangeBarRow[]>(() => {
    if (!valuation) return [];
    return valuation.methods.map((method) => {
      const labelKey = METHOD_LABEL[method.method];
      const label = labelKey ? t(labelKey) : method.method;
      const low = method.fairLow !== null ? Number(method.fairLow) : null;
      const base = method.fairBase !== null ? Number(method.fairBase) : null;
      const high = method.fairHigh !== null ? Number(method.fairHigh) : null;
      if (low === null || high === null || !Number.isFinite(low) || !Number.isFinite(high)) {
        const reasonKey = method.absentReason ? ABSENT_LABEL[method.absentReason] : undefined;
        return { key: method.method, label, absentText: reasonKey ? t(reasonKey) : "" };
      }
      const lowText = fmtMoney(locale, method.fairLow);
      const highText = fmtMoney(locale, method.fairHigh);
      return {
        key: method.method,
        label,
        low,
        base: base !== null && Number.isFinite(base) ? base : null,
        high,
        rangeText: `${lowText}–${highText} ${currency}`,
      };
    });
  }, [valuation, t, locale, currency]);

  const markerValue = valuation?.currentPrice ? Number(valuation.currentPrice) : null;
  const marker =
    markerValue !== null && Number.isFinite(markerValue)
      ? { value: markerValue, label: `${groupFormat(markerValue, locale, 0)} ${currency}` }
      : null;

  const noSector =
    percentiles?.emptyReason === "no_sector" || valuation?.emptyReason === "no_sector";
  const thin = (valuation?.thin ?? false) && !noSector;
  const peerCount = valuation?.peerCount ?? percentiles?.peerCount ?? 0;
  const subjectTicker = companyById.get(valuationCompanyId)?.qualifiedTicker ?? valuationCompanyId;

  const grade = valuation?.confidence;
  const gradeComponents = grade
    ? [
        { key: "completeness", label: t("compare.valuation.grade.completeness"), value: grade.dataCompleteness },
        { key: "peerDepth", label: t("compare.valuation.grade.peerDepth"), value: grade.peerDepth },
        { key: "convergence", label: t("compare.valuation.grade.convergence"), value: grade.methodConvergence },
        { key: "validation", label: t("compare.valuation.grade.validation"), value: grade.validation },
      ]
    : [];

  return (
    <section className="compare-valuation" aria-label={t("compare.valuation.section")}>
      <SectionHeader
        level="h3"
        title={t("compare.valuation.heading")}
        description={`${subjectTicker} · ${t("compare.valuation.vsPeers")} (N=${peerCount})`}
      />

      {/* Scope selector: pick which company in the zestaw the valuation is for. */}
      {companyIds.length > 1 ? (
        <SelectField
          label={t("compare.valuation.company.label")}
          value={valuationCompanyId}
          onChange={(event) => setValuationCompanyId(event.target.value)}
        >
          {companyIds.map((id) => (
            <option key={id} value={id}>
              {companyById.get(id)?.qualifiedTicker ?? id}
            </option>
          ))}
        </SelectField>
      ) : null}

      {status === "loading" ? (
        <Skeleton variant="list-row" count={3} />
      ) : status === "error" ? (
        <div className="compare-error">
          <ErrorText>{t("compare.valuation.error")}</ErrorText>
          <Button variant="secondary" onClick={retry}>
            {t("compare.retry")}
          </Button>
        </div>
      ) : noSector ? (
        <EmptyState>{t("compare.valuation.noSector")}</EmptyState>
      ) : valuation ? (
        <>
          {thin ? (
            <StatusChip tone="warn" className="compare-valuation-thin">
              {`N=${peerCount} — ${t("compare.valuation.thinReason")} (${t("compare.valuation.threshold")}: 4)`}
            </StatusChip>
          ) : null}

          {percentileChips.length > 0 ? (
            <div className="compare-valuation-chips">
              {percentileChips.map((chip) => (
                <StatusChip key={chip.key} tone="neutral" className="compare-valuation-pctile">
                  {`${chip.label}: p${chip.p} ${t("compare.valuation.percentile.amongPeers")} (N=${chip.n})`}
                </StatusChip>
              ))}
              {grade ? (
                <span className="compare-valuation-grade">
                  <span
                    className="compare-valuation-grade-badge"
                    title={t("compare.valuation.grade.tooltip")}
                  >
                    {`${t("compare.valuation.grade.label")}: ${grade.grade}`}
                  </span>
                  <Button
                    variant="ghost"
                    className="compare-valuation-grade-toggle"
                    aria-label={t("compare.valuation.grade.toggle")}
                    aria-expanded={breakdownOpen}
                    onClick={() => setBreakdownOpen((open) => !open)}
                  >
                    {breakdownOpen ? "▴" : "▾"}
                  </Button>
                </span>
              ) : null}
            </div>
          ) : null}

          {breakdownOpen && grade ? (
            <dl className="compare-valuation-breakdown">
              {gradeComponents.map((component) => (
                <div key={component.key} className="compare-valuation-breakdown-row">
                  <dt>{component.label}</dt>
                  <dd className="num-tabular">{fmtComponent(locale, component.value)}</dd>
                </div>
              ))}
            </dl>
          ) : null}

          <RangeBarChart
            ariaLabel={t("compare.valuation.field.aria")}
            rangeLegendLabel={t("compare.valuation.rangeLegend")}
            markerLegendLabel={t("compare.valuation.markerLegend")}
            marker={marker}
            rows={rows}
          />

          <Hint>{t("compare.valuation.foot")}</Hint>
        </>
      ) : null}
    </section>
  );
}
