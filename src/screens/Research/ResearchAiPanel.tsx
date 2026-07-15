import { Sparkles } from "lucide-react";
import type { Company, Watchlist } from "../../api/types";
import type {
  ResearchBriefJob,
  ResearchDigestJob,
  ResearchEvidenceItem,
  ResearchEvidenceType,
} from "../../api/researchTypes";
import type { ResearchMode } from "../../app/useResearchController";
import { formatAiProvider } from "../../shared/formatting/labels";
import { Button, EmptyState, ErrorText, SectionHeader, useToast } from "../../ui";

type ResearchAiPanelProps = {
  briefJobs: ResearchBriefJob[];
  digestJobs: ResearchDigestJob[];
  briefInFlight: boolean;
  digestInFlight: boolean;
  mode: ResearchMode;
  selectedCompany: Company | null;
  selectedWatchlist: Watchlist | null;
  items: ResearchEvidenceItem[];
  startBrief: () => void;
  startDigest: () => void;
  openEvidence: (item: ResearchEvidenceItem) => void;
  formatTimestamp: (value: string | null | undefined) => string;
  text: (value: string) => string;
};

export function ResearchAiPanel({
  briefJobs,
  digestJobs,
  briefInFlight,
  digestInFlight,
  mode,
  selectedCompany,
  selectedWatchlist,
  items,
  startBrief,
  startDigest,
  openEvidence,
  formatTimestamp,
  text,
}: ResearchAiPanelProps) {
  const toast = useToast();
  // v0.54 T6: an AI generation starts async and renders its result far down the
  // output section — a transient start toast confirms the kick-off globally.
  function beginDigest() {
    toast.show({ message: text("Generating digest…") });
    startDigest();
  }
  function beginBrief() {
    toast.show({ message: text("Generating brief…") });
    startBrief();
  }
  const latestBriefJob = briefJobs[0] ?? null;
  const latestBrief = latestBriefJob?.brief ?? null;
  const latestDigestJob = digestJobs[0] ?? null;
  const latestDigest = latestDigestJob?.digest ?? null;
  const briefRunning = briefJobs.some((job) => job.status === "queued" || job.status === "running");
  const digestRunning = digestJobs.some((job) => job.status === "queued" || job.status === "running");
  const aiPanelDisabled = mode === "company" ? !selectedCompany : !selectedWatchlist;
  const latestBriefTitle =
    mode === "company" && selectedCompany
      ? `${text("Investor research brief")}: ${selectedCompany.displayName}`
      : mode === "watchlist" && selectedWatchlist
        ? `${text("Investor research brief")}: ${selectedWatchlist.name}`
        : text("Investor research brief");
  const latestBriefParagraphs = markdownParagraphs(latestBrief?.contentMarkdown);
  const latestDigestParagraphs = markdownParagraphs(latestDigest?.contentMarkdown);

  function openCitationEvidence(evidenceType: ResearchEvidenceType, evidenceId: string) {
    const item = items.find(
      (current) => current.evidenceType === evidenceType && current.sourceId === evidenceId,
    );

    if (item) {
      openEvidence(item);
    }
  }

  function isCitationOpenable(evidenceType: ResearchEvidenceType, evidenceId: string) {
    return items.some((item) => item.evidenceType === evidenceType && item.sourceId === evidenceId);
  }

  return (
    <section className="research-ai-panel" aria-label={text("AI research")}>
      <SectionHeader
        className="research-section-ai"
        description={text("Generate either a review checkpoint or a source-grounded research summary.")}
        meta={briefJobs.length + digestJobs.length}
        title={text("AI research")}
        variant="accent"
      />
      <div className="research-ai-modes" aria-label={text("AI research modes")}>
        <div className="research-ai-mode-card">
          <div>
            <span className="research-ai-mode-kicker">{text("What needs attention")}</span>
            <strong>{text("Digest")}</strong>
            <p>{text("Summarizes open reminders and changed evidence for this research scope.")}</p>
          </div>
          <Button
            className="compact-button"
            disabled={digestInFlight || digestRunning || aiPanelDisabled}
            onClick={beginDigest}
            title={text("Summarizes open reminders and changed evidence for this research scope.")}
          >
            <Sparkles size={15} />
            {digestRunning ? text("Generating") : text("Generate digest")}
          </Button>
        </div>
        <div className="research-ai-mode-card">
          <div>
            <span className="research-ai-mode-kicker">{text("Research summary")}</span>
            <strong>{text("Brief")}</strong>
            <p>{text("Creates a cited research snapshot from the selected evidence.")}</p>
          </div>
          <Button
            className="compact-button"
            disabled={briefInFlight || briefRunning || aiPanelDisabled}
            onClick={beginBrief}
            title={text("Creates a cited research snapshot from the selected evidence.")}
          >
            <Sparkles size={15} />
            {briefRunning ? text("Generating") : text("Generate brief")}
          </Button>
        </div>
      </div>
      <div className="research-ai-output-list">
        <section className="research-ai-output" aria-label={text("Digest output")}>
          <SectionHeader
            className="research-ai-output-heading"
            description={text("Reminder and changed-evidence checkpoint.")}
            level="h3"
            meta={digestJobs.length}
            title={text("What needs attention")}
          />
          {latestDigest ? (
            <article className="research-brief-card">
              <div className="research-brief-card-header">
                <div>
                  <h3>{latestDigest.title}</h3>
                  <p className="research-brief-summary">{latestDigest.summary}</p>
                </div>
                <span>{formatTimestamp(latestDigest.generatedAt)}</span>
              </div>
              <div className="research-brief-content">
                {latestDigestParagraphs.map((line, index) => (
                  <p key={`${latestDigest.id}-${index}`}>{line}</p>
                ))}
              </div>
              <details className="research-brief-citations">
                <summary>{text("Citations")} ({latestDigest.citations.length})</summary>
                <div className="research-brief-citation-list">
                  {latestDigest.citations.map((citation) => (
                    <button
                      className="research-brief-citation"
                      disabled={!isCitationOpenable(citation.evidenceType, citation.evidenceId)}
                      key={citation.id}
                      type="button"
                      onClick={() => openCitationEvidence(citation.evidenceType, citation.evidenceId)}
                    >
                      <strong>{citation.citationKey}</strong>
                      {citation.label}
                    </button>
                  ))}
                </div>
              </details>
            </article>
          ) : latestDigestJob?.status === "failed" ? (
            <ErrorText>{latestDigestJob.error ?? text("Research digest failed")}</ErrorText>
          ) : (
            <EmptyState>{text("No research digest generated yet.")}</EmptyState>
          )}
        </section>
        <section className="research-ai-output" aria-label={text("Brief output")}>
          <SectionHeader
            className="research-ai-output-heading"
            description={text("Source-grounded memo for the selected scope.")}
            level="h3"
            meta={briefJobs.length}
            title={text("Research summary")}
          />
          {latestBrief ? (
            <article className="research-brief-card">
              <div className="research-brief-card-header">
                <div>
                  <h3>{latestBriefTitle}</h3>
                  <p className="research-brief-summary">{latestBrief.summary}</p>
                </div>
                <span>{formatTimestamp(latestBrief.generatedAt)}</span>
              </div>
              <div className="research-brief-content">
                {latestBriefParagraphs.map((line, index) => (
                  <p key={`${latestBrief.id}-${index}`}>{line}</p>
                ))}
              </div>
              <details className="research-brief-citations">
                <summary>{text("Citations")} ({latestBrief.citations.length})</summary>
                <div className="research-brief-citation-list">
                  {latestBrief.citations.map((citation) => (
                    <button
                      className="research-brief-citation"
                      disabled={!isCitationOpenable(citation.evidenceType, citation.evidenceId)}
                      key={citation.id}
                      type="button"
                      onClick={() => openCitationEvidence(citation.evidenceType, citation.evidenceId)}
                    >
                      <strong>{citation.citationKey}</strong>
                      {citation.label}
                    </button>
                  ))}
                </div>
              </details>
              <div className="research-brief-provenance">
                <span>{text("Generated")} {formatTimestamp(latestBrief.generatedAt)}</span>
                <span>{text(formatAiProvider(latestBrief.providerId))}</span>
                <span>{latestBrief.model}</span>
              </div>
            </article>
          ) : latestBriefJob?.status === "failed" ? (
            <ErrorText>{latestBriefJob.error ?? text("Research brief failed")}</ErrorText>
          ) : (
            <EmptyState>{text("No research brief generated yet.")}</EmptyState>
          )}
        </section>
      </div>
    </section>
  );
}

function markdownParagraphs(markdown: string | null | undefined) {
  return (
    markdown
      ?.split(/\n+/)
      .map((line) => line.replace(/^##\s*/, "").trim())
      .filter(Boolean) ?? []
  );
}
