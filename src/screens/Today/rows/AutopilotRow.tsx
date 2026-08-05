import { useState, type ReactNode } from "react";
import { Sparkles } from "lucide-react";

import type { AutopilotRun } from "../../../api/autopilot";
import { FACT_FORMS, pluralNoun } from "../../../shared/locale/plural";
import { splitDocumentTitle } from "../documentTitle";
import { DriftDiff, parseDrift, type ParsedDrift } from "../../../shared/components/DriftDiff";
import { Button, InlineConfirm, StatusChip } from "../../../ui";
import {
  composeAutopilotRunSummary,
  isTokenizedSummary,
  renderAutopilotSummaryTokens,
} from "../autopilotRunSummary";
import type { Severity } from "../streamModel";
import type { RowContext } from "./rowContext";
import { StreamRow, type RowDescriptor } from "./streamRowKit";

/**
 * Parses an autopilot run's opaque `kpiDeltaJson` envelope defensively for the
 * "structure changed" drift a structured-extraction attempt may have carried
 * (ADR 0061 wave 2). A legacy run, a non-drifted delta, or malformed JSON all
 * yield `null` — no crash, no drift section.
 */
export function autopilotRunDrift(kpiDeltaJson: string | null): ParsedDrift | null {
  if (!kpiDeltaJson) return null;
  try {
    const parsed = JSON.parse(kpiDeltaJson) as { structureChanged?: unknown; driftJson?: unknown };
    if (parsed.structureChanged !== true) return null;
    return parseDrift(typeof parsed.driftJson === "string" ? parsed.driftJson : null);
  } catch {
    return null;
  }
}

/** The run's localized "what changed" summary (typed tokens, legacy prose tolerated). */
function autopilotSummary(run: AutopilotRun, ctx: RowContext): string {
  return isTokenizedSummary(run.summaryText)
    ? renderAutopilotSummaryTokens(run.summaryText, ctx.text, ctx.locale)
    : composeAutopilotRunSummary(run, ctx.text, ctx.locale);
}

/** The autopilot row descriptor (shared by the single row and a grouped member). */
export function autopilotDescriptor(run: AutopilotRun, ctx: RowContext): RowDescriptor {
  const company = ctx.companyById.get(run.companyId);
  const failed = run.status === "failed" || run.status === "partial";
  const revertedCount = ctx.autopilot.undoneAutopilotRuns[run.id];
  const extraChips = (
    <>
      {failed ? (
        <StatusChip tone="danger">
          {run.status === "partial" ? ctx.text("Partial") : ctx.text("Failed")}
        </StatusChip>
      ) : null}
      {revertedCount !== undefined ? (
        <StatusChip tone="ok">
          {ctx.text("Reverted")} {revertedCount} {pluralNoun(ctx.locale, revertedCount, FACT_FORMS)}
        </StatusChip>
      ) : null}
    </>
  );
  // The statement is WHICH report the run processed (v0.60 D6): the document
  // title leads, the token-composed "what changed" summary drops to the
  // sub-line. A run whose document identity is unknown
  // (legacy row / pruned document) keeps the summary as the statement — the bare
  // "Przetworzono nowy raport." never appears alone once the report is named.
  //
  // D7: a stored title is sometimes a filename glued onto the human title, or a
  // raw filename alone. The filename is metadata → it splits onto a quiet
  // secondary link line; the human part (or the generic summary, when the title
  // is filename-only) stays the statement.
  const summary = autopilotSummary(run, ctx);
  const { statement, filename } = splitDocumentTitle(run.reportDocumentTitle);
  const openWorkspace = () =>
    company ? ctx.openCompanyWorkspace(company.id, "Fundamentals") : ctx.openInbox();
  return {
    key: run.id,
    icon: <Sparkles aria-hidden="true" size={15} />,
    ticker: company?.qualifiedTicker ?? "",
    typeBadge: <StatusChip tone="accent">{ctx.text("Autopilot")}</StatusChip>,
    extraChips,
    date: run.createdAt,
    title: statement ?? summary,
    subtitle: statement ? summary : null,
    // The filename click-through reuses the run's review target (the company's
    // Fundamentals surface, where the processed report and its facts live). NOTE:
    // no per-document open/reveal affordance exists — the AutopilotRun payload
    // carries no document URL — so this points at the report surface, not the raw
    // file. Opening the exact stored file would need new plumbing (not invented).
    documentLink: filename ? { filename, onOpen: openWorkspace } : null,
    onReview: openWorkspace,
  };
}

/**
 * The autopilot run's in-place detail (ADR 0055 §4): Undo (two-step), Dismiss,
 * and the Structure-changed drift block. Local state owns the undo confirm step.
 */
export function AutopilotDetail({ run, ctx }: { run: AutopilotRun; ctx: RowContext }): ReactNode {
  const [confirmingUndo, setConfirmingUndo] = useState(false);
  // Facts are review-free in both modes (ADR 0086 dec. 5): any run that produced
  // facts is undo-eligible — eligibility is "produced facts", not a mode gate.
  const undoEligible = run.producedFactIds.length > 0;
  const reverted = ctx.autopilot.undoneAutopilotRuns[run.id] !== undefined;
  const drift = autopilotRunDrift(run.kpiDeltaJson);
  return (
    <div className="today-run-detail">
      <div className="today-run-detail-actions">
        {undoEligible && !confirmingUndo && !reverted ? (
          <Button onClick={() => setConfirmingUndo(true)} type="button" variant="ghost">
            {ctx.text("Undo")}
          </Button>
        ) : null}
        <Button onClick={() => ctx.autopilot.dismissAutopilotRun(run.id)} type="button" variant="ghost">
          {ctx.text("Dismiss")}
        </Button>
      </div>
      {confirmingUndo ? (
        <InlineConfirm
          cancelLabel={ctx.text("Cancel")}
          confirmLabel={ctx.text("Undo")}
          onCancel={() => setConfirmingUndo(false)}
          onConfirm={() => {
            setConfirmingUndo(false);
            void ctx.autopilot.undoAutopilotRun(run.id);
          }}
        >
          {ctx.text("Undo this run and revert its facts?")}
        </InlineConfirm>
      ) : null}
      {drift ? (
        <section className="today-run-drift" aria-label={ctx.text("Structure changed")}>
          <span className="eyebrow">{ctx.text("Structure changed")}</span>
          <DriftDiff drift={drift} />
        </section>
      ) : null}
    </div>
  );
}

/** A single (ungrouped) autopilot stream row. */
export function AutopilotRow({
  run,
  severity,
  ctx,
  variant = "row",
}: {
  run: AutopilotRun;
  severity: Severity;
  ctx: RowContext;
  variant?: "row" | "member";
}) {
  return (
    <StreamRow
      category="autopilot"
      severity={severity}
      descriptor={autopilotDescriptor(run, ctx)}
      expandable={<AutopilotDetail run={run} ctx={ctx} />}
      variant={variant}
    />
  );
}
