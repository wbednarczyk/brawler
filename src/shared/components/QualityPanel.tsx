import { useCallback, useEffect, useMemo, useState } from "react";
import { CheckCircle2, Copy, Plus, RotateCcw, Trash2 } from "lucide-react";

import {
  cloneFramework,
  createFrameworkCriterion,
  createQualityFramework,
  deleteFrameworkCriterion,
  deleteFrameworkEvaluation,
  deleteQualityFramework,
  evaluateFramework,
  listAvailableMetricKeys,
  listFrameworkEvaluations,
  listQualityFrameworks,
  resetFrameworkToTemplate,
  validateCriterionExpression,
} from "../../api/qualityFrameworks";
import type {
  CriterionVerdict,
  FrameworkEvaluation,
  MetricKeyInfo,
  QualityFramework,
} from "../../api/qualityFrameworksTypes";
import { useLocale } from "../locale";
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
  SelectField,
  StatusChip,
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
      return "neutral";
  }
}

/// The Quality tab (ADR 0046): manage user frameworks of DSL criteria and run a
/// deterministic, immutable-snapshot scorecard against the company's confirmed
/// fundamentals facts. Decision support only — no buy/sell output.
export function QualityPanel({ companyId }: QualityPanelProps) {
  const { text } = useLocale();
  const [frameworks, setFrameworks] = useState<QualityFramework[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [latest, setLatest] = useState<FrameworkEvaluation | null>(null);
  const [history, setHistory] = useState<FrameworkEvaluation[]>([]);
  const [expandedRunId, setExpandedRunId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Add-criterion form.
  const [label, setLabel] = useState("");
  const [expression, setExpression] = useState("");
  const [exprError, setExprError] = useState<string | null>(null);
  const [exprMetrics, setExprMetrics] = useState<string[]>([]);
  const [metricKeys, setMetricKeys] = useState<MetricKeyInfo[]>([]);

  // Name-a-framework modal (shared by New and Clone).
  const [nameModal, setNameModal] = useState<null | { mode: "new" | "clone"; value: string }>(null);

  const selected = useMemo(
    () => frameworks.find((framework) => framework.id === selectedId) ?? null,
    [frameworks, selectedId],
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
        default:
          return text("No data");
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
        setLatest(rows[0] ?? null);
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
    if (!selectedId || label.trim() === "" || expression.trim() === "") return;
    void runGuarded(async () => {
      await createFrameworkCriterion({
        frameworkId: selectedId,
        label: label.trim(),
        expression: expression.trim(),
      });
      setLabel("");
      setExpression("");
      setExprMetrics([]);
      await reloadFrameworks(selectedId);
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
      setLatest(rows[0] ?? null);
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

  return (
    <section className="company-tab-panel quality-panel" aria-label={text("Quality frameworks")}>
      <SectionHeader
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
        <>
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
          </ActionRow>

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
              {selected?.criteria.map((criterion) => {
                const result = resultByCriterion.get(criterion.id);
                const measured =
                  result?.measuredValue != null
                    ? `${result.measuredValue}${result.measuredUnit ? ` ${result.measuredUnit}` : ""}`
                    : null;
                return (
                  <ListRow
                    key={criterion.id}
                    title={criterion.label}
                    titleAttr={criterion.label}
                    meta={criterion.expression}
                    trailing={
                      <span className="quality-criterion-trailing">
                        {measured ? <span className="quality-measured">{measured}</span> : null}
                        {result ? (
                          <StatusChip tone={verdictTone(result.verdict)}>
                            {verdictLabel(result.verdict)}
                          </StatusChip>
                        ) : null}
                        <Button
                          icon={<Trash2 size={12} />}
                          variant="icon"
                          className="danger-button"
                          disabled={busy}
                          onClick={() => handleDeleteCriterion(criterion.id)}
                          aria-label={text("Delete criterion")}
                        />
                      </span>
                    }
                  />
                );
              })}
            </ul>
          )}

          <SectionHeader level="h4" title={text("Add criterion")} />
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

          {history.length > 0 ? (
            <>
              <SectionHeader level="h4" title={text("Evaluation history")} meta={history.length} />
              <div className="ui-list-rows">
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
                            const measured =
                              result.measuredValue != null
                                ? `${result.measuredValue}${
                                    result.measuredUnit ? ` ${result.measuredUnit}` : ""
                                  }`
                                : null;
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
            </>
          ) : null}
        </>
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
