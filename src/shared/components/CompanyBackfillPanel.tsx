import { useEffect, useRef, useState } from "react";
import { History } from "lucide-react";
import { backfillCompanyHistory, getBackfillProgress } from "../../api/sources";
import type { BackfillProgress } from "../../api/types";
import { useLocale } from "../locale";
import { Button } from "../../ui";

type CompanyBackfillPanelProps = {
  companyId: string;
  // Called after a backfill run finishes (success or failure) so sibling views can reload.
  onComplete?: () => void;
};

/// Self-contained on-track history backfill control (ADR 0036). Triggers the ~3-year backfill
/// for one company and shows live progress/diagnostics while it runs, polling the backend so the
/// counts update before the (long-running) command resolves.
export function CompanyBackfillPanel({ companyId, onComplete }: CompanyBackfillPanelProps) {
  const { text } = useLocale();
  const [progress, setProgress] = useState<BackfillProgress | null>(null);
  const [running, setRunning] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const stopPolling = () => {
    if (pollRef.current !== null) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
  };

  // Load any progress recorded for this company on mount / company change.
  useEffect(() => {
    let cancelled = false;
    getBackfillProgress(companyId)
      .then((value) => {
        if (!cancelled) setProgress(value);
      })
      .catch(() => {
        /* best-effort; a missing progress row is not an error */
      });
    return () => {
      cancelled = true;
      stopPolling();
    };
  }, [companyId]);

  const runBackfill = async () => {
    if (running) return;
    setRunning(true);
    stopPolling();
    pollRef.current = setInterval(() => {
      getBackfillProgress(companyId)
        .then((value) => {
          if (value) setProgress(value);
        })
        .catch(() => {});
    }, 1500);

    try {
      const result = await backfillCompanyHistory(companyId);
      setProgress(result);
    } catch (error) {
      setProgress({
        companyId,
        status: "failed",
        pagesFetched: 0,
        itemsIngested: 0,
        documentsStored: 0,
        detailErrors: 0,
        error: error instanceof Error ? error.message : String(error),
        startedAt: "",
        updatedAt: "",
      });
    } finally {
      stopPolling();
      setRunning(false);
      onComplete?.();
    }
  };

  const statusLabel =
    progress?.status === "completed"
      ? text("Backfill complete")
      : progress?.status === "failed"
        ? text("Backfill failed")
        : running || progress?.status === "running"
          ? text("Backfilling…")
          : null;

  return (
    <section className="company-backfill-panel" aria-label={text("Company history")}>
      <div className="company-backfill-head">
        <Button
          className="compact-button"
          disabled={running}
          onClick={runBackfill}
          aria-label={text("Backfill history")}
        >
          <History size={15} />
          {running ? text("Backfilling…") : text("Backfill history")}
        </Button>
        <p className="company-backfill-hint">
          {text("Fetch the last ~3 years of reports and filings for this company.")}
        </p>
      </div>
      {progress ? (
        <div
          className={`company-backfill-status status-${progress.status}`}
          aria-label={text("Company history")}
          role="status"
        >
          <strong>{statusLabel}</strong>
          <span>
            {progress.pagesFetched} {text("pages fetched")} · {progress.itemsIngested}{" "}
            {text("items ingested")} · {progress.documentsStored} {text("documents stored")}
            {progress.detailErrors > 0 ? ` · ${progress.detailErrors} ${text("errors")}` : ""}
          </span>
          {progress.status === "failed" && progress.error ? (
            <span className="error-text">{progress.error}</span>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
