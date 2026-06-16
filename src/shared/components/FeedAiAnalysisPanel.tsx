import { Bot, RefreshCw, Send, Sparkles } from "lucide-react";
import type { FormEvent } from "react";
import { useState } from "react";
import type { AiAnalysisJob, FeedItem } from "../../api/types";
import { Button } from "./Button";
import { StatusPill } from "./StatusPill";
import { DetailSection, Modal, TextareaField } from "../../ui";
import { useLocale } from "../locale";

export type FeedAiAnalysisPanelProps = {
  feedItem: FeedItem;
  jobs: AiAnalysisJob[];
  error: string | null;
  providerConfigured: boolean;
  requestInFlight: boolean;
  onStart: (feedItem: FeedItem, promptPresetId?: string, customQuestion?: string) => Promise<void>;
  onRetry: (jobId: string, feedItemId: string) => Promise<void>;
};

const promptPresets = [
  { id: "default_summary", label: "Summarize impact" },
  { id: "risk_review", label: "Find risks" },
  { id: "claim_review", label: "Extract claims" },
] as const;

function latestJob(jobs: AiAnalysisJob[]) {
  return jobs[0] ?? null;
}

function statusTone(status: AiAnalysisJob["status"]) {
  if (status === "succeeded") return "ok";
  if (status === "failed" || status === "cancelled") return "danger";
  if (status === "queued" || status === "running") return "warn";
  return "neutral";
}

export function FeedAiAnalysisPanel({
  feedItem,
  jobs,
  error,
  providerConfigured,
  requestInFlight,
  onStart,
  onRetry,
}: FeedAiAnalysisPanelProps) {
  const { text } = useLocale();
  const [customQuestion, setCustomQuestion] = useState("");
  const [modalOpen, setModalOpen] = useState(false);
  const job = latestJob(jobs);
  const isBusy = requestInFlight || job?.status === "queued" || job?.status === "running";
  const canAnalyze = providerConfigured && !isBusy;

  function startPreset(promptPresetId: string) {
    void onStart(feedItem, promptPresetId);
  }

  function submitCustomQuestion(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmedQuestion = customQuestion.trim();
    if (!trimmedQuestion) return;

    void onStart(feedItem, undefined, trimmedQuestion);
    setCustomQuestion("");
  }

  return (
    <DetailSection
      ariaLabel={text("AI analysis")}
      aside={job ? <StatusPill tone={statusTone(job.status)}>{text(job.status)}</StatusPill> : null}
      description={text("Source-grounded decision support for the selected feed item.")}
      icon={<Sparkles size={15} />}
      title={text("AI analysis")}
    >
      {!providerConfigured ? (
        <p className="ai-analysis-empty">
          {text("Configure a general AI provider in Settings before running analysis.")}
        </p>
      ) : (
        <>
          {job?.result && !modalOpen ? (
            <article className="ai-analysis-preview" aria-label={text("AI analysis result")}>
              <span className="detail-launcher-status">
                {text("Significance")}: {text(job.result.significance)}
              </span>
              <p>{job.result.summary}</p>
            </article>
          ) : null}
          <div className="detail-launcher">
            <span className="detail-launcher-status">
              {job?.result
                ? text("Open the analysis for reasoning, tags, and sources.")
                : text("Run a preset prompt or ask a custom question about this item.")}
            </span>
            <Button className="compact-button" onClick={() => setModalOpen(true)} variant="primary">
              <Sparkles size={15} />
              {job?.result ? text("View analysis") : text("Analyze with AI")}
            </Button>
          </div>
        </>
      )}

      {error && !modalOpen ? (
        <p className="error-text">{text("AI analysis command failed")}: {error}</p>
      ) : null}

      <Modal
        ariaLabel={text("AI analysis")}
        onClose={() => setModalOpen(false)}
        open={modalOpen}
        title={text("AI analysis")}
        footer={
          <Button className="compact-button" onClick={() => setModalOpen(false)}>
            {text("Close")}
          </Button>
        }
      >
        <div className="ai-analysis-review">
          {job ? (
            <div className="kpi-extraction-status" aria-label={text("Analysis status")}>
              <StatusPill tone={statusTone(job.status)}>{text(job.status)}</StatusPill>
            </div>
          ) : null}
          <div className="ai-analysis-presets" aria-label={text("Suggested AI prompts")}>
            {promptPresets.map((preset) => (
              <Button
                className="compact-button"
                disabled={!canAnalyze}
                key={preset.id}
                onClick={() => startPreset(preset.id)}
              >
                <Bot size={15} />
                {text(preset.label)}
              </Button>
            ))}
          </div>
          <form className="ai-analysis-question" onSubmit={submitCustomQuestion}>
            <TextareaField
              label={text("Custom question")}
              aria-label={text("AI custom question")}
              disabled={!canAnalyze}
              onChange={(event) => setCustomQuestion(event.target.value)}
              placeholder={text("Ask about impact, risks, management claims, or follow-up questions")}
              rows={3}
              value={customQuestion}
            />
            <Button
              className="compact-button"
              disabled={!canAnalyze || !customQuestion.trim()}
              type="submit"
              variant="primary"
            >
              <Send size={15} />
              {text("Analyze")}
            </Button>
          </form>

          {job?.result ? (
            <article className="ai-analysis-result" aria-label={text("AI analysis result")}>
              <div className="ai-analysis-result-meta">
                <span>{text("Significance")}: {text(job.result.significance)}</span>
                <span>{job.providerId}</span>
                <span>{job.model}</span>
              </div>
              <p>{job.result.summary}</p>
              {job.result.reasoning ? <p className="ai-analysis-reasoning">{job.result.reasoning}</p> : null}
              {job.result.tags.length > 0 ? (
                <div className="ai-analysis-tags" aria-label={text("AI analysis tags")}>
                  {job.result.tags.map((tag) => (
                    <span key={tag}>{tag}</span>
                  ))}
                </div>
              ) : null}
              {job.result.sourceReferences.length > 0 ? (
                <div className="ai-analysis-sources" aria-label={text("AI analysis source references")}>
                  {job.result.sourceReferences.map((source) => (
                    <a href={source.sourceUrl} key={source.id} rel="noreferrer" target="_blank">
                      {source.label ?? source.sourceUrl}
                    </a>
                  ))}
                </div>
              ) : null}
            </article>
          ) : null}

          {job?.status === "failed" ? (
            <div className="ai-analysis-error">
              <p>{job.error ?? text("AI analysis failed.")}</p>
              <Button
                className="compact-button"
                disabled={requestInFlight}
                onClick={() => void onRetry(job.id, feedItem.id)}
              >
                <RefreshCw size={15} />
                {text("Retry")}
              </Button>
            </div>
          ) : null}

          {error ? <p className="error-text">{text("AI analysis command failed")}: {error}</p> : null}
        </div>
      </Modal>
    </DetailSection>
  );
}
