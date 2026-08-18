import { useEffect, useRef, useState } from "react";
import { Check, History, RefreshCw, Sparkles } from "lucide-react";
import { getFundamentalsCoverage } from "../../api/fundamentalsCoverage";
import type {
  CoveragePeriodRow,
  CoverageReportCell,
} from "../../api/fundamentalsCoverageTypes";
import { backfillCompanyHistory, getBackfillProgress } from "../../api/sources";
import type { BackfillProgress } from "../../api/types";
import { getHistorySweepProgress, runHistorySweep } from "../../api/historySweep";
import type { HistorySweepProgress } from "../../api/historySweep";
import {
  getPipelineReextractionProgress,
  runPipelineReextraction,
} from "../../api/pipelineReextraction";
import type { PipelineReextractionProgress } from "../../api/pipelineReextraction";
import { getCompanyAutopilot } from "../../api/autopilot";
import { useLocale } from "../locale";
import { pluralNoun, FACT_FORMS, type PluralForms } from "../locale/plural";
import { Button, EmptyState, ErrorText, Hint, SectionHeader, StatusChip } from "../../ui";
import { middleTruncate } from "./CompanyReportDocumentsPanel";
import { CoverageFlaggedPeriods } from "./CoverageFlaggedPeriods";
import { CoverageFlaggedFacts } from "./CoverageFlaggedFacts";
import { CoverageRawCapture } from "./CoverageRawCapture";

// Facts the pipeline flagged (drift/contradiction) — an informational origin
// label, not a review to-do (ADR 0086 dec. 5). Three Polish forms.
const FLAGGED_FORMS: PluralForms = {
  en: ["flagged fact", "flagged facts"],
  pl: ["oflagowany fakt", "oflagowane fakty", "oflagowanych faktów"],
};

// The sweep poll runs at a 1.5s cadence; cap it at a generous safety horizon
// (~10 min = 400 ticks) so a sweep stuck behind a long job-queue backlog
// surfaces an honest error instead of spinning forever or silently settling.
const SWEEP_POLL_INTERVAL_MS = 1500;
const MAX_SWEEP_POLL_TICKS = 400;

type CompanyCoveragePanelProps = {
  companyId: string;
  // Bump to force a reload (e.g. after a backfill or extraction elsewhere).
  reloadKey?: number;
  // Opens the company's report-documents pane. The whole period row is a click
  // target for it (ADR 0077 §2 mockup): reviewing a period starts from its docs.
  onOpenDocuments?: () => void;
  // Called after a backfill or history sweep finishes so the workspace can reload
  // sibling views (e.g. the Fundamentals map). Full progress UI is T3.3; here the
  // footer just triggers the refresh (ADR 0077 §3, T3.2).
  onHistoryRefreshed?: () => void;
};

// Fiscal-period label: `annual` renders as the accented FY marker (matching the
// coverage-map mockup); every other stored/report period type upper-cases (Q1,
// H1, Q3, FY). The union carries report-side (`FY`) and fact-side (`annual`)
// spellings for the same period, so both normalize here.
function periodTypeLabel(periodType: string): string {
  return periodType.toLowerCase() === "annual" ? "FY" : periodType.toUpperCase();
}

function isFullYear(periodType: string): boolean {
  return periodTypeLabel(periodType) === "FY";
}

/// The fundamentals coverage map for one company (ADR 0077 §2): one row per fiscal
/// period, unioned across the canonical report, the extracted facts, and the review
/// queue. Every gap (a period with no report, a report never processed, an
/// AI-budget skip) stays visible instead of silently missing. Clicking a row opens
/// the company's report documents. The footer (ADR 0077 §3, T3.2) drives the two
/// history actions — "Backfill history" (fetch + auto-chained sweep) and "Extract
/// missing periods" (sweep only, documents already fetched) — with a lean status
/// line, a live drain counter while a sweep runs (T3.3), and the latest sweep's
/// AI-call spend alongside it whenever a sweep row exists (T5.3).
export function CompanyCoveragePanel({
  companyId,
  reloadKey = 0,
  onOpenDocuments,
  onHistoryRefreshed,
}: CompanyCoveragePanelProps) {
  const { text, locale } = useLocale();
  const [periods, setPeriods] = useState<CoveragePeriodRow[]>([]);
  const [error, setError] = useState<string | null>(null);
  // History-action state (footer). `mode === "off"` disables both actions.
  const [mode, setMode] = useState<string>("off");
  const [backfillProgress, setBackfillProgress] = useState<BackfillProgress | null>(null);
  const [sweepProgress, setSweepProgress] = useState<HistorySweepProgress | null>(null);
  const [reextractProgress, setReextractProgress] = useState<PipelineReextractionProgress | null>(
    null,
  );
  const [backfilling, setBackfilling] = useState(false);
  const [sweeping, setSweeping] = useState(false);
  const [reextracting, setReextracting] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  // Local reload tick: a history sweep or a flagged-period re-run writes facts,
  // so the coverage table AND the flagged list must both refetch — without
  // depending on the host remembering to bump `reloadKey`.
  const [localReloadTick, setLocalReloadTick] = useState(0);
  // Shared by all three history-footer actions — they are mutually exclusive
  // (`busy` disables the others while one runs), so one interval ref/stop
  // function is enough.
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Every path that lands new facts refreshes both this panel's own reads and
  // the host's siblings (Fundamentals map etc.).
  const refreshAfterExtraction = () => {
    setLocalReloadTick((tick) => tick + 1);
    onHistoryRefreshed?.();
  };

  const stopPolling = () => {
    if (pollRef.current !== null) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
  };

  useEffect(() => {
    let cancelled = false;
    getFundamentalsCoverage(companyId)
      .then((coverage) => {
        if (!cancelled) setPeriods(coverage.periods);
      })
      .catch((reason) => {
        if (!cancelled) setError(reason instanceof Error ? reason.message : String(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [companyId, reloadKey, localReloadTick]);

  // Load the automation mode + any prior backfill/sweep status on company change,
  // so the footer starts from the real state (not a blank one). In-flight action
  // flags belong to the previous company — reset them alongside the poll.
  useEffect(() => {
    let cancelled = false;
    setBackfilling(false);
    setSweeping(false);
    setReextracting(false);
    getCompanyAutopilot(companyId)
      .then((autopilot) => {
        if (!cancelled) setMode(autopilot.mode);
      })
      .catch(() => {});
    getBackfillProgress(companyId)
      .then((value) => {
        if (!cancelled) setBackfillProgress(value);
      })
      .catch(() => {});
    getHistorySweepProgress(companyId)
      .then((value) => {
        if (!cancelled) setSweepProgress(value);
      })
      .catch(() => {});
    getPipelineReextractionProgress(companyId)
      .then((value) => {
        if (!cancelled) setReextractProgress(value);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      stopPolling();
    };
  }, [companyId]);

  const modeOff = mode === "off";
  const busy = backfilling || sweeping || reextracting;

  // Poll the sweep until it settles: the sweep row is a DURABLE JOB — the start
  // command returns with the sweep merely queued, the job completes seconds
  // later, and the enqueued extraction runs drain after that. A single refresh
  // at start left the status stuck on "Queued" forever while the backend
  // finished everything (caught on the owner's real app, T3.2 live-drive).
  // Settled = terminal sweep status AND every enqueued run reached a terminal
  // state; only then do facts exist, so that is when siblings reload.
  //
  // The poll target is EXPLICIT (`expectedSweepId`): the caller names the sweep
  // it started (the backfill's eagerly-chained sweep id, or the manual sweep
  // command's return). When back-to-back backfills queue many sweeps, the one we
  // started may sit behind earlier work, so `get_history_sweep_progress` can
  // briefly report an OLDER (already-terminal) sweep as "the latest" — polling
  // "latest" would then false-settle on the wrong sweep and freeze a stale
  // "AI: 0/N" (owner live-validation, MDV). With an explicit id we ignore every
  // other sweep, never give up while ours is absent, and settle ONLY on ours —
  // so `sweepProgress` (and the AI-budget footer) reflect the sweep we started.
  // A generous horizon (~10 min) then surfaces an honest error rather than
  // spinning forever. The legacy no-id path keeps the bounded give-up for a
  // caller that cannot name its sweep (e.g. a backfill whose chain failed).
  const pollSweepUntilSettled = (expectedSweepId?: string) => {
    stopPolling();
    let ticksWithoutSweep = 0;
    let totalTicks = 0;
    const settle = (value: HistorySweepProgress) => {
      const sweep = value.sweep;
      const terminal = sweep?.status === "completed" || sweep?.status === "failed";
      const drained = value.runsDone >= value.runsTotal;
      if (terminal && drained) {
        stopPolling();
        setSweeping(false);
        refreshAfterExtraction();
        return true;
      }
      return false;
    };
    const tick = async () => {
      totalTicks += 1;
      if (totalTicks > MAX_SWEEP_POLL_TICKS) {
        // Safety horizon: never spin forever or silently settle — say so.
        stopPolling();
        setSweeping(false);
        setActionError(text("History sweep is taking longer than expected — reopen this company to check its status."));
        return;
      }
      let value: HistorySweepProgress | null;
      try {
        value = await getHistorySweepProgress(companyId);
      } catch {
        return; // transient read failure — keep polling
      }
      const sweep = value?.sweep;
      if (expectedSweepId !== undefined) {
        // Ignore any sweep that is not the one we started (absent, or a
        // stale/other sweep reported as "latest"): keep polling — no give-up —
        // until ours surfaces and settles, or the horizon above trips.
        if (!sweep || sweep.id !== expectedSweepId) return;
        setSweepProgress(value);
        settle(value);
        return;
      }
      // Legacy no-id path: keep the bounded give-up so a sweep row that never
      // appears (a failed chain) does not poll forever.
      setSweepProgress(value);
      if (!sweep) {
        ticksWithoutSweep += 1;
        if (ticksWithoutSweep >= 4) {
          stopPolling();
          setSweeping(false);
        }
        return;
      }
      settle(value);
    };
    void tick(); // immediate first check — a fast (mock/test) sweep settles at once
    pollRef.current = setInterval(() => void tick(), SWEEP_POLL_INTERVAL_MS);
  };

  // "Backfill history": fetch the last few years of reports; the backend auto-chains
  // a history sweep at the end (ADR 0077 §3), so we hand off to the sweep poll —
  // and reload siblings once — when the backfill resolves.
  const runBackfill = async () => {
    if (busy) return;
    setBackfilling(true);
    setActionError(null);
    stopPolling();
    pollRef.current = setInterval(() => {
      getBackfillProgress(companyId)
        .then((value) => {
          if (value) setBackfillProgress(value);
        })
        .catch(() => {});
    }, 1500);
    try {
      const result = await backfillCompanyHistory(companyId);
      setBackfillProgress(result);
      refreshAfterExtraction(); // new documents are visible before extraction lands
      setSweeping(true); // the chained sweep now drives the status line
      // Poll the sweep the backfill just chained, by its eager id. If the chain
      // failed (no id), fall back to the legacy give-up path.
      pollSweepUntilSettled(result.chainedSweepId ?? undefined);
    } catch (reason) {
      setActionError(reason instanceof Error ? reason.message : String(reason));
      stopPolling();
    } finally {
      setBackfilling(false);
    }
  };

  // "Extract missing periods": run a sweep only (the CBF case where documents are
  // already fetched and only extraction is missing — no re-download).
  const runSweep = async () => {
    if (busy) return;
    setSweeping(true);
    setActionError(null);
    try {
      const started = await runHistorySweep(companyId);
      pollSweepUntilSettled(started.id);
    } catch (reason) {
      setActionError(reason instanceof Error ? reason.message : String(reason));
      setSweeping(false);
    }
  };

  // Poll a re-extraction batch until it settles (terminal status + every
  // re-armed run reached a terminal state) — the `pollSweepUntilSettled`
  // pattern, over the leaner batch shape (no `trigger`/`skippedExisting`).
  const pollReextractionUntilSettled = (expectedBatchId: string) => {
    stopPolling();
    let totalTicks = 0;
    const tick = async () => {
      totalTicks += 1;
      if (totalTicks > MAX_SWEEP_POLL_TICKS) {
        stopPolling();
        setReextracting(false);
        setActionError(
          text("Re-extraction is taking longer than expected — reopen this company to check its status."),
        );
        return;
      }
      let value: PipelineReextractionProgress | null;
      try {
        value = await getPipelineReextractionProgress(companyId);
      } catch {
        return; // transient read failure — keep polling
      }
      const batch = value?.batch;
      if (!batch || batch.id !== expectedBatchId) return; // not ours yet
      setReextractProgress(value);
      const terminal = batch.status === "completed" || batch.status === "failed";
      const drained = value.runsDone >= value.runsTotal;
      if (terminal && drained) {
        stopPolling();
        setReextracting(false);
        refreshAfterExtraction();
      }
    };
    void tick();
    pollRef.current = setInterval(() => void tick(), SWEEP_POLL_INTERVAL_MS);
  };

  // "Re-extract with latest pipeline" (epic #398 Item B): re-arm the company's
  // successful ESEF-tier runs whose stored pipeline version is stale, so a
  // widened crosswalk/projection reaches already-landed filings. NOT gated on
  // automation mode — the "Try again" posture (explicit, per-document
  // re-read), not new automation.
  const runReextraction = async () => {
    if (busy) return;
    setReextracting(true);
    setActionError(null);
    try {
      const started = await runPipelineReextraction(companyId);
      pollReextractionUntilSettled(started.id);
    } catch (reason) {
      setActionError(reason instanceof Error ? reason.message : String(reason));
      setReextracting(false);
    }
  };

  // "Extracting…" with the live drain counter while runs settle (ADR 0077 §3,
  // T3.3): the sweep enqueues N extraction runs that drain after the job queues,
  // so show {done}/{total} once the total is known, plain otherwise. The AI
  // Shared by the sweep and the re-extraction batch — both are "N runs
  // drain after the job queues" progress with the identical shape.
  const drainLabel = (done: number, total: number): string => {
    if (total > 0) {
      return text("Extracting… {done}/{total}")
        .replace("{done}", String(done))
        .replace("{total}", String(total));
    }
    return text("Extracting…");
  };
  const extractingLabel = (): string =>
    drainLabel(sweepProgress?.runsDone ?? 0, sweepProgress?.runsTotal ?? 0);
  const reextractingLabel = (): string =>
    drainLabel(reextractProgress?.runsDone ?? 0, reextractProgress?.runsTotal ?? 0);

  // AI-budget footer slot (ADR 0077 §6, T5.3): the latest sweep's tier-4
  // Lean status line (ADR 0077 §3, T3.2): backfill phase while backfilling, then the
  // sweep status. `automation_off` is explicit, never a silent skip.
  const statusLabel = (): string | null => {
    if (backfilling) return text("Backfilling…");
    if (sweeping) return extractingLabel();
    if (reextracting) return reextractingLabel();
    if (modeOff) return text("Automation off");
    if (backfillProgress?.status === "failed" && backfillProgress.error) {
      // A market with no history-capable source adapter (NewConnect today) fails
      // with the machine code `unsupported_market` (card bfc4c98) — say so
      // specifically, never the blanket "failed". Other errors keep the generic.
      if (backfillProgress.error.startsWith("unsupported_market")) {
        return text("Backfill isn't available for this company's market (NewConnect) yet");
      }
      if (backfillProgress.error.startsWith("no_bankier_page")) {
        return text("No Bankier page was found for this company");
      }
      if (backfillProgress.error.startsWith("http_error")) {
        return text("Couldn't reach Bankier — try again later");
      }
      if (backfillProgress.error.startsWith("parse_error")) {
        return text("The page was fetched but couldn't be read");
      }
      if (backfillProgress.error.startsWith("not_tracked")) {
        return text("This company isn't tracked by this source");
      }
      return text("Backfill failed");
    }
    const sweep = sweepProgress?.sweep;
    if (sweep) {
      if (sweep.skippedReason === "automation_off") return text("Automation off");
      switch (sweep.status) {
        case "queued":
          return text("Queued");
        case "running":
          return extractingLabel();
        case "failed":
          return text("History sweep failed");
        case "completed":
          return text("Extracted {n} · skipped {m}")
            .replace("{n}", String(sweep.runsEnqueued))
            .replace("{m}", String(sweep.skippedExisting));
        default:
          return null;
      }
    }
    // No sweep yet — fall back to the re-extraction batch's own summary, so
    // a company that only ever used "Re-extract with latest pipeline" still
    // gets a status line instead of a permanently blank one.
    const batch = reextractProgress?.batch;
    if (!batch) return null;
    switch (batch.status) {
      case "queued":
        return text("Queued");
      case "running":
        return reextractingLabel();
      case "failed":
        return text("Re-extraction failed");
      case "completed":
        return text("Re-extracted {n}").replace("{n}", String(batch.runsEnqueued));
      default:
        return null;
    }
  };

  // Product-facing kind label for the canonical report (reuses the report-documents
  // taxonomy labels, ADR 0077 §1). Only the two periodic kinds are canonical.
  const reportKindLabel = (docKind: string): string =>
    docKind === "periodic_jsf" ? text("Standalone report") : text("Consolidated report");

  const renderReport = (report: CoverageReportCell | null) => {
    if (!report) {
      return (
        <>
          <span className="coverage-cell-miss">{text("No report")}</span>
          <span className="coverage-sub">{text("not found in backfill")}</span>
        </>
      );
    }
    const title = report.title?.trim();
    return (
      <>
        <span className="coverage-report-chips">
          <StatusChip tone="accent">{reportKindLabel(report.docKind)}</StatusChip>
          {report.structured ? <StatusChip>{text("ESEF")}</StatusChip> : null}
        </span>
        {title ? (
          <span className="coverage-sub coverage-doc-title" title={title}>
            {middleTruncate(title)}
          </span>
        ) : null}
      </>
    );
  };

  const renderFacts = (row: CoveragePeriodRow) => {
    const { total, validated, unvalidated, flagged } = row.facts;
    if (total === 0) {
      // No report and no facts → simply absent.
      if (!row.report) {
        return (
          <span className="coverage-cell-miss" aria-hidden="true">
            —
          </span>
        );
      }
      // A metadata-only (link-only) canonical report has no stored file, so
      // "click → Extract" would mislead (found on the owner's real DB: 2024/2023
      // FY annuals). Align with the sweep, which skips these (T3.1). The report
      // cell still shows the report's identity, so the two cells stay consistent:
      // "report exists as a link" + "no file to extract".
      if (!row.report.fetched) {
        return <span className="coverage-cell-miss">{text("link-only — no stored file")}</span>;
      }
      // A fetched report exists but was never processed → an actionable gap (row
      // click → documents → Extract).
      return (
        <>
          <span className="coverage-cell-miss">{text("not processed")}</span>
          <span className="coverage-sub">{text("click → Extract")}</span>
        </>
      );
    }
    const factsLabel = `${total} ${pluralNoun(locale, total, FACT_FORMS)}`;
    if (validated > 0 && flagged === 0 && unvalidated === 0) {
      return (
        <>
          <span className="coverage-cell-ok">
            <Check size={13} aria-hidden="true" /> <span className="coverage-primary">{factsLabel}</span>
          </span>
          <span className="coverage-sub">{text("all validated")}</span>
        </>
      );
    }
    const attention = flagged + unvalidated;
    const breakdown = text("{v} validated · {n} flagged/divergent")
      .replace("{v}", String(validated))
      .replace("{n}", String(attention));
    return (
      <>
        <span className="coverage-cell-warn">
          <span className="coverage-primary">{factsLabel}</span>
        </span>
        <span className="coverage-sub">{breakdown}</span>
      </>
    );
  };

  // "Flagged" = the period's flagged facts. Facts are review-free (ADR 0086 dec.
  // 5): this is an informational origin label — the pipeline flagged a value as
  // drifted/contradicted — never a confirmation the user owes. The KPI-proposal
  // staging ledger was dropped in the ADR 0084 clean cut.
  const renderReview = (row: CoveragePeriodRow) => {
    const flaggedFacts = row.review.flaggedFacts;
    if (flaggedFacts > 0) {
      return (
        <>
          <span className="coverage-cell-warn">
            {flaggedFacts} {pluralNoun(locale, flaggedFacts, FLAGGED_FORMS)}
          </span>
        </>
      );
    }
    return (
      <span className="coverage-cell-miss" aria-hidden="true">
        —
      </span>
    );
  };

  const clickable = Boolean(onOpenDocuments);

  // data-company-id: a cockpit layout can hold several coverage panes (follow +
  // pinned to other companies) — tests and live probes MUST scope by company, or
  // a read can silently land on a neighbour's pane (found live, 2026-07-10).
  return (
    <div role="group" className="company-coverage" data-company-id={companyId} aria-label={text("Coverage")}>
      <SectionHeader paneLead title={text("Coverage")} meta={periods.length} />
      {error ? <ErrorText>{error}</ErrorText> : null}
      {periods.length === 0 && !error ? (
        <EmptyState>{text("No coverage data yet.")}</EmptyState>
      ) : (
        <>
          {/* The coverage table is DELIBERATE wide content: it scrolls inside this
              bounded, `contain: inline-size` wrapper so a narrow (S) pane scrolls
              it horizontally instead of forcing a panel-level scrollbar (the
              facts-matrix pattern; ui-authoring § panel-internal scrollbars). */}
          <div className="coverage-scroll" data-hscroll aria-label={text("Coverage map")}>
            <table className="coverage-table">
              <thead>
                <tr>
                  <th scope="col">{text("Period")}</th>
                  <th scope="col">{text("Report")}</th>
                  <th scope="col">{text("Data")}</th>
                  <th scope="col">{text("Flagged")}</th>
                </tr>
              </thead>
              <tbody>
                {periods.map((row) => {
                  const periodLabel = `${row.fiscalYear} ${periodTypeLabel(row.periodType)}`;
                  return (
                    // The whole row opens documents on mouse click (convenience),
                    // but the KEYBOARD/SR affordance is the period-cell button
                    // below — the row itself is NOT `role="button"`, because a row
                    // that is a button while containing the review button is an axe
                    // nested-interactive (WCAG 4.1.2) violation.
                    <tr
                      key={`${row.fiscalYear}-${row.periodType}`}
                      className={[
                        "coverage-row",
                        clickable ? "coverage-row-clickable" : "",
                      ]
                        .filter(Boolean)
                        .join(" ")}
                      onClick={clickable ? onOpenDocuments : undefined}
                    >
                      <td
                        className={[
                          "coverage-period",
                          isFullYear(row.periodType) ? "coverage-period-fy" : "",
                        ]
                          .filter(Boolean)
                          .join(" ")}
                      >
                        {clickable ? (
                          <button
                            type="button"
                            className="coverage-period-button"
                            aria-label={`${periodLabel} — ${text("Open report documents")}`}
                            onClick={(event) => {
                              // The row's own onClick also opens documents — stop
                              // the bubble so a single click fires it once.
                              event.stopPropagation();
                              onOpenDocuments?.();
                            }}
                          >
                            {periodLabel}
                          </button>
                        ) : (
                          periodLabel
                        )}
                      </td>
                      <td className="coverage-cell">{renderReport(row.report)}</td>
                      <td className="coverage-cell">{renderFacts(row)}</td>
                      <td className="coverage-cell">{renderReview(row)}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          <div className="coverage-legend">
            <span>{text("Validated")}</span>
            <span>{text("Flagged")}</span>
            <span>{text("Missing")}</span>
            <span>{text("Row click → documents")}</span>
          </div>
        </>
      )}
      {/* Flagged periods (ADR 0061 decision 2, ADR 0084 decision 4/6) — the UI
          half of "never silently wrong". Rendered OUTSIDE the periods-empty
          branch on purpose: a `no_period_derived` failure has no coverage row
          (the period-union rule cannot name a period it could not derive), so
          gating it on the table would hide exactly the gaps it exists to show. */}
      <CoverageFlaggedPeriods
        companyId={companyId}
        reloadKey={reloadKey + localReloadTick}
        onRerun={refreshAfterExtraction}
      />
      {/* Flagged FACTS (epic #229 T5): the values that DID land but carry a
          drift/contradiction. The "Flagged" table column counts them per
          period; this section is where the figures themselves are readable —
          without it the count pointed at nothing (the read had no UI consumer). */}
      <CoverageFlaggedFacts companyId={companyId} reloadKey={reloadKey + localReloadTick} />
      {/* Raw tagged-fact capture proof + promotion (ADR 0100, epic #398 final
          slice): the funnel compacted to one line, with a disclosure into the
          "positions the program doesn't know yet" list — silent (renders
          nothing) for a company with no tagged capture yet. */}
      <CoverageRawCapture companyId={companyId} reloadKey={reloadKey + localReloadTick} />
      {/* History actions footer (ADR 0077 §3, T3.2; epic #398 Item B). Fixed
          slots: three actions + a status line (ui-authoring styling rules).
          Backfill/Extract-missing disable in automation mode `off`;
          re-extraction does NOT (it reprocesses already-stored documents on
          explicit request, the "Try again" posture) — only `busy` (another
          footer action in flight) disables it. The status line names the
          phase, never silent. */}
      <div className="coverage-actions">
        <div className="coverage-action-buttons">
          <Button
            variant="primary"
            className="coverage-action-button"
            disabled={modeOff || busy}
            onClick={runBackfill}
          >
            <History size={15} aria-hidden="true" />
            {backfilling ? text("Backfilling…") : text("Backfill history")}
          </Button>
          <Button
            variant="secondary"
            className="coverage-action-button"
            disabled={modeOff || busy}
            onClick={runSweep}
          >
            <Sparkles size={15} aria-hidden="true" />
            {sweeping ? text("Extracting…") : text("Extract missing periods")}
          </Button>
          <Button
            variant="secondary"
            className="coverage-action-button"
            disabled={busy}
            onClick={runReextraction}
          >
            <RefreshCw size={15} aria-hidden="true" />
            {reextracting ? text("Re-extracting…") : text("Re-extract with latest pipeline")}
          </Button>
        </div>
        <div className="coverage-action-status" role="status">
          {statusLabel() ?? ""}
                  </div>
        {/* Truncation honesty (ADR 0077 §3): a backfill that hit the page cap
            before the configured cutoff may be missing older filings — surface
            it, never silent. A caution, not an error (the fetch still succeeded). */}
        {backfillProgress?.truncated ? (
          <Hint>{text("History truncated at the page cap — older filings may be missing.")}</Hint>
        ) : null}
        {modeOff ? <Hint>{text("Enable automation to extract history.")}</Hint> : null}
        {actionError ? <ErrorText>{actionError}</ErrorText> : null}
      </div>
    </div>
  );
}
