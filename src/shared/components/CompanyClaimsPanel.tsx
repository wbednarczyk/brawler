import { useCallback, useEffect, useRef, useState } from "react";
import { CheckCircle2, Plus } from "lucide-react";

import {
  createManagementClaim,
  listClaimsToVerify,
  listManagementClaims,
  setClaimVerdict,
  type ClaimStatus,
  type ClaimsToVerify,
  type ClaimToVerify,
  type ManagementClaim,
} from "../../api/managementClaims";
import { formatFinancialValue } from "../format/financialValue";
import { useLocale } from "../locale";
import { useToolHost } from "../toolHost";
import {
  Button,
  EmptyState,
  ErrorText,
  Hint,
  SectionHeader,
  SelectField,
  StatusPill,
  type StatusPillTone,
  TextField,
} from "../../ui";

type CompanyClaimsPanelProps = {
  companyId: string;
  /** Today's `openCompanyClaims(companyId, claimId)` nav seam (F2 S3, plan
   * decision 6): highlight + scroll this claim into view once it loads,
   * whether it surfaces in the main list or the review queue. */
  highlightClaimId?: string | null;
};

const VERDICTS: ClaimStatus[] = [
  "pending",
  "delivered",
  "partially_delivered",
  "missed",
  "revised",
];

const PERIOD_TYPES = ["FY", "H1", "H2", "Q1", "Q2", "Q3", "Q4", "9M"];

function verdictTone(status: ClaimStatus): StatusPillTone {
  switch (status) {
    case "delivered":
      return "ok";
    case "partially_delivered":
      return "warn";
    case "missed":
      return "danger";
    default:
      return "neutral";
  }
}

/// Self-contained management claims tracker for one company (ADR 0040). Shows the
/// "claims to verify" review queue, the full claims list with user-set verdicts, and
/// a create-claim form. AI claim extraction is launched from the report/transcript
/// context (like KPI extraction); this panel resolves the verdicts.
export function CompanyClaimsPanel({ companyId, highlightClaimId = null }: CompanyClaimsPanelProps) {
  const { text, locale } = useLocale();
  const [claims, setClaims] = useState<ManagementClaim[]>([]);
  const [queue, setQueue] = useState<ClaimsToVerify | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  // Fades on its own after the scroll+flash — the incoming prop stays set for
  // the panel's lifetime (nothing clears it at the root), so the highlight
  // itself has to be transient, not the data driving it.
  const [activeHighlightId, setActiveHighlightId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [statement, setStatement] = useState("");
  const [dueYear, setDueYear] = useState("");
  const [duePeriod, setDuePeriod] = useState("");
  // S-tier composer disclosure + short-tier "top 3 due" expansion (ADR 0076 D6).
  // Both are driven by data flags the density CSS keys off; the tier switch is
  // CSS-only (container queries) so these states are inert at M/L/tall.
  const [composerOpen, setComposerOpen] = useState(false);
  const [shortExpanded, setShortExpanded] = useState(false);
  // In-flight guard for verdict saves (issue #87): the write is idempotent
  // today, but a double dispatch is a latent double-write on any
  // non-idempotent successor — block re-entry while a save is pending.
  const [savingVerdict, setSavingVerdict] = useState(false);

  // Register the claims composer draft with the Spółka workshop's dirty gate
  // (F3a S2, ADR 0107) — a no-op when hosted outside it (e.g. the Companies
  // screen). Dirty = composer open with any field typed.
  const { register } = useToolHost();
  useEffect(() => {
    return register({
      isDirty: () => composerOpen && (statement.trim() !== "" || dueYear !== "" || duePeriod !== ""),
      discard: () => {
        setComposerOpen(false);
        setStatement("");
        setDueYear("");
        setDuePeriod("");
      },
    });
  }, [register, composerOpen, statement, dueYear, duePeriod]);

  const reload = useCallback(async () => {
    try {
      const [nextClaims, nextQueue] = await Promise.all([
        listManagementClaims(companyId),
        listClaimsToVerify(companyId),
      ]);
      setClaims(nextClaims);
      setQueue(nextQueue);
      setError(null);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    }
  }, [companyId]);

  useEffect(() => {
    void reload();
  }, [reload]);

  // Scroll the targeted claim into view + flash it once it's actually
  // rendered (either list — the main claims list or the review queue), then
  // let the flash fade on its own after a few seconds. Also lifts the short
  // pane-height tier's collapse (`claims.css` "short height tier": the full
  // `.claims-body` — where the row lives — is `display:none` behind
  // `data-short-expanded` under 480px) — a highlight the user cannot see
  // defeats the whole seam (sol R1 finding 9 browser-proof caught this: the
  // Claims tab activated and the row got the highlight class, but the row
  // stayed CSS-hidden in a short dock pane).
  useEffect(() => {
    if (!highlightClaimId) return undefined;
    const row = panelRef.current?.querySelector<HTMLElement>(`[data-claim-id="${highlightClaimId}"]`);
    if (!row) return undefined;
    setShortExpanded(true);
    row.scrollIntoView({ block: "center" });
    setActiveHighlightId(highlightClaimId);
    const timer = window.setTimeout(() => setActiveHighlightId(null), 4000);
    return () => window.clearTimeout(timer);
  }, [highlightClaimId, claims, queue]);

  const resolveVerdict = async (claim: ManagementClaim, status: ClaimStatus) => {
    if (savingVerdict) return;
    setSavingVerdict(true);
    try {
      await setClaimVerdict({ claimId: claim.id, status });
      await reload();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setSavingVerdict(false);
    }
  };

  const submitNewClaim = async () => {
    const trimmed = statement.trim();
    if (!trimmed) return;
    try {
      await createManagementClaim({
        companyId,
        statement: trimmed,
        dueFiscalYear: dueYear ? Number(dueYear) : null,
        duePeriodType: duePeriod || null,
      });
      setStatement("");
      setDueYear("");
      setDuePeriod("");
      await reload();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    }
  };

  const queueBuckets: { key: keyof ClaimsToVerify; label: string }[] = [
    { key: "due", label: text("Due now") },
    { key: "overdue", label: text("Overdue") },
    { key: "upcoming", label: text("Upcoming") },
  ];
  const hasQueue =
    queue !== null &&
    (queue.due.length > 0 || queue.overdue.length > 0 || queue.upcoming.length > 0);
  // Short-tier summary (ADR 0076 D6): per-status counts line + the top 3 due
  // claims only; the full panel folds behind an expansion.
  const topDue = (queue?.due ?? []).slice(0, 3);

  return (
    <div
      ref={panelRef}
      role="group"
      className="company-claims-panel"
      aria-label={text("Management claims")}
      {...(composerOpen ? { "data-composer-open": "" } : {})}
      {...(shortExpanded ? { "data-short-expanded": "" } : {})}
    >
      <SectionHeader
        level="h3"
        paneLead
        title={text("Management claims")}
        description={text(
          "Track management promises with a due period and a verdict. Claims resurface for review when the due-period report arrives.",
        )}
      />

      <div className="claims-queue-summary" aria-label={text("Claims summary")}>
        <div className="claims-queue-counts">
          {queueBuckets.map(({ key, label }) => (
            <span className="claims-count" key={key}>
              <span className="claims-count-value">{queue ? queue[key].length : 0}</span>
              <span className="claims-count-label">{label}</span>
            </span>
          ))}
        </div>
        {topDue.length > 0 ? (
          <div className="claims-queue-top">
            {topDue.map((entry: ClaimToVerify) => (
              <div className="claims-queue-top-item" key={entry.claim.id}>
                {entry.claim.statement}
              </div>
            ))}
          </div>
        ) : null}
        <Button
          className="claims-short-toggle compact-button"
          onClick={() => setShortExpanded((current) => !current)}
          aria-expanded={shortExpanded}
        >
          {shortExpanded ? text("Show fewer claims") : text("Show all claims")}
        </Button>
      </div>

      <div className="claims-body">
        <div className="claims-main">
          <Button
            className="claims-add-toggle compact-button"
            // The Spółka workshop's `tezy` tool primary action (ADR 0081 Q4,
            // plan §6 "W otwartym narzędziu: primary narzędzia, max: 1") — the
            // queue's Delivered/Missed pair stays unmarked (a deliberate peer
            // binary, not this surface's single primary).
            data-ux-primary-action="true"
            onClick={() => setComposerOpen((current) => !current)}
            aria-expanded={composerOpen}
          >
            <Plus size={15} />
            {text("Add claim")}
          </Button>

          <form
            className="claim-create-form"
            aria-label={text("Add a claim")}
            onSubmit={(event) => {
              event.preventDefault();
              void submitNewClaim();
            }}
          >
            <TextField
              label={text("Claim")}
              value={statement}
              onChange={(event) => setStatement(event.target.value)}
              placeholder={text("What did management promise?")}
            />
            <TextField
              label={text("Due year")}
              inputMode="numeric"
              value={dueYear}
              onChange={(event) => setDueYear(event.target.value.replace(/[^0-9]/g, ""))}
              placeholder="2026"
            />
            <SelectField
              label={text("Due period")}
              value={duePeriod}
              onChange={(event) => setDuePeriod(event.target.value)}
            >
              <option value="">{text("None")}</option>
              {PERIOD_TYPES.map((period) => (
                <option key={period} value={period}>
                  {period}
                </option>
              ))}
            </SelectField>
            <Button type="submit" variant="primary" disabled={!statement.trim()}>
              <Plus size={15} />
              {text("Add claim")}
            </Button>
          </form>

          <div className="claims-list" aria-label={text("Management claims")}>
            {claims.map((claim) => (
              <div
                className={["claim-row", claim.id === activeHighlightId ? "claim-row-highlighted" : ""]
                  .filter(Boolean)
                  .join(" ")}
                data-claim-id={claim.id}
                key={claim.id}
              >
                <div className="claim-row-main">
                  <CheckCircle2 size={15} />
                  <span className="claim-row-statement">{claim.statement}</span>
                  <StatusPill tone={verdictTone(claim.status)}>
                    {text(claim.status.replace(/_/g, " "))}
                  </StatusPill>
                </div>
                <div className="claim-row-meta">
                  {claim.dueFiscalYear && claim.duePeriodType ? (
                    <Hint>{`${claim.duePeriodType} ${claim.dueFiscalYear}`}</Hint>
                  ) : null}
                  <SelectField
                    label={text("Verdict")}
                    aria-label={text("Claim verdict")}
                    value={claim.status}
                    disabled={savingVerdict}
                    onChange={(event) =>
                      void resolveVerdict(claim, event.target.value as ClaimStatus)
                    }
                  >
                    {VERDICTS.map((verdict) => (
                      <option key={verdict} value={verdict}>
                        {text(verdict.replace(/_/g, " "))}
                      </option>
                    ))}
                  </SelectField>
                </div>
              </div>
            ))}
            {claims.length === 0 ? (
              <EmptyState>{text("No management claims tracked yet.")}</EmptyState>
            ) : null}
          </div>
        </div>

        {hasQueue ? (
          <div role="group" className="claims-verdict-column claims-review-queue" aria-label={text("Claims to verify")}>
            <SectionHeader level="h4" title={text("Claims to verify")} />
            {queueBuckets.map(({ key, label }) =>
              queue && queue[key].length > 0 ? (
                <div className="claims-queue-bucket" key={key}>
                  <Hint>{`${label} · ${queue[key].length}`}</Hint>
                  {queue[key].map((entry: ClaimToVerify) => (
                    <div
                      className={[
                        "claim-queue-row",
                        entry.claim.id === activeHighlightId ? "claim-row-highlighted" : "",
                      ]
                        .filter(Boolean)
                        .join(" ")}
                      data-claim-id={entry.claim.id}
                      key={entry.claim.id}
                    >
                      <div className="claim-queue-statement">{entry.claim.statement}</div>
                      {entry.verifyingFactCandidate ? (
                        <Hint>
                          {text("Reported value")}:{" "}
                          {formatFinancialValue(entry.verifyingFactCandidate, locale)}
                        </Hint>
                      ) : null}
                      {key !== "upcoming" ? (
                        <div className="claim-queue-actions">
                          <Button
                            className="compact-button"
                            disabled={savingVerdict}
                            onClick={() => resolveVerdict(entry.claim, "delivered")}
                          >
                            {text("Delivered")}
                          </Button>
                          <Button
                            className="compact-button"
                            disabled={savingVerdict}
                            onClick={() => resolveVerdict(entry.claim, "missed")}
                          >
                            {text("Missed")}
                          </Button>
                        </div>
                      ) : null}
                    </div>
                  ))}
                </div>
              ) : null,
            )}
          </div>
        ) : null}
      </div>

      {error ? (
        <ErrorText>
          {text("Claims command failed")}: {error}
        </ErrorText>
      ) : null}
    </div>
  );
}
