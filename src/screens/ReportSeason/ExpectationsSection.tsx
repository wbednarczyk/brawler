import { useMemo, useState } from "react";
import { Check, Pencil, PenLine, Plus, X } from "lucide-react";
import { useLocale } from "../../shared/locale";
import { localizedKpiLabelForKey } from "../../shared/locale/kpiLabels";
import {
  ActionButton,
  ActionRow,
  EmptyState,
  ErrorText,
  SectionHeader,
  SelectField,
  StatusChip,
  TextField,
  TextareaField,
} from "../../ui";
import { MarkdownNoteBody } from "../../shared/components/MarkdownNoteBody";
import type { ReportSeasonEntry } from "../../api/reportSeason";
import type { PreReportKpi } from "../../api/reportSeason";
import type {
  ExpectationReview,
  MetricExpectationOutcome,
  NewExpectationMetric,
  ReportExpectation,
} from "../../api/reportExpectations";
import type { ExpectationDraft, ExpectationEntryState } from "./useReportSeason";

// The closed comparator set (report_expectations.rs EXPECTATION_COMPARATORS),
// each shown as its math symbol so the row reads "≥ 1 000 000".
const COMPARATORS = [
  { value: "lt", label: "<" },
  { value: "lte", label: "≤" },
  { value: "eq", label: "=" },
  { value: "gte", label: "≥" },
  { value: "gt", label: ">" },
] as const;

// Fiscal period labels the backend accepts (financials.rs). The user picks the
// period the upcoming report covers — the occurrence key does not carry it.
const PERIOD_TYPES = ["Q1", "Q2", "Q3", "Q4", "H1", "H2", "FY"] as const;

type MetricRow = { metricKey: string; comparator: string; expectedValue: string };

type ExpectationsSectionProps = {
  entry: ReportSeasonEntry;
  kpis: PreReportKpi[];
  state: ExpectationEntryState | undefined;
  busy: boolean;
  /** F4b S4 (contract § Report Season): the card-level primary choreography
   * lives in `ReportSeasonScreen` (it also marks `Mark as prepared`) — this
   * section only renders what it's told is primary in this state. */
  addPrimary: boolean;
  saveVerdictPrimary: boolean;
  composerOpen: boolean;
  onComposerOpenChange: (open: boolean) => void;
  onWrite: (draft: ExpectationDraft) => Promise<void>;
  onResolve: (note: string) => Promise<void>;
};

function comparatorSymbol(comparator: string): string {
  return COMPARATORS.find((item) => item.value === comparator)?.label ?? comparator;
}

export function ExpectationsSection({
  entry,
  kpis,
  state,
  busy,
  addPrimary,
  saveVerdictPrimary,
  composerOpen,
  onComposerOpenChange,
  onWrite,
  onResolve,
}: ExpectationsSectionProps) {
  const { locale } = useLocale();
  const expectation = state?.expectation ?? null;
  const review = state?.review ?? null;
  const frozen = review?.factsAvailable ?? false;

  if (frozen && review) {
    return (
      <ReviewSurface
        review={review}
        busy={busy}
        primary={saveVerdictPrimary}
        locale={locale}
        onResolve={onResolve}
      />
    );
  }

  return (
    <ExpectationEditor
      entry={entry}
      kpis={kpis}
      expectation={expectation}
      busy={busy}
      addPrimary={addPrimary}
      open={composerOpen}
      onOpenChange={onComposerOpenChange}
      onWrite={onWrite}
    />
  );
}

// --- Write / edit (unfrozen) -------------------------------------------------

function ExpectationEditor({
  entry,
  kpis,
  expectation,
  busy,
  addPrimary,
  open,
  onOpenChange,
  onWrite,
}: {
  entry: ReportSeasonEntry;
  kpis: PreReportKpi[];
  expectation: ReportExpectation | null;
  busy: boolean;
  addPrimary: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onWrite: (draft: ExpectationDraft) => Promise<void>;
}) {
  const { locale, text } = useLocale();
  const defaultYear = useMemo(() => {
    const parsed = Number.parseInt(entry.eventDate.slice(0, 4), 10);
    return Number.isFinite(parsed) ? String(parsed) : "";
  }, [entry.eventDate]);

  const [fiscalYear, setFiscalYear] = useState(defaultYear);
  const [periodType, setPeriodType] = useState("");
  const [stanceMd, setStanceMd] = useState("");
  const [rows, setRows] = useState<MetricRow[]>([]);
  const [formError, setFormError] = useState<string | null>(null);

  function openComposer() {
    setFiscalYear(expectation ? String(expectation.fiscalYear) : defaultYear);
    setPeriodType(expectation ? expectation.periodType : "");
    setStanceMd(expectation ? expectation.stanceMd : "");
    setRows(
      expectation
        ? expectation.metrics.map((metric) => ({
            metricKey: metric.metricKey,
            comparator: metric.comparator,
            expectedValue: metric.expectedValue,
          }))
        : [],
    );
    setFormError(null);
    onOpenChange(true);
  }

  const yearValid = /^\d{4}$/.test(fiscalYear.trim());
  const metricsComplete = rows.every(
    (row) => row.metricKey && row.comparator && row.expectedValue.trim() !== "",
  );
  const canSave =
    stanceMd.trim() !== "" && periodType !== "" && yearValid && metricsComplete && !busy;

  async function submit() {
    const metrics: NewExpectationMetric[] = rows.map((row) => {
      const unit = kpis.find((kpi) => kpi.metricKey === row.metricKey)?.unit;
      return {
        metricKey: row.metricKey,
        comparator: row.comparator,
        expectedValue: row.expectedValue.trim(),
        ...(unit ? { unit } : {}),
      };
    });
    try {
      await onWrite({
        fiscalYear: Number.parseInt(fiscalYear.trim(), 10),
        periodType,
        stanceMd: stanceMd.trim(),
        metrics,
      });
      onOpenChange(false);
    } catch (cause) {
      setFormError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  if (!open) {
    return (
      <div className="report-season-expectations">
        <SectionHeader
          level="h3"
          title={text("Expectations")}
          meta={expectation ? text("Editable until the report lands") : undefined}
        />
        {expectation ? (
          <>
            <MarkdownNoteBody body={expectation.stanceMd} ariaLabel={text("Your stance")} />
            {expectation.metrics.length > 0 ? (
              <ul className="report-season-expectation-metrics">
                {expectation.metrics.map((metric) => (
                  <li key={metric.id}>
                    {localizedKpiLabelForKey(metric.metricKey, locale)} {comparatorSymbol(metric.comparator)}{" "}
                    {metric.expectedValue}
                    {metric.unit ? ` ${metric.unit}` : ""}
                  </li>
                ))}
              </ul>
            ) : null}
            <ActionRow>
              <ActionButton verb="edit" variant="secondary" onClick={openComposer}>
                <Pencil size={14} />
                {text("Edit expectations")}
              </ActionButton>
            </ActionRow>
          </>
        ) : (
          <EmptyState
            kind="invitation"
            title={text("No expectations yet")}
            source={text(
              "Record what you expect before the report lands — it freezes once the results are in.",
            )}
            action={
              <ActionButton
                verb="add"
                variant={addPrimary ? "primary" : "secondary"}
                data-ux-primary-action={addPrimary ? "true" : undefined}
                onClick={openComposer}
              >
                <PenLine size={14} />
                {text("Add expectations")}
              </ActionButton>
            }
          />
        )}
      </div>
    );
  }

  return (
    <div className="report-season-expectations">
      <SectionHeader level="h3" title={text("Expectations")} />
      <div className="report-season-expectation-composer">
        <div className="report-season-expectation-period">
          <TextField
            label={text("Fiscal year")}
            inputMode="numeric"
            value={fiscalYear}
            disabled={expectation !== null}
            onChange={(event) => setFiscalYear(event.target.value)}
          />
          <SelectField
            label={text("Period type")}
            value={periodType}
            disabled={expectation !== null}
            onChange={(event) => setPeriodType(event.target.value)}
          >
            <option value="">{text("Select period")}</option>
            {PERIOD_TYPES.map((value) => (
              <option key={value} value={value}>
                {value}
              </option>
            ))}
          </SelectField>
        </div>

        <TextareaField
          label={text("Your stance")}
          rows={4}
          value={stanceMd}
          placeholder={text("What do you expect from this report, and why?")}
          onChange={(event) => setStanceMd(event.target.value)}
        />

        {rows.map((row, index) => (
          <div className="report-season-metric-row" key={index}>
            <SelectField
              label={text("Metric")}
              value={row.metricKey}
              onChange={(event) =>
                setRows((current) =>
                  current.map((item, i) =>
                    i === index ? { ...item, metricKey: event.target.value } : item,
                  ),
                )
              }
            >
              <option value="">{text("Select metric")}</option>
              {kpis.map((kpi) => (
                <option key={kpi.metricKey} value={kpi.metricKey}>
                  {kpi.label}
                </option>
              ))}
            </SelectField>
            <SelectField
              label={text("Direction")}
              value={row.comparator}
              onChange={(event) =>
                setRows((current) =>
                  current.map((item, i) =>
                    i === index ? { ...item, comparator: event.target.value } : item,
                  ),
                )
              }
            >
              {COMPARATORS.map((comparator) => (
                <option key={comparator.value} value={comparator.value}>
                  {comparator.label}
                </option>
              ))}
            </SelectField>
            <TextField
              label={text("Expected value")}
              inputMode="decimal"
              value={row.expectedValue}
              onChange={(event) =>
                setRows((current) =>
                  current.map((item, i) =>
                    i === index ? { ...item, expectedValue: event.target.value } : item,
                  ),
                )
              }
            />
            <ActionButton
              verb="remove"
              variant="icon"
              aria-label={text("Remove metric")}
              onClick={() => setRows((current) => current.filter((_, i) => i !== index))}
            >
              <X size={14} />
            </ActionButton>
          </div>
        ))}

        <ActionRow>
          <ActionButton
            verb="add"
            variant="minimal"
            onClick={() =>
              setRows((current) => [...current, { metricKey: "", comparator: "gte", expectedValue: "" }])
            }
          >
            <Plus size={14} />
            {text("Add metric")}
          </ActionButton>
        </ActionRow>

        {formError ? <ErrorText>{formError}</ErrorText> : null}

        <ActionRow>
          <ActionButton
            verb="save"
            variant="primary"
            data-ux-primary-action="true"
            disabled={!canSave}
            onClick={() => void submit()}
          >
            <Check size={14} />
            {text("Save")}
          </ActionButton>
          <ActionButton kind="control" variant="minimal" onClick={() => onOpenChange(false)}>
            {text("Cancel")}
          </ActionButton>
        </ActionRow>
      </div>
    </div>
  );
}

// --- Review (frozen) ---------------------------------------------------------

function ReviewSurface({
  review,
  busy,
  primary,
  locale,
  onResolve,
}: {
  review: ExpectationReview;
  busy: boolean;
  primary: boolean;
  locale: "en" | "pl";
  onResolve: (note: string) => Promise<void>;
}) {
  const { text } = useLocale();
  const [note, setNote] = useState("");
  const [formError, setFormError] = useState<string | null>(null);
  const resolved = review.resolutionNoteMd !== null;

  function outcomeChip(outcome: MetricExpectationOutcome) {
    if (outcome === "met") {
      return <StatusChip tone="ok">{text("Met")}</StatusChip>;
    }
    if (outcome === "missed") {
      return <StatusChip tone="warn">{text("Missed")}</StatusChip>;
    }
    return <StatusChip tone="neutral">{text("No data")}</StatusChip>;
  }

  async function submit() {
    try {
      await onResolve(note.trim());
      setNote("");
    } catch (cause) {
      setFormError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  return (
    <div className="report-season-expectations">
      <SectionHeader
        level="h3"
        title={text("Expectations vs actuals")}
        meta={text("Locked — the report is in")}
      />
      <MarkdownNoteBody body={review.stanceMd} ariaLabel={text("Your stance")} />

      {review.metrics.length > 0 ? (
        <div
          className="report-season-expectation-review"
          role="table"
          aria-label={text("Expectations vs actuals")}
          data-hscroll
        >
          <div className="report-season-expectation-review-head" role="row">
            <span role="columnheader">{text("Metric")}</span>
            <span role="columnheader">{text("Expected")}</span>
            <span role="columnheader">{text("Actual")}</span>
            <span role="columnheader">{text("Outcome")}</span>
          </div>
          {review.metrics.map((metric, index) => (
            <div className="report-season-expectation-review-row" role="row" key={`${metric.metricKey}-${index}`}>
              <span role="cell">{localizedKpiLabelForKey(metric.metricKey, locale)}</span>
              <span role="cell">
                {comparatorSymbol(metric.comparator)} {metric.expectedValue}
                {metric.unit ? ` ${metric.unit}` : ""}
              </span>
              <span role="cell">
                {metric.actualValue ?? "—"}
                {metric.actualValue && metric.unit ? ` ${metric.unit}` : ""}
              </span>
              <span role="cell">{outcomeChip(metric.outcome)}</span>
            </div>
          ))}
        </div>
      ) : null}

      {resolved ? (
        <div className="report-season-expectation-verdict">
          <StatusChip tone="ok">{text("Verdict recorded")}</StatusChip>
          <MarkdownNoteBody
            body={review.resolutionNoteMd ?? ""}
            ariaLabel={text("Your verdict")}
          />
        </div>
      ) : (
        <>
          <TextareaField
            label={text("Your verdict")}
            rows={3}
            value={note}
            placeholder={text("How did the results compare to what you expected?")}
            onChange={(event) => setNote(event.target.value)}
          />
          {formError ? <ErrorText>{formError}</ErrorText> : null}
          <ActionRow>
            <ActionButton
              verb="save"
              variant={primary ? "primary" : "secondary"}
              data-ux-primary-action={primary ? "true" : undefined}
              disabled={note.trim() === "" || busy}
              onClick={() => void submit()}
            >
              <Check size={14} />
              {text("Save verdict")}
            </ActionButton>
          </ActionRow>
        </>
      )}
    </div>
  );
}
