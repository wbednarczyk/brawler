import { useCallback, useEffect, useState } from "react";
import {
  confirmKpiProposal,
  listPendingKpiProposals,
  rejectKpiProposal,
} from "../../api/kpiExtraction";
import type { PendingKpiProposal } from "../../api/kpiExtraction";
import { useLocale } from "../locale";
import { pluralNoun, type PluralForms } from "../locale/plural";
import { formatFinancialValue } from "../format/financialValue";
import { Button, EmptyState, ErrorText, Hint, SectionHeader, StatusChip } from "../../ui";
import { middleTruncate } from "./CompanyReportDocumentsPanel";

// Pending KPI proposals awaiting review (three Polish forms).
const PROPOSAL_FORMS: PluralForms = {
  en: ["proposal", "proposals"],
  pl: ["propozycja", "propozycje", "propozycji"],
};

type CompanyReviewQueuePanelProps = {
  companyId: string;
  // Bump to force a reload (e.g. after an extraction elsewhere in the cockpit).
  reloadKey?: number;
  // Called after a confirm/reject so the workspace can reload sibling views
  // (the coverage map's pending-proposal counts, the fundamentals facts).
  onReviewed?: () => void;
};

type SourceChip = { tone: "accent" | "warn" | "neutral"; label: string };

// The tier-4 writer stamps the source-snippet with a provenance prefix
// (`ocr_bootstrap` / `ocr_pending_profile` for values parsed from an
// unconfirmed OCR profile, `ocr_flagged` for a deterministic parse the
// identity validation flagged). The chip is derived from it; anything else is
// an older text-AI proposal.
function sourceChip(snippet: string | null, chipText: (s: string) => string): SourceChip {
  const s = snippet?.trim() ?? "";
  if (s.startsWith("ocr_bootstrap") || s.startsWith("ocr_pending_profile")) {
    return { tone: "accent", label: chipText("OCR bootstrap") };
  }
  if (s.startsWith("ocr_flagged")) {
    return { tone: "warn", label: chipText("OCR · flagged") };
  }
  return { tone: "neutral", label: chipText("AI") };
}

// The verbatim citation is the snippet minus its provenance prefix; an older
// text-AI proposal has no prefix, so the whole snippet is the citation.
function citation(snippet: string | null): string | null {
  const s = snippet?.trim();
  if (!s) return null;
  const colon = s.indexOf(":");
  if (colon > 0 && /^ocr_[a-z_]+$/.test(s.slice(0, colon))) {
    return s.slice(colon + 1).trim();
  }
  return s;
}

// A confirm rejected because the proposal's slot already holds a different value
// (backend `value_conflict`, ADR 0077): `invalid financials value for
// value_conflict: stored=<x> proposed=<y>`. Extract both values so the panel can
// explain the stored fact stays, instead of leaking the raw error.
function valueConflict(message: string): { stored: string; proposed: string } | null {
  const match = /value_conflict: stored=(\S+) proposed=(\S+)/.exec(message);
  return match ? { stored: match[1], proposed: match[2] } : null;
}

// The source document's display name: its title, else the file name from its URL.
function documentName(proposal: PendingKpiProposal): string {
  const title = proposal.documentTitle?.trim();
  if (title) return title;
  const url = proposal.documentUrl;
  const tail = url.split("?")[0].split("/").filter(Boolean).pop() ?? url;
  return tail;
}

const PERIOD_INDEX: Record<string, number> = { Q1: 1, Q2: 2, H1: 2, Q3: 3, "9M": 3, Q4: 4, H2: 4, FY: 4 };

function periodTypeLabel(periodType: string): string {
  return periodType.toLowerCase() === "annual" ? "FY" : periodType.toUpperCase();
}

type PeriodGroup = {
  key: string;
  fiscalYear: number | null;
  periodType: string | null;
  proposals: PendingKpiProposal[];
};

// Group the flat proposal list by its detected period (newest first); proposals
// with no detected period fall into a trailing "ungrouped" bucket so nothing is
// silently dropped.
function groupByPeriod(proposals: PendingKpiProposal[]): PeriodGroup[] {
  const groups = new Map<string, PeriodGroup>();
  for (const proposal of proposals) {
    const key =
      proposal.fiscalYear != null && proposal.periodType != null
        ? `${proposal.fiscalYear} ${periodTypeLabel(proposal.periodType)}`
        : "—";
    let group = groups.get(key);
    if (!group) {
      group = {
        key,
        fiscalYear: proposal.fiscalYear,
        periodType: proposal.periodType,
        proposals: [],
      };
      groups.set(key, group);
    }
    group.proposals.push(proposal);
  }
  return [...groups.values()].sort((a, b) => {
    if (a.fiscalYear == null) return 1;
    if (b.fiscalYear == null) return -1;
    return (
      b.fiscalYear - a.fiscalYear ||
      PERIOD_INDEX[periodTypeLabel(b.periodType ?? "")] - PERIOD_INDEX[periodTypeLabel(a.periodType ?? "")]
    );
  });
}

/// The F5 review queue (ADR 0077 §4/§5, T5.3b): every pending KPI proposal for a
/// company, grouped by detected period. Each row is a proposed metric + value,
/// its source citation and document, and a source chip in a fixed slot (OCR
/// bootstrap / OCR · flagged / AI). Confirming an OCR proposal validates the
/// value and confirms the company's extraction profile (so later documents read
/// with the deterministic parser); rejecting discards the proposal. A flagged
/// validation outcome is surfaced honestly and does not block.
export function CompanyReviewQueuePanel({
  companyId,
  reloadKey = 0,
  onReviewed,
}: CompanyReviewQueuePanelProps) {
  const { text, locale } = useLocale();
  const [proposals, setProposals] = useState<PendingKpiProposal[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  // A non-blocking caution when a confirmed value failed identity validation.
  const [flaggedNotice, setFlaggedNotice] = useState<string | null>(null);

  const load = useCallback(() => {
    let cancelled = false;
    listPendingKpiProposals(companyId)
      .then((rows) => {
        if (!cancelled) setProposals(rows);
      })
      .catch((reason) => {
        if (!cancelled) setError(reason instanceof Error ? reason.message : String(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [companyId]);

  useEffect(() => load(), [load, reloadKey]);

  const refetch = useCallback(async () => {
    try {
      setProposals(await listPendingKpiProposals(companyId));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }, [companyId]);

  const confirm = useCallback(
    async (proposal: PendingKpiProposal) => {
      setBusyId(proposal.id);
      setError(null);
      try {
        const result = await confirmKpiProposal({ proposalId: proposal.id });
        if (result.validationStatus === "flagged") {
          setFlaggedNotice(
            text("Confirmed — validation flagged {metric}. Check it in the coverage map.").replace(
              "{metric}",
              proposal.label,
            ),
          );
        }
        await refetch();
        onReviewed?.();
      } catch (reason) {
        const message = reason instanceof Error ? reason.message : String(reason);
        const conflict = valueConflict(message);
        if (conflict) {
          // The row stays pending — the saved fact is never overwritten.
          setError(
            text(
              "Can't confirm {metric}: a different value is already saved (saved {stored}, this proposal {proposed}). The saved value stays — reject this proposal or keep the saved fact; nothing was overwritten.",
            )
              .replace("{metric}", proposal.label)
              .replace("{stored}", conflict.stored)
              .replace("{proposed}", conflict.proposed),
          );
        } else {
          setError(message);
        }
      } finally {
        setBusyId(null);
      }
    },
    [onReviewed, refetch, text],
  );

  const reject = useCallback(
    async (proposal: PendingKpiProposal) => {
      setBusyId(proposal.id);
      setError(null);
      try {
        await rejectKpiProposal(proposal.id);
        await refetch();
        onReviewed?.();
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason));
      } finally {
        setBusyId(null);
      }
    },
    [onReviewed, refetch],
  );

  const groups = groupByPeriod(proposals);

  // data-company-id: see CompanyCoveragePanel — multi-pane layouts require
  // company-scoped selection in tests/live probes.
  return (
    <section className="company-review-queue" data-company-id={companyId} aria-label={text("Review queue")}>
      <SectionHeader paneLead title={text("Review queue")} meta={proposals.length} />
      {error ? <ErrorText>{error}</ErrorText> : null}
      {flaggedNotice ? <Hint>{flaggedNotice}</Hint> : null}
      {proposals.length === 0 && !error ? (
        <EmptyState>
          {text("No proposals to review — deterministic extractions write facts directly.")}
        </EmptyState>
      ) : (
        <>
          <div className="review-groups">
            {groups.map((group) => (
              <div key={group.key} className="review-group">
                <div className="review-group-head">
                  <span className="review-group-label">{group.key}</span>
                  <span className="review-group-meta">
                    {group.proposals.length} {pluralNoun(locale, group.proposals.length, PROPOSAL_FORMS)}
                  </span>
                </div>
                {group.proposals.map((proposal) => {
                  const chip = sourceChip(proposal.sourceSnippet, text);
                  const cite = citation(proposal.sourceSnippet);
                  const doc = documentName(proposal);
                  const busy = busyId === proposal.id;
                  return (
                    <div key={proposal.id} className="review-row">
                      <div className="review-main">
                        <div className="review-line1">
                          <span className="review-metric">{proposal.label}</span>
                          <span className="review-value">
                            {formatFinancialValue(
                              {
                                valueNumeric: proposal.valueNumeric,
                                currency: proposal.currency,
                                unit: proposal.unit,
                              },
                              locale,
                            )}
                          </span>
                        </div>
                        <div className="review-line2" title={`${cite ? `„${cite}" · ` : ""}${doc}`}>
                          {cite ? `„${cite}" · ` : ""}
                          {middleTruncate(doc)}
                        </div>
                      </div>
                      <div className="review-side">
                        <span className="review-source-slot">
                          <StatusChip tone={chip.tone}>{chip.label}</StatusChip>
                        </span>
                        <Button
                          variant="primary"
                          className="review-action"
                          disabled={busy}
                          onClick={() => confirm(proposal)}
                        >
                          {text("Confirm")}
                        </Button>
                        <Button
                          variant="danger"
                          className="review-action"
                          disabled={busy}
                          onClick={() => reject(proposal)}
                        >
                          {text("Reject")}
                        </Button>
                      </div>
                    </div>
                  );
                })}
              </div>
            ))}
          </div>
          <div className="review-footer">
            <Hint>
              {text(
                "Confirming an OCR proposal confirms the company's extraction profile — later documents read with the deterministic parser.",
              )}
            </Hint>
          </div>
        </>
      )}
    </section>
  );
}
