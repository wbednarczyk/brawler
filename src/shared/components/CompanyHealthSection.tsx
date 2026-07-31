import { useEffect, useMemo, useState } from "react";

import { getCompanyHealth } from "../../api/companyHealth";
import type {
  AltmanBand,
  AltmanScore,
  CompanyHealth,
  HealthPeriodScores,
  MeasuredInput,
  MissingInput,
  PiotroskiScore,
} from "../../api/companyHealth";
import { useLocale, type LocaleCode } from "../locale";
import { localizedKpiLabelForKey } from "../locale/kpiLabels";
import { EmptyState, ErrorText, ExpandableRow, Hint, SectionHeader, StatusChip } from "../../ui";

type Props = {
  companyId: string;
};

type ChipTone = "neutral" | "accent" | "ok" | "warn" | "danger";

/** 7–9 strong (ok), 4–6 mixed (warn), 0–3 weak (danger) — a display tone, not advice. */
function piotroskiTone(score: number): ChipTone {
  if (score >= 7) return "ok";
  if (score >= 4) return "warn";
  return "danger";
}

function altmanTone(band: AltmanBand): ChipTone {
  switch (band) {
    case "safe":
      return "ok";
    case "grey":
      return "warn";
    default:
      return "danger";
  }
}

type Localize = (value: string) => string;

/** Localized value for the F5/F9 shared averaging-basis marker (health.rs
 * `AvgBasis::label`) — never the raw `two_year_average`/`period_end` token. */
function basisValueLabel(value: string, text: Localize): string {
  if (value === "two_year_average") return text("two-year average");
  if (value === "period_end") return text("period-end");
  return value;
}

/** F5/F9 record their averaging/leverage basis as extra `MeasuredInput`
 * entries (`basis`, `leverage_input`) rather than a `<metric>@FY<year>`
 * reading — localize those two marker keys explicitly so their English
 * config tokens never leak into the detail line; returns `null` for an
 * ordinary metric-reading input, left to render as-is. */
function markerInputLabel(input: MeasuredInput, text: Localize): string | null {
  if (input.key === "basis") {
    return `${text("Averaging basis")} = ${basisValueLabel(input.value, text)}`;
  }
  if (input.key === "leverage_input") {
    return `${text("Leverage basis")} = ${text("non-current liabilities")}`;
  }
  return null;
}

/** Render one measured input as `key = value`; F5/F9's basis markers get a
 * localized label + value (see {@link markerInputLabel}), while numeric
 * `<metric>@FY<year>` citations render as-is (Decision 3's evidentiary
 * format — a precise reading, not an internal metric id in prose). */
function inputsLine(inputs: MeasuredInput[], text: Localize): string {
  return inputs.map((i) => markerInputLabel(i, text) ?? `${i.key} = ${i.value}`).join(" · ");
}

/** A degenerate zero-denominator Altman input records its period as
 * `FY{year} (zero)` (health.rs `altman_component`) — localize the `(zero)`
 * suffix rather than showing the raw English word. */
function periodLabel(period: string, text: Localize): string {
  const zeroMatch = /^(FY\d+) \(zero\)$/.exec(period);
  if (zeroMatch) return `${zeroMatch[1]} — ${text("zero denominator")}`;
  return period;
}

/** One missing-input entry, localized: `prior_fy_period` (the oldest FY has
 * no prior year to compare against) renders as a human sentence with its
 * fiscal year interpolated — never the raw `prior_fy_period` token
 * concatenated with the backend's English `before FY{year}` reason phrase.
 * Every other entry names its metric via the same localized-KPI-name
 * mechanism the Fundamentals panel uses ({@link localizedKpiLabelForKey}). */
function missingItemLabel(m: MissingInput, text: Localize, locale: LocaleCode): string {
  if (m.metric === "prior_fy_period") {
    const year = /FY(\d+)/.exec(m.period)?.[1] ?? "";
    return text("prior fiscal year (needed before FY{year})").replace("{year}", year);
  }
  return `${localizedKpiLabelForKey(m.metric, locale)} (${periodLabel(m.period, text)})`;
}

function missingLine(missing: MissingInput[], text: Localize, locale: LocaleCode): string {
  return missing.map((m) => missingItemLabel(m, text, locale)).join(", ");
}

/** The `NotApplicable` reason echoes the company's `statement_type` column
 * (`banking` / `insurance` / `specialty_finance` / `brokerage` / `reit`,
 * migrations 0095/0098/0127) — localize the recognized values; an unrecognized
 * value still renders (never blocked), just untranslated. */
function notApplicableReasonLabel(reason: string, text: Localize): string {
  switch (reason) {
    case "banking":
      return text("Banking");
    case "insurance":
      return text("Insurance");
    case "specialty_finance":
      // Debt collection since the 2026-07-31 split — brokers moved to `brokerage`.
      return text("Specialty finance");
    case "brokerage":
      return text("Brokerage");
    case "reit":
      return text("REIT");
    default:
      return reason;
  }
}

/// The "Company health" section of the Quality panel (ADR 0083): deterministic
/// Piotroski F + Altman Z″ tiles with expandable per-component breakdowns,
/// published-formula citations, and explicit insufficient-data / not-applicable
/// states. Decision support only — no verdict/advice language.
export function CompanyHealthSection({ companyId }: Props) {
  const { text, locale } = useLocale();
  const [health, setHealth] = useState<CompanyHealth | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<"piotroski" | "altman" | null>(null);

  useEffect(() => {
    let cancelled = false;
    setExpanded(null);
    getCompanyHealth(companyId)
      .then((result) => {
        if (!cancelled) setHealth(result);
      })
      .catch((reason) => {
        if (!cancelled) setError(reason instanceof Error ? reason.message : String(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [companyId]);

  // Stable name → localized copy maps (built once per locale).
  const signalNames = useMemo<Record<string, string>>(
    () => ({
      roa_positive: text("Return on assets is positive"),
      cfo_positive: text("Operating cash flow is positive"),
      roa_improved: text("Return on assets improved"),
      accrual_quality: text("Cash flow exceeds profit (accrual quality)"),
      leverage_improved: text("Long-term leverage did not rise"),
      liquidity_improved: text("Current ratio improved"),
      no_dilution: text("No share dilution"),
      margin_improved: text("Gross margin improved"),
      turnover_improved: text("Asset turnover improved"),
    }),
    [text],
  );
  const componentNames = useMemo<Record<string, string>>(
    () => ({
      working_capital_to_assets: text("Working capital / assets"),
      retained_earnings_to_assets: text("Retained earnings / assets"),
      ebit_to_assets: text("Operating profit / assets"),
      equity_to_liabilities: text("Equity / liabilities"),
    }),
    [text],
  );
  const bandLabel = useMemo<Record<AltmanBand, string>>(
    () => ({
      safe: text("Safe"),
      grey: text("Grey zone"),
      distress: text("Distress"),
    }),
    [text],
  );

  const latest: HealthPeriodScores | null = health?.latest ?? null;

  function piotroskiChip(score: PiotroskiScore) {
    switch (score.state) {
      case "headline":
        return (
          <StatusChip tone={piotroskiTone(score.score)}>{`${score.score}/9`}</StatusChip>
        );
      case "insufficient_data":
        return <StatusChip tone="neutral">{text("Insufficient data")}</StatusChip>;
      default:
        return <StatusChip tone="neutral">{text("Not applicable")}</StatusChip>;
    }
  }

  function altmanChip(score: AltmanScore) {
    switch (score.state) {
      case "headline":
        return (
          <StatusChip tone={altmanTone(score.band)}>
            {`${score.zScore} · ${bandLabel[score.band]}`}
          </StatusChip>
        );
      case "insufficient_data":
        return <StatusChip tone="neutral">{text("Insufficient data")}</StatusChip>;
      default:
        return <StatusChip tone="neutral">{text("Not applicable")}</StatusChip>;
    }
  }

  function piotroskiDetail(score: PiotroskiScore) {
    if (score.state === "not_applicable") {
      return (
        <div className="quality-quantitative-detail">
          <Hint>{`${text("Not applicable to financial statements")} (${notApplicableReasonLabel(score.reason, text)}).`}</Hint>
        </div>
      );
    }
    return (
      <div className="quality-quantitative-detail">
        <ul className="ui-list-rows quality-history-detail">
          {score.signals.map((signal) => (
            <li key={signal.code} className="quality-detail-line" data-signal={signal.code}>
              <span className="quality-detail-label">
                {`${signal.code} — ${signalNames[signal.name] ?? signal.name}`}
              </span>{" "}
              <StatusChip tone={signal.passed ? "ok" : "neutral"}>
                {signal.passed ? text("Pass") : text("No point")}
              </StatusChip>{" "}
              <span className="quality-measured">{inputsLine(signal.inputs, text)}</span>
            </li>
          ))}
        </ul>
        {score.state === "insufficient_data" ? (
          <Hint>{`${text("Missing inputs")}: ${missingLine(score.missing, text, locale)}`}</Hint>
        ) : null}
      </div>
    );
  }

  function altmanDetail(score: AltmanScore) {
    if (score.state === "not_applicable") {
      return (
        <div className="quality-quantitative-detail">
          <Hint>{`${text("Not applicable to financial statements")} (${notApplicableReasonLabel(score.reason, text)}).`}</Hint>
        </div>
      );
    }
    return (
      <div className="quality-quantitative-detail">
        <ul className="ui-list-rows quality-history-detail">
          {score.components.map((component) => (
            <li key={component.code} className="quality-detail-line" data-component={component.code}>
              <span className="quality-detail-label">
                {`${component.code} — ${componentNames[component.name] ?? component.name}`}
              </span>{" "}
              <span className="quality-measured">
                {`${component.ratio} × ${component.weight} = ${component.contribution}`}
              </span>{" "}
              <span className="quality-measured">{inputsLine(component.inputs, text)}</span>
            </li>
          ))}
        </ul>
        {score.state === "insufficient_data" ? (
          <Hint>{`${text("Missing inputs")}: ${missingLine(score.missing, text, locale)}`}</Hint>
        ) : null}
      </div>
    );
  }

  return (
    <section className="company-health-section" aria-label={text("Company health")}>
      <SectionHeader
        level="h4"
        title={text("Company health")}
        description={text("Published-formula health scores over confirmed annual facts. Decision support only.")}
      />
      {error ? <ErrorText>{error}</ErrorText> : null}
      {!error && !latest ? (
        <EmptyState>{text("No annual periods yet — health scores need at least one full-year report.")}</EmptyState>
      ) : null}
      {latest ? (
        <ul className="ui-list-rows quality-health-scores">
          <li>
            <ExpandableRow
              label={health?.piotroskiVariant ?? text("Piotroski F")}
              isExpanded={expanded === "piotroski"}
              onToggle={() => setExpanded(expanded === "piotroski" ? null : "piotroski")}
              detail={piotroskiDetail(latest.piotroski)}
            >
              <span className="quality-criterion-header">
                <span className="quality-criterion-label">
                  {health?.piotroskiVariant ?? text("Piotroski F")}
                </span>
                <span className="quality-criterion-trailing">{piotroskiChip(latest.piotroski)}</span>
              </span>
            </ExpandableRow>
          </li>
          <li>
            <ExpandableRow
              label={health?.altmanVariant ?? text("Altman Z")}
              isExpanded={expanded === "altman"}
              onToggle={() => setExpanded(expanded === "altman" ? null : "altman")}
              detail={altmanDetail(latest.altman)}
            >
              <span className="quality-criterion-header">
                <span className="quality-criterion-label">
                  {health?.altmanVariant ?? text("Altman Z")}
                </span>
                <span className="quality-criterion-trailing">{altmanChip(latest.altman)}</span>
              </span>
            </ExpandableRow>
          </li>
        </ul>
      ) : null}
    </section>
  );
}
