import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Bot,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Copy,
  Plus,
  RotateCcw,
  Sparkles,
  Trash2,
} from "lucide-react";

import {
  cloneFramework,
  createFrameworkCriterion,
  createQualityFramework,
  deleteFrameworkCriterion,
  deleteFrameworkEvaluation,
  deleteQualityFramework,
  evaluateFramework,
  getQualitativeAssessment,
  getQualitativeAssessmentStatus,
  listAvailableMetricKeys,
  listFrameworkEvaluations,
  listQualityFrameworks,
  rerunQualitativeCriterion,
  resetFrameworkToTemplate,
  runQualitativeAssessment,
  validateCriterionExpression,
} from "../../api/qualityFrameworks";
import type {
  CriterionResult,
  CriterionVerdict,
  FrameworkEvaluation,
  MetricKeyInfo,
  QualityFramework,
} from "../../api/qualityFrameworksTypes";
import { useFocusAfterRemove } from "../focus/focusAfterRemove";
import { formatCriterionMeasure } from "../format/criterionMeasure";
import { useLocale } from "../locale";
import type { LocaleCode } from "../locale";
import {
  ActionRow,
  Button,
  EmptyState,
  ErrorText,
  ExpandableRow,
  Hint,
  ListRow,
  Modal,
  SectionHeader,
  SegmentedControl,
  SegmentedControlOption,
  SelectField,
  StatusChip,
  TextareaField,
  TextField,
} from "../../ui";

type QualityPanelProps = {
  companyId: string;
};

type ChipTone = "neutral" | "accent" | "ok" | "warn" | "danger";

function verdictTone(verdict: CriterionVerdict): ChipTone {
  switch (verdict) {
    case "pass":
      return "ok";
    case "partial":
      return "warn";
    case "fail":
      return "danger";
    default:
      // unavailable + insufficient_evidence → neutral (could not determine).
      return "neutral";
  }
}

/** A typed evidence reference stored on an agent result (ADR 0075). */
type ParsedCitation = {
  citationKey: string;
  evidenceType: string;
  evidenceId: string;
  label?: string | null;
  snippet?: string | null;
};

// A queued assessment has no completion event here, so poll the current-state
// read this many times at this interval before clearing the "queued" hint.
const ASSESSMENT_POLL_TICKS = 8;
const ASSESSMENT_POLL_INTERVAL_MS = 3000;

/** Parse a `criterion_results.citations` JSON blob into typed refs (safe on null/garbage). */
function parseCitations(json: string | null): ParsedCitation[] {
  if (!json) return [];
  try {
    const parsed: unknown = JSON.parse(json);
    return Array.isArray(parsed) ? (parsed as ParsedCitation[]) : [];
  } catch {
    return [];
  }
}

/**
 * A criterion result's measured value + unit, or null when there is no measure.
 * Rendered through the format layer (owner T7 round-2: the raw decimal-exact
 * engine text — "0.28624734427852605755…" — must never render inline).
 */
function formatMeasured(
  result: Pick<CriterionResult, "measuredValue" | "measuredUnit" | "threshold" | "expression">,
  locale: LocaleCode,
): string | null {
  if (result.measuredValue == null) return null;
  // The engine stores the threshold as a plain decimal ("0.15") even when the
  // criterion was written with a percent literal — join both so the percent
  // intent survives normalization.
  const percentContext = `${result.threshold ?? ""} ${result.expression}`;
  const value = formatCriterionMeasure(result.measuredValue, percentContext, locale);
  return `${value}${result.measuredUnit ? ` ${result.measuredUnit}` : ""}`;
}

/// The Quality tab (ADR 0046): manage user frameworks of DSL criteria and run a
/// deterministic, immutable-snapshot scorecard against the company's confirmed
/// The newest snapshot that carries the quantitative scorecard. Qualitative
/// assessment runs (ADR 0075) write qualitative-only snapshots into the same
/// evaluation history (`source: "agent"` rows, `periodId` null); those must
/// never become the panel's quantitative "latest" — the summary chips and
/// per-criterion verdicts would silently show agent tallies instead of the
/// engine scorecard. ADR 0075 Decision 5: the quant current-state read is the
/// newest snapshot with engine results; the qual current-state read is
/// `getQualitativeAssessment`.
function latestQuantitativeEvaluation(
  rows: FrameworkEvaluation[],
): FrameworkEvaluation | null {
  return rows.find((row) => row.results.some((result) => result.source !== "agent")) ?? null;
}

/// fundamentals facts. Decision support only — no buy/sell output.
export function QualityPanel({ companyId }: QualityPanelProps) {
  const { text, locale } = useLocale();
  const [frameworks, setFrameworks] = useState<QualityFramework[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [latest, setLatest] = useState<FrameworkEvaluation | null>(null);
  const [history, setHistory] = useState<FrameworkEvaluation[]>([]);
  // Deleting an evaluation run must not strand focus on <body> (ADR 0076 D9);
  // move it to the next history row. The row is the ExpandableRow article.
  const { listRef: historyListRef } = useFocusAfterRemove<HTMLDivElement>(
    history.map((evaluation) => evaluation.id),
    { rowSelector: ".quality-history-row" },
  );
  const [expandedRunId, setExpandedRunId] = useState<string | null>(null);
  const [expandedCriterionId, setExpandedCriterionId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Density (ADR 0076 D6, Quality): the evaluation history is always reachable
  // behind a disclosure and folds by default in the short height tier. The tier
  // is read from the hosting pane (the `@container pane` queries resolve against
  // it); a manual toggle wins once the user interacts.
  const panelRef = useRef<HTMLElement>(null);
  const [paneShort, setPaneShort] = useState(false);
  const [historyManual, setHistoryManual] = useState<boolean | null>(null);
  const historyOpen = historyManual ?? !paneShort;

  useEffect(() => {
    const pane = panelRef.current?.closest<HTMLElement>(".cockpit-pane");
    if (!pane || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => {
      setPaneShort(pane.clientHeight < 480);
    });
    observer.observe(pane);
    return () => observer.disconnect();
  }, []);

  // Add-criterion form.
  const [criterionKind, setCriterionKind] = useState<"quantitative" | "qualitative">("quantitative");
  const [label, setLabel] = useState("");
  const [expression, setExpression] = useState("");
  const [guidance, setGuidance] = useState("");
  const [exprError, setExprError] = useState<string | null>(null);
  const [exprMetrics, setExprMetrics] = useState<string[]>([]);
  const [metricKeys, setMetricKeys] = useState<MetricKeyInfo[]>([]);

  // Current-state agent assessments (most-recent per qualitative criterion,
  // ADR 0075 Decision 5) — NOT the latest snapshot, which may be quant-only.
  const [qualResults, setQualResults] = useState<CriterionResult[]>([]);
  const [assessmentQueued, setAssessmentQueued] = useState(false);
  const [qualRefreshToken, setQualRefreshToken] = useState(0);

  // Name-a-framework modal (shared by New and Clone).
  const [nameModal, setNameModal] = useState<null | { mode: "new" | "clone"; value: string }>(null);

  const selected = useMemo(
    () => frameworks.find((framework) => framework.id === selectedId) ?? null,
    [frameworks, selectedId],
  );

  const hasQualitative = useMemo(
    () => (selected?.criteria ?? []).some((criterion) => criterion.kind === "qualitative"),
    [selected],
  );

  const verdictLabel = useCallback(
    (verdict: CriterionVerdict) => {
      switch (verdict) {
        case "pass":
          return text("Pass");
        case "partial":
          return text("Partial");
        case "fail":
          return text("Fail");
        case "insufficient_evidence":
          return text("Insufficient evidence");
        default:
          return text("No data");
      }
    },
    [text],
  );

  const confidenceLabel = useCallback(
    (confidence: string) => {
      switch (confidence) {
        case "high":
          return text("High");
        case "medium":
          return text("Medium");
        case "low":
          return text("Low");
        default:
          return confidence;
      }
    },
    [text],
  );

  const fail = useCallback((reason: unknown) => {
    setError(reason instanceof Error ? reason.message : String(reason));
  }, []);

  const reloadFrameworks = useCallback(
    async (preferId?: string) => {
      const rows = await listQualityFrameworks();
      setFrameworks(rows);
      setSelectedId((current) => {
        const wanted = preferId ?? current;
        if (wanted && rows.some((framework) => framework.id === wanted)) return wanted;
        return rows[0]?.id ?? null;
      });
    },
    [],
  );

  useEffect(() => {
    reloadFrameworks().catch(fail);
  }, [reloadFrameworks, fail]);

  // Load the metric catalog for this company so criteria can be built by picking
  // metrics from a list rather than recalling key names.
  useEffect(() => {
    let cancelled = false;
    listAvailableMetricKeys(companyId)
      .then((keys) => {
        if (!cancelled) setMetricKeys(keys);
      })
      .catch((reason) => {
        if (!cancelled) fail(reason);
      });
    return () => {
      cancelled = true;
    };
  }, [companyId, fail]);

  // Load the latest evaluation + history whenever the framework or company changes.
  useEffect(() => {
    if (!selectedId) {
      setLatest(null);
      setHistory([]);
      return;
    }
    let cancelled = false;
    listFrameworkEvaluations({ frameworkId: selectedId, companyId })
      .then((rows) => {
        if (cancelled) return;
        setHistory(rows);
        setLatest(latestQuantitativeEvaluation(rows));
      })
      .catch((reason) => {
        if (!cancelled) fail(reason);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedId, companyId, fail]);

  // Map criterionId → its latest verdict result.
  const resultByCriterion = useMemo(() => {
    const map = new Map<string, FrameworkEvaluation["results"][number]>();
    for (const result of latest?.results ?? []) {
      if (result.criterionId) map.set(result.criterionId, result);
    }
    return map;
  }, [latest]);

  // Map criterionId → its most-recent agent assessment (current-state read).
  const qualResultByCriterion = useMemo(() => {
    const map = new Map<string, CriterionResult>();
    for (const result of qualResults) {
      if (result.criterionId) map.set(result.criterionId, result);
    }
    return map;
  }, [qualResults]);

  // Single fetch path for the current-state agent assessments (framework/company
  // change, or a bumped refresh token after enqueue polling). One effect owns the
  // read so the auto-load and the post-Assess refresh never diverge.
  useEffect(() => {
    let cancelled = false;
    if (!selectedId) {
      setQualResults([]);
      return;
    }
    getQualitativeAssessment({ frameworkId: selectedId, companyId })
      .then((rows) => {
        if (!cancelled) setQualResults(rows);
      })
      .catch((reason) => {
        if (!cancelled) fail(reason);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedId, companyId, qualRefreshToken, fail]);

  // A queued assessment is async (durable job); there is no completion event
  // here, so poll its lifecycle status a bounded number of times. On `failed`
  // (P1: the async job erred — no provider, uncited response — and the hint used
  // to clear silently) stop and surface the backend error; on `succeeded` refresh
  // the current-state read; otherwise keep polling while queued/running.
  useEffect(() => {
    if (!assessmentQueued || !selectedId) return;
    let ticks = 0;
    let cancelled = false;
    const frameworkId = selectedId;
    const timer = setInterval(() => {
      ticks += 1;
      getQualitativeAssessmentStatus({ frameworkId, companyId })
        .then((status) => {
          if (cancelled) return;
          if (status.status === "failed") {
            clearInterval(timer);
            setAssessmentQueued(false);
            setError(`${text("Qualitative assessment failed")}: ${status.lastError ?? ""}`);
            return;
          }
          if (status.status === "succeeded") {
            clearInterval(timer);
            setAssessmentQueued(false);
            setQualRefreshToken((token) => token + 1);
            return;
          }
          // queued / running / idle → keep polling; refresh so a completed run
          // that raced ahead of the status read still appears.
          setQualRefreshToken((token) => token + 1);
        })
        .catch(() => {
          // A transient poll error is not fatal; the tick budget still bounds this.
        });
      if (ticks >= ASSESSMENT_POLL_TICKS) {
        clearInterval(timer);
        setAssessmentQueued(false);
      }
    }, ASSESSMENT_POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [assessmentQueued, selectedId, companyId, text]);

  // Switching framework/company clears a stale "queued" hint from another context.
  useEffect(() => {
    setAssessmentQueued(false);
  }, [selectedId, companyId]);

  async function runGuarded(action: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (reason) {
      fail(reason);
    } finally {
      setBusy(false);
    }
  }

  function onExpressionChange(value: string) {
    setExpression(value);
    if (value.trim() === "") {
      setExprError(null);
      setExprMetrics([]);
      return;
    }
    validateCriterionExpression(value)
      .then((result) => {
        setExprError(result.ok ? null : result.error);
        setExprMetrics(result.referencedMetricKeys);
      })
      .catch(fail);
  }

  // Insert a picked metric key into the expression (appended, space-separated),
  // so metrics are selectable from a list instead of typed from memory.
  function insertMetric(key: string) {
    if (key === "") return;
    const next = expression.trim() === "" ? key : `${expression.trimEnd()} ${key}`;
    onExpressionChange(next);
  }

  function handleEvaluate() {
    if (!selectedId) return;
    void runGuarded(async () => {
      const evaluation = await evaluateFramework({ frameworkId: selectedId, companyId });
      setLatest(evaluation);
      setHistory((current) => [evaluation, ...current]);
    });
  }

  function handleAddCriterion() {
    if (!selectedId || label.trim() === "") return;
    const qualitative = criterionKind === "qualitative";
    if (qualitative ? guidance.trim() === "" : expression.trim() === "" || exprError != null) return;
    void runGuarded(async () => {
      await createFrameworkCriterion({
        frameworkId: selectedId,
        label: label.trim(),
        expression: qualitative ? "" : expression.trim(),
        kind: criterionKind,
        assessmentGuidance: qualitative ? guidance.trim() : null,
      });
      setLabel("");
      setExpression("");
      setGuidance("");
      setExprMetrics([]);
      await reloadFrameworks(selectedId);
    });
  }

  // Enqueue the agent assessment over the framework's qualitative criteria. The
  // job is async (surfaced via the jobs read model); we show a queued hint and
  // refresh the current-state read so a completed run appears on the next load.
  function handleRunAssessment() {
    if (!selectedId) return;
    void runGuarded(async () => {
      await runQualitativeAssessment({ frameworkId: selectedId, companyId });
      // Don't refresh now — the job has only been queued; the poll effect picks
      // up the completed results and clears the hint.
      setAssessmentQueued(true);
    });
  }

  function handleRerunCriterion(criterionId: string) {
    if (!selectedId) return;
    void runGuarded(async () => {
      await rerunQualitativeCriterion({ frameworkId: selectedId, companyId, criterionId });
      setAssessmentQueued(true);
    });
  }

  function handleDeleteCriterion(id: string) {
    if (!selectedId) return;
    void runGuarded(async () => {
      await deleteFrameworkCriterion(id);
      await reloadFrameworks(selectedId);
    });
  }

  function handleDeleteEvaluation(id: string) {
    if (!selectedId) return;
    void runGuarded(async () => {
      await deleteFrameworkEvaluation(id);
      const rows = await listFrameworkEvaluations({ frameworkId: selectedId, companyId });
      setHistory(rows);
      setLatest(latestQuantitativeEvaluation(rows));
    });
  }

  function handleSubmitName() {
    if (!nameModal) return;
    const name = nameModal.value.trim();
    const mode = nameModal.mode;
    setNameModal(null);
    void runGuarded(async () => {
      if (mode === "new") {
        const created = await createQualityFramework({ name: name || text("New framework") });
        await reloadFrameworks(created.id);
      } else if (selectedId) {
        const clone = await cloneFramework({ frameworkId: selectedId, name: name || null });
        await reloadFrameworks(clone.id);
      }
    });
  }

  function handleReset() {
    if (!selectedId) return;
    void runGuarded(async () => {
      await resetFrameworkToTemplate(selectedId);
      await reloadFrameworks(selectedId);
    });
  }

  function handleDeleteFramework() {
    if (!selectedId) return;
    void runGuarded(async () => {
      await deleteQualityFramework(selectedId);
      await reloadFrameworks();
    });
  }

  const deleteCriterionButton = (criterionId: string) => (
    <Button
      icon={<Trash2 size={12} />}
      variant="icon"
      className="danger-button"
      disabled={busy}
      onClick={() => handleDeleteCriterion(criterionId)}
      aria-label={text("Delete criterion")}
    />
  );

  // A quantitative criterion: the expression is shown inline as a column from the
  // M tier up and folds behind the row's expansion at S (density contract, ADR
  // 0076 D6). Expanding always reveals the expression (so it stays reachable when
  // the inline column is hidden) plus the measured value / threshold / note.
  function renderQuantitativeCriterion(criterion: QualityFramework["criteria"][number]) {
    const result = resultByCriterion.get(criterion.id);
    const measured = result ? formatMeasured(result, locale) : null;
    const expanded = expandedCriterionId === criterion.id;
    return (
      <li key={criterion.id} className="quality-quantitative-row">
        <ExpandableRow
          label={`${criterion.label}${result ? ` — ${verdictLabel(result.verdict)}` : ""}`}
          isExpanded={expanded}
          onToggle={() => setExpandedCriterionId(expanded ? null : criterion.id)}
          detail={
            <div className="quality-quantitative-detail">
              <p className="quality-detail-line">
                <span className="quality-detail-label">{text("Expression")}</span>{" "}
                <code>{criterion.expression}</code>
              </p>
              {measured || result?.threshold ? (
                <p className="quality-detail-line">
                  {measured ? (
                    <span className="quality-measured">{`${text("Measured")}: ${measured}`}</span>
                  ) : null}
                  {result?.threshold ? (
                    <span className="quality-measured">{`${text("Threshold")}: ${result.threshold}`}</span>
                  ) : null}
                </p>
              ) : null}
              {result?.note ? <Hint>{result.note}</Hint> : null}
            </div>
          }
        >
          <span className="quality-criterion-header">
            <span className="quality-criterion-label">{criterion.label}</span>
            <span className="quality-expression" title={criterion.expression}>
              {criterion.expression}
            </span>
            <span className="quality-criterion-trailing">
              {measured ? <span className="quality-measured">{measured}</span> : null}
              {result ? (
                <StatusChip tone={verdictTone(result.verdict)}>{verdictLabel(result.verdict)}</StatusChip>
              ) : null}
              {deleteCriterionButton(criterion.id)}
            </span>
          </span>
        </ExpandableRow>
      </li>
    );
  }

  // A qualitative criterion (ADR 0075): the agent's current-state assessment,
  // visually distinct from measured quantitative rows — labelled "agent-assessed",
  // regeneratable (re-run), never mutating quantitative data. Expands to reveal
  // the reasoning and its evidence citations.
  function renderQualitativeCriterion(criterion: QualityFramework["criteria"][number]) {
    const result = qualResultByCriterion.get(criterion.id);
    const expanded = expandedCriterionId === criterion.id;
    // Parse citations only when the row is expanded (collapsed rows never show
    // them), so a re-render of N criteria doesn't JSON.parse N blobs each time.
    const citations = expanded ? parseCitations(result?.citations ?? null) : [];
    const trailing = (
      <span className="quality-criterion-trailing">
        {/* "Agent-assessed" is a STATE, not a kind label — showing it on a
            never-assessed criterion reads as a finished assessment. */}
        {result ? (
          <>
            <StatusChip tone="accent">{text("Agent-assessed")}</StatusChip>
            <StatusChip tone={verdictTone(result.verdict)}>{verdictLabel(result.verdict)}</StatusChip>
          </>
        ) : (
          <StatusChip tone="neutral">{text("Not assessed yet")}</StatusChip>
        )}
        {result?.confidence ? (
          <StatusChip tone="neutral">{`${text("Confidence")}: ${confidenceLabel(result.confidence)}`}</StatusChip>
        ) : null}
        <Button
          icon={<RotateCcw size={12} />}
          variant="icon"
          disabled={busy}
          onClick={() => handleRerunCriterion(criterion.id)}
          // A never-assessed criterion's action is its first assessment, not a re-run.
          aria-label={result ? text("Re-run assessment") : text("Assess this criterion")}
        />
        {deleteCriterionButton(criterion.id)}
      </span>
    );

    // Not assessed yet → no verdict chip (distinct from an
    // `insufficient_evidence` verdict). The owner-authored guidance is prose —
    // it wraps in the expandable detail block, never in a single-line row slot
    // (panel-overflow guardrail, ADR 0045).
    if (!result) {
      return (
        <li key={criterion.id} className="quality-qualitative-row">
          <ExpandableRow
            label={`${criterion.label} — ${text("Not assessed yet")}`}
            isExpanded={expanded}
            onToggle={() => setExpandedCriterionId(expanded ? null : criterion.id)}
            detail={
              <div className="quality-qualitative-detail">
                <p className="quality-reasoning">
                  {criterion.assessmentGuidance ??
                    text("Not assessed yet. Click Assess to evaluate this criterion.")}
                </p>
              </div>
            }
          >
            <span className="quality-history-header">
              <span className="quality-history-when">{criterion.label}</span>
              {trailing}
            </span>
          </ExpandableRow>
        </li>
      );
    }

    return (
      <li key={criterion.id} className="quality-qualitative-row">
      <ExpandableRow
        label={`${criterion.label} — ${verdictLabel(result.verdict)}`}
        isExpanded={expanded}
        onToggle={() => setExpandedCriterionId(expanded ? null : criterion.id)}
        detail={
          <div className="quality-qualitative-detail">
            {result.reasoning ? <p className="quality-reasoning">{result.reasoning}</p> : null}
            {citations.length > 0 ? (
              <div className="quality-citations">
                <SectionHeader level="h4" title={text("Citations")} meta={citations.length} />
                <ul className="ui-list-rows">
                  {citations.map((citation, index) => (
                    <ListRow
                      key={`${citation.evidenceType}:${citation.evidenceId}:${index}`}
                      icon={<Bot size={14} />}
                      title={citation.label ?? citation.evidenceId}
                      titleAttr={citation.label ?? citation.evidenceId}
                      meta={citation.snippet ?? citation.evidenceType}
                    />
                  ))}
                </ul>
              </div>
            ) : (
              <Hint>{text("No citations for this assessment.")}</Hint>
            )}
            {result.promptVersion ? (
              <Hint>{`${text("Prompt")}: ${result.promptVersion}`}</Hint>
            ) : null}
          </div>
        }
      >
        <span className="quality-history-header">
          <span className="quality-history-when">{criterion.label}</span>
          {trailing}
        </span>
      </ExpandableRow>
      </li>
    );
  }

  return (
    <section ref={panelRef} className="company-tab-panel quality-panel" aria-label={text("Quality frameworks")}>
      <SectionHeader
        paneLead
        title={text("Quality")}
        description={text("Evaluate this company against your quality frameworks. Decision support only.")}
        actions={
          <ActionRow ariaLabel={text("Framework actions")}>
            <Button icon={<Plus size={14} />} onClick={() => setNameModal({ mode: "new", value: "" })}>
              {text("New")}
            </Button>
            <Button
              icon={<Copy size={14} />}
              disabled={!selected}
              onClick={() => setNameModal({ mode: "clone", value: "" })}
            >
              {text("Clone")}
            </Button>
            {selected?.origin === "app_template" ? (
              <Button icon={<RotateCcw size={14} />} disabled={busy} onClick={handleReset}>
                {text("Reset")}
              </Button>
            ) : null}
            <Button
              icon={<Trash2 size={14} />}
              variant="icon"
              className="danger-button"
              disabled={!selected || busy}
              onClick={handleDeleteFramework}
              aria-label={text("Delete framework")}
            />
          </ActionRow>
        }
      />

      {error ? <ErrorText>{error}</ErrorText> : null}

      {frameworks.length === 0 ? (
        <EmptyState>{text("No quality frameworks yet. Create one or clone a template.")}</EmptyState>
      ) : (
        <div className="quality-body">
          <div className="quality-main">
          <ActionRow ariaLabel={text("Selected framework")} className="quality-framework-bar">
            <SelectField
              label={text("Framework")}
              value={selectedId ?? ""}
              onChange={(event) => setSelectedId(event.target.value)}
            >
              {frameworks.map((framework) => (
                <option key={framework.id} value={framework.id}>
                  {framework.name}
                  {framework.origin === "app_template" ? ` · ${text("Template")}` : ""}
                </option>
              ))}
            </SelectField>
            <Button
              icon={<CheckCircle2 size={14} />}
              variant="primary"
              disabled={busy || !selected}
              onClick={handleEvaluate}
            >
              {text("Evaluate")}
            </Button>
            {hasQualitative ? (
              <Button
                icon={<Sparkles size={14} />}
                disabled={busy || !selected}
                onClick={handleRunAssessment}
              >
                {text("Assess")}
              </Button>
            ) : null}
          </ActionRow>

          {assessmentQueued ? (
            <Hint>
              {text("Assessment queued. Agent results appear here when the job completes.")}
            </Hint>
          ) : null}

          {latest ? (
            <ActionRow ariaLabel={text("Scorecard summary")} className="quality-scorecard-summary">
              <StatusChip tone="ok">{`${latest.passCount} ${text("pass")}`}</StatusChip>
              <StatusChip tone="warn">{`${latest.partialCount} ${text("partial")}`}</StatusChip>
              <StatusChip tone="danger">{`${latest.failCount} ${text("fail")}`}</StatusChip>
              <StatusChip tone="neutral">{`${latest.unavailableCount} ${text("no data")}`}</StatusChip>
            </ActionRow>
          ) : (
            <Hint>{text("Not evaluated yet. Click Evaluate to score this company.")}</Hint>
          )}

          {selected && selected.criteria.length === 0 ? (
            <EmptyState>{text("This framework has no criteria yet. Add one below.")}</EmptyState>
          ) : (
            <ul className="ui-list-rows">
              {selected?.criteria.map((criterion) =>
                criterion.kind === "qualitative"
                  ? renderQualitativeCriterion(criterion)
                  : renderQuantitativeCriterion(criterion),
              )}
            </ul>
          )}

          <SectionHeader level="h4" title={text("Add criterion")} />
          <SegmentedControl ariaLabel={text("Criterion type")} className="quality-kind-switch">
            <SegmentedControlOption
              active={criterionKind === "quantitative"}
              onClick={() => setCriterionKind("quantitative")}
            >
              {text("Quantitative")}
            </SegmentedControlOption>
            <SegmentedControlOption
              active={criterionKind === "qualitative"}
              onClick={() => setCriterionKind("qualitative")}
            >
              {text("Qualitative")}
            </SegmentedControlOption>
          </SegmentedControl>
          {criterionKind === "quantitative" ? (
            <>
              <ActionRow ariaLabel={text("Add criterion")} className="quality-add-criterion">
                <TextField
                  label={text("Label")}
                  value={label}
                  onChange={(event) => setLabel(event.target.value)}
                  placeholder={text("Strong return on equity")}
                />
                <TextField
                  label={text("Expression")}
                  value={expression}
                  onChange={(event) => onExpressionChange(event.target.value)}
                  placeholder="roe >= 15%"
                />
                <SelectField
                  label={text("Insert metric")}
                  value=""
                  onChange={(event) => {
                    insertMetric(event.target.value);
                    event.target.value = "";
                  }}
                >
                  <option value="">{text("Pick a metric…")}</option>
                  {metricKeys.map((metric) => (
                    <option key={metric.key} value={metric.key}>
                      {metric.label} ({metric.key})
                      {metric.unit ? ` · ${metric.unit}` : ""}
                    </option>
                  ))}
                </SelectField>
                <Button
                  variant="primary"
                  disabled={busy || label.trim() === "" || expression.trim() === "" || exprError != null}
                  onClick={handleAddCriterion}
                >
                  {text("Add")}
                </Button>
              </ActionRow>
              {exprError ? <ErrorText>{exprError}</ErrorText> : null}
              {exprMetrics.length > 0 && !exprError ? (
                <Hint>{`${text("Uses metrics")}: ${exprMetrics.join(", ")}`}</Hint>
              ) : null}
            </>
          ) : (
            <>
              <ActionRow ariaLabel={text("Add qualitative criterion")} className="quality-add-criterion">
                <TextField
                  label={text("Label")}
                  value={label}
                  onChange={(event) => setLabel(event.target.value)}
                  placeholder={text("Wide, durable moat")}
                />
                <Button
                  variant="primary"
                  disabled={busy || label.trim() === "" || guidance.trim() === ""}
                  onClick={handleAddCriterion}
                >
                  {text("Add")}
                </Button>
              </ActionRow>
              <TextareaField
                label={text("Assessment guidance")}
                value={guidance}
                rows={3}
                onChange={(event) => setGuidance(event.target.value)}
                placeholder={text("Describe what the agent should assess and what strong evidence looks like.")}
              />
              <Hint>
                {text("The agent judges this from app-held evidence only (reports, notes, claims, signals) and cites it. Decision support, not advice.")}
              </Hint>
            </>
          )}

          </div>
          {history.length > 0 ? (
            <aside className="quality-history" aria-label={text("Evaluation history")}>
              <button
                type="button"
                className="quality-history-toggle"
                aria-expanded={historyOpen}
                onClick={() => setHistoryManual(!historyOpen)}
              >
                <span aria-hidden="true" className="quality-history-chevron">
                  {historyOpen ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
                </span>
                <span className="quality-history-toggle-title">{text("Evaluation history")}</span>
                <span className="quality-history-toggle-count">{history.length}</span>
              </button>
              {historyOpen ? (
              <div className="ui-list-rows quality-history-list" ref={historyListRef}>
                {history.map((evaluation) => {
                  const total =
                    evaluation.passCount +
                    evaluation.partialCount +
                    evaluation.failCount +
                    evaluation.unavailableCount;
                  const expanded = expandedRunId === evaluation.id;
                  return (
                    <ExpandableRow
                      key={evaluation.id}
                      className="quality-history-row"
                      label={`${evaluation.createdAt} — ${evaluation.passCount}/${total} ${text("pass")}`}
                      isExpanded={expanded}
                      onToggle={() => setExpandedRunId(expanded ? null : evaluation.id)}
                      detail={
                        <ul className="ui-list-rows quality-history-detail">
                          {evaluation.results.map((result) => {
                            const measured = formatMeasured(result, locale);
                            return (
                              <ListRow
                                key={result.id}
                                title={result.label}
                                titleAttr={result.label}
                                meta={result.expression}
                                trailing={
                                  <span className="quality-criterion-trailing">
                                    {measured ? (
                                      <span className="quality-measured">{measured}</span>
                                    ) : null}
                                    <StatusChip tone={verdictTone(result.verdict)}>
                                      {verdictLabel(result.verdict)}
                                    </StatusChip>
                                  </span>
                                }
                              />
                            );
                          })}
                        </ul>
                      }
                    >
                      <span className="quality-history-header">
                        <span className="quality-history-when">{evaluation.createdAt}</span>
                        <span className="quality-criterion-trailing">
                          <span className="quality-measured">{`${evaluation.passCount}/${total} ${text("pass")}`}</span>
                          <Button
                            icon={<Trash2 size={12} />}
                            variant="icon"
                            className="danger-button"
                            disabled={busy}
                            onClick={(event) => {
                              event.stopPropagation();
                              handleDeleteEvaluation(evaluation.id);
                            }}
                            aria-label={text("Delete evaluation")}
                          />
                        </span>
                      </span>
                    </ExpandableRow>
                  );
                })}
              </div>
              ) : null}
            </aside>
          ) : null}
        </div>
      )}

      <Modal
        open={nameModal != null}
        onClose={() => setNameModal(null)}
        title={nameModal?.mode === "clone" ? text("Clone framework") : text("New framework")}
        ariaLabel={nameModal?.mode === "clone" ? text("Clone framework") : text("New framework")}
        footer={
          <ActionRow ariaLabel={text("Confirm")}>
            <Button onClick={() => setNameModal(null)}>{text("Cancel")}</Button>
            <Button variant="primary" onClick={handleSubmitName}>
              {nameModal?.mode === "clone" ? text("Clone") : text("Create")}
            </Button>
          </ActionRow>
        }
      >
        <TextField
          label={text("Name")}
          value={nameModal?.value ?? ""}
          onChange={(event) =>
            setNameModal((current) => (current ? { ...current, value: event.target.value } : current))
          }
          placeholder={text("My quality checklist")}
        />
      </Modal>
    </section>
  );
}
