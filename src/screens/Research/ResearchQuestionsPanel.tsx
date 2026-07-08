import { Plus, Trash2, X } from "lucide-react";
import { useState } from "react";
import type {
  EvidenceLink,
  ResearchQuestion,
  ResearchQuestionStatus,
} from "../../api/researchTypes";
import { useFocusAfterRemove } from "../../shared/focus/focusAfterRemove";
import { Button, EmptyState, InlineConfirm, SectionHeader } from "../../ui";
import { formatEvidenceType, formatQuestionStatus } from "./researchFormatters";

type ResearchQuestionsPanelProps = {
  questions: ResearchQuestion[];
  selectedQuestion: ResearchQuestion | null;
  selectedQuestionId: string | null;
  questionLinks: EvidenceLink[];
  canAdd: boolean;
  questionInFlight: boolean;
  onAdd: () => void;
  setSelectedQuestionId: (questionId: string | null) => void;
  updateQuestionStatus: (questionId: string, status: ResearchQuestionStatus) => void;
  deleteQuestion: (questionId: string) => void;
  unlinkEvidence: (linkId: string) => void;
  text: (value: string) => string;
};

export function ResearchQuestionsPanel({
  questions,
  selectedQuestion,
  selectedQuestionId,
  questionLinks,
  canAdd,
  questionInFlight,
  onAdd,
  setSelectedQuestionId,
  updateQuestionStatus,
  deleteQuestion,
  unlinkEvidence,
  text,
}: ResearchQuestionsPanelProps) {
  // Cascading (ADR 0076 D5): deleting a question drops its evidence links, so
  // confirm in place rather than via a native dialog.
  const [confirmDeleteQuestionId, setConfirmDeleteQuestionId] = useState<string | null>(null);
  // Deleting a question row moves focus to the next row's select control rather
  // than dropping it on <body> (ADR 0076 D9). The row div is not focusable, so
  // land on its primary select button.
  const { listRef } = useFocusAfterRemove<HTMLDivElement>(
    questions.map((question) => question.id),
    { rowSelector: ".research-question-row", focusSelector: ".research-question-row-main" },
  );
  return (
    <section className="research-questions" aria-label={text("Research questions")}>
      <div className="research-question-strip">
        <SectionHeader
          className="research-section-questions"
          description={text("Open questions you are actively tracking.")}
          meta={questions.length}
          title={text("Research questions")}
          variant="accent"
        />
        <Button className="compact-button" disabled={!canAdd} onClick={onAdd}>
          <Plus size={15} />
          {text("Add question")}
        </Button>
      </div>
      {/* A DELIBERATE horizontal card strip (grid-auto-flow: column) — it scrolls
          inside its own bounded container; data-hscroll exempts it from the
          panel-overflow layout gate. */}
      <div className="research-question-card-list" data-hscroll ref={listRef}>
        {questions.map((question) => (
          <div
            className={question.id === selectedQuestionId ? "research-question-row selected" : "research-question-row"}
            key={question.id}
          >
            <button
              className="research-question-row-main"
              type="button"
              onClick={() => setSelectedQuestionId(question.id)}
            >
              <strong>{question.title}</strong>
              <span>{text(formatQuestionStatus(question.status))}</span>
            </button>
            {confirmDeleteQuestionId === question.id ? (
              <InlineConfirm
                cancelLabel={text("Cancel")}
                confirmLabel={text("Delete")}
                disabled={questionInFlight}
                onCancel={() => setConfirmDeleteQuestionId(null)}
                onConfirm={() => {
                  setConfirmDeleteQuestionId(null);
                  deleteQuestion(question.id);
                }}
              >
                {text("Delete research question")}
              </InlineConfirm>
            ) : (
              <button
                aria-label={text("Delete research question")}
                className="research-question-delete"
                disabled={questionInFlight}
                title={text("Delete research question")}
                type="button"
                onClick={() => setConfirmDeleteQuestionId(question.id)}
              >
                <Trash2 size={14} />
              </button>
            )}
          </div>
        ))}
        {questions.length === 0 ? <EmptyState>{text("No research questions yet.")}</EmptyState> : null}
      </div>
      {selectedQuestion ? (
        <div className="research-active-question">
          <div className="research-active-question-main">
            <span>{text("Active question")}</span>
            <strong>{selectedQuestion.title}</strong>
            {selectedQuestion.body ? <p>{selectedQuestion.body}</p> : null}
            <div className="research-linked-evidence">
              <span>{text("Linked evidence")}</span>
              {questionLinks.map((link) => (
                <button key={link.id} type="button" onClick={() => unlinkEvidence(link.id)}>
                  <X size={13} />
                  {text(formatEvidenceType(link.fromType === "research_question" ? link.toType : link.fromType))}
                </button>
              ))}
              {questionLinks.length === 0 ? <small>{text("No linked evidence yet.")}</small> : null}
            </div>
          </div>
          <div className="research-question-actions">
            <Button
              className="compact-button"
              disabled={questionInFlight || selectedQuestion.status === "answered"}
              onClick={() => updateQuestionStatus(selectedQuestion.id, "answered")}
            >
              {text("Answered")}
            </Button>
            <Button
              className="compact-button"
              disabled={questionInFlight || selectedQuestion.status === "closed"}
              onClick={() => updateQuestionStatus(selectedQuestion.id, "closed")}
            >
              {text("Close")}
            </Button>
            <Button
              className="compact-button"
              disabled={questionInFlight || selectedQuestion.status === "open"}
              onClick={() => updateQuestionStatus(selectedQuestion.id, "open")}
            >
              {text("Reopen")}
            </Button>
            <Button className="compact-button" onClick={() => setSelectedQuestionId(null)}>
              {text("Clear selection")}
            </Button>
          </div>
        </div>
      ) : null}
    </section>
  );
}
