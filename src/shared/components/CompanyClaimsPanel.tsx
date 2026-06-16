import { useCallback, useEffect, useState } from "react";
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
import { useLocale } from "../locale";
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
export function CompanyClaimsPanel({ companyId }: CompanyClaimsPanelProps) {
  const { text } = useLocale();
  const [claims, setClaims] = useState<ManagementClaim[]>([]);
  const [queue, setQueue] = useState<ClaimsToVerify | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [statement, setStatement] = useState("");
  const [dueYear, setDueYear] = useState("");
  const [duePeriod, setDuePeriod] = useState("");

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

  const resolveVerdict = async (claim: ManagementClaim, status: ClaimStatus) => {
    try {
      await setClaimVerdict({ claimId: claim.id, status });
      await reload();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
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

  return (
    <section className="company-claims-panel" aria-label={text("Management claims")}>
      <SectionHeader
        level="h3"
        title={text("Management claims")}
        description={text(
          "Track management promises with a due period and a verdict. Claims resurface for review when the due-period report arrives.",
        )}
      />

      {hasQueue ? (
        <div className="claims-review-queue" aria-label={text("Claims to verify")}>
          <SectionHeader level="h4" title={text("Claims to verify")} />
          {queueBuckets.map(({ key, label }) =>
            queue && queue[key].length > 0 ? (
              <div className="claims-queue-bucket" key={key}>
                <Hint>{`${label} · ${queue[key].length}`}</Hint>
                {queue[key].map((entry: ClaimToVerify) => (
                  <div className="claim-queue-row" key={entry.claim.id}>
                    <div className="claim-queue-statement">{entry.claim.statement}</div>
                    {entry.verifyingFactCandidate ? (
                      <Hint>
                        {text("Reported value")}: {entry.verifyingFactCandidate.valueNumeric}
                      </Hint>
                    ) : null}
                    {key !== "upcoming" ? (
                      <div className="claim-queue-actions">
                        <Button
                          className="compact-button"
                          onClick={() => resolveVerdict(entry.claim, "delivered")}
                        >
                          {text("Delivered")}
                        </Button>
                        <Button
                          className="compact-button"
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
          <div className="claim-row" key={claim.id}>
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

      {error ? (
        <ErrorText>
          {text("Claims command failed")}: {error}
        </ErrorText>
      ) : null}
    </section>
  );
}
