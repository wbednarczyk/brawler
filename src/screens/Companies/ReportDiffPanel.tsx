import { useCallback, useEffect, useState } from "react";
import { ArrowLeft } from "lucide-react";
import { useLocale } from "../../shared/locale";
import { Button, EmptyState, ErrorText, ListRow, SectionHeader, StatusChip } from "../../ui";
import {
  extractReportSections,
  fetchReportDocument,
  getReportDiff,
  listReportDiffCandidates,
} from "../../api/reportDiff";
import type {
  ReportDiffCandidate,
  ReportDiffResult,
  SectionDiff,
} from "../../api/reportDiff";

type ReportDiffPanelProps = {
  companyId: string;
};

type Tone = "neutral" | "accent" | "ok" | "warn" | "danger";

function sectionTone(status: SectionDiff["status"]): Tone {
  switch (status) {
    case "changed":
      return "warn";
    case "only_older":
      return "danger";
    case "only_newer":
      return "ok";
    default:
      return "neutral";
  }
}

function refLabel(ref: ReportDiffCandidate["older"]): string {
  return ref.periodLabel ?? ref.title ?? ref.reportDocumentId;
}

// The report-over-report diff (v0.47.0, ADR 0052): compare two consecutive
// same-type financial statements section by section. Deterministic and local —
// no AI. Reports without an extractable text layer (scanned) are flagged, not diffed.
export function ReportDiffPanel({ companyId }: ReportDiffPanelProps) {
  const { text } = useLocale();
  const [candidates, setCandidates] = useState<ReportDiffCandidate[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [active, setActive] = useState<ReportDiffCandidate | null>(null);
  const [diff, setDiff] = useState<ReportDiffResult | null>(null);
  const [diffError, setDiffError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setCandidates(null);
    setError(null);
    setActive(null);
    setDiff(null);
    listReportDiffCandidates(companyId)
      .then((result) => {
        if (!cancelled) setCandidates(result.candidates);
      })
      .catch((reason) => {
        if (!cancelled) setError(String(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [companyId]);

  const openDiff = useCallback(
    async (candidate: ReportDiffCandidate, doExtract: boolean) => {
      setActive(candidate);
      setDiff(null);
      setDiffError(null);
      setBusy(true);
      try {
        if (doExtract) {
          // Fetch the files on demand (no-op if already downloaded), then extract.
          await fetchReportDocument(candidate.older.reportDocumentId);
          await fetchReportDocument(candidate.newer.reportDocumentId);
          await extractReportSections(candidate.older.reportDocumentId);
          await extractReportSections(candidate.newer.reportDocumentId);
        }
        const result = await getReportDiff(
          candidate.older.reportDocumentId,
          candidate.newer.reportDocumentId,
        );
        setDiff(result);
      } catch (reason) {
        setDiffError(String(reason));
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  if (active) {
    return (
      <section className="company-report-documents report-diff-panel" aria-label={text("Report comparison")}>
        <SectionHeader
          title={`${refLabel(active.older)} → ${refLabel(active.newer)} · ${active.statementType.toUpperCase()}`}
          actions={
            <Button variant="secondary" icon={<ArrowLeft size={16} />} onClick={() => setActive(null)}>
              {text("Back")}
            </Button>
          }
        />
        {busy ? <p>{text("Comparing…")}</p> : null}
        {diffError ? <ErrorText>{text("Report comparison command failed")}: {diffError}</ErrorText> : null}
        {!busy && diff ? <DiffBody diff={diff} onExtract={() => openDiff(active, true)} /> : null}
      </section>
    );
  }

  return (
    <section className="company-report-documents report-diff-panel" aria-label={text("Report comparison")}>
      <SectionHeader
        title={text("Report comparison")}
        description={text("Compare consecutive financial statements section by section.")}
      />
      {error ? <ErrorText>{text("Report comparison command failed")}: {error}</ErrorText> : null}
      {candidates && candidates.length === 0 ? (
        <EmptyState>{text("No comparable financial statements yet.")}</EmptyState>
      ) : null}
      {candidates && candidates.length > 0 ? (
        <ul className="ui-list-rows">
          {candidates.map((candidate) => {
            const ready =
              candidate.older.extractionStatus !== "no_text_layer" &&
              candidate.older.extractionStatus !== "extraction_failed" &&
              candidate.newer.extractionStatus !== "no_text_layer" &&
              candidate.newer.extractionStatus !== "extraction_failed";
            return (
              <ListRow
                key={`${candidate.older.reportDocumentId}:${candidate.newer.reportDocumentId}`}
                title={`${refLabel(candidate.older)} → ${refLabel(candidate.newer)}`}
                meta={candidate.statementType.toUpperCase()}
                trailing={
                  <Button
                    variant="secondary"
                    onClick={() => openDiff(candidate, true)}
                    disabled={!ready}
                  >
                    {text("Compare")}
                  </Button>
                }
              />
            );
          })}
        </ul>
      ) : null}
    </section>
  );
}

function DiffBody({ diff, onExtract }: { diff: ReportDiffResult; onExtract: () => void }) {
  const { text } = useLocale();

  if (diff.status === "extraction_pending") {
    return (
      <EmptyState actions={<Button variant="primary" onClick={onExtract}>{text("Extract & compare")}</Button>}>
        {text("Sections are still being extracted.")}
      </EmptyState>
    );
  }
  if (diff.status === "not_diffable" || !diff.diff) {
    return <EmptyState>{text("Can't compare — no extractable text (scanned report).")}</EmptyState>;
  }

  const sections = diff.diff.sections;
  const changed = sections.filter((s) => s.status !== "unchanged");
  if (changed.length === 0) {
    return <EmptyState>{text("No changes between these statements.")}</EmptyState>;
  }

  return (
    <>
      <p>
        {changed.length} {text(changed.length === 1 ? "changed section" : "changed sections")} ·{" "}
        {diff.diff.alignedCount} {text("aligned")}
      </p>
      <ul className="ui-list-rows">
        {changed.map((section, index) => (
          <ListRow
            key={`${section.heading}:${section.olderOrdinal ?? "x"}:${section.newerOrdinal ?? "x"}:${index}`}
            title={section.heading}
            titleAttr={section.heading}
            meta={
              section.status === "changed"
                ? `+${section.addedLines} −${section.removedLines}`
                : undefined
            }
            trailing={<StatusChip tone={sectionTone(section.status)}>{statusLabel(section.status, text)}</StatusChip>}
          />
        ))}
      </ul>
    </>
  );
}

function statusLabel(status: SectionDiff["status"], text: (s: string) => string): string {
  switch (status) {
    case "changed":
      return text("Changed");
    case "only_older":
      return text("Removed");
    case "only_newer":
      return text("Added");
    default:
      return text("Unchanged");
  }
}
