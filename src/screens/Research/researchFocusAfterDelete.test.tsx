import { describe, it, expect } from "vitest";
import { useState } from "react";
import { render, act } from "@testing-library/react";

import { ResearchQuestionsPanel } from "./ResearchQuestionsPanel";
import { ResearchRemindersPanel } from "./ResearchRemindersPanel";
import type { ResearchQuestion, ResearchReminder } from "../../api/researchTypes";

// These tests exercise only the render-side focus-after-destroy wiring (ADR
// 0076 D9): when the rendered list shrinks (however the delete was confirmed),
// focus moves to the next row. The removal is driven through the controlled
// parent rather than the delete control, so the behavior composes with the
// U4 InlineConfirm/Toast confirmation mechanics layered on the same button.

function question(id: string, title: string): ResearchQuestion {
  return {
    id,
    scopeType: "company",
    scopeId: "company_1",
    title,
    body: "",
    status: "open",
    closedAt: null,
    createdAt: "2026-06-01T10:00:00Z",
    updatedAt: "2026-06-01T10:00:00Z",
  };
}

function reminder(id: string, title: string): ResearchReminder {
  return {
    id,
    scopeType: "company",
    scopeId: "company_1",
    companyId: "company_1",
    reminderKind: "manual_research",
    sourceType: null,
    sourceId: null,
    title,
    body: "",
    dueAt: null,
    status: "open",
    snoozedUntil: null,
    completedAt: null,
    dismissedAt: null,
    createdAt: "2026-06-01T10:00:00Z",
    updatedAt: "2026-06-01T10:00:00Z",
  };
}

describe("ResearchQuestionsPanel focus-after-delete", () => {
  function Harness({ onReady }: { onReady: (remove: (id: string) => void) => void }) {
    const [questions, setQuestions] = useState([
      question("q1", "Alpha"),
      question("q2", "Bravo"),
      question("q3", "Charlie"),
    ]);
    onReady((id) => setQuestions((current) => current.filter((q) => q.id !== id)));
    return (
      <ResearchQuestionsPanel
        questions={questions}
        selectedQuestion={null}
        selectedQuestionId={null}
        questionLinks={[]}
        canAdd
        questionInFlight={false}
        onAdd={() => {}}
        setSelectedQuestionId={() => {}}
        updateQuestionStatus={() => {}}
        deleteQuestion={() => {}}
        unlinkEvidence={() => {}}
        text={(value) => value}
      />
    );
  }

  it("moves focus to the next question's select control when a row is removed", () => {
    let remove: (id: string) => void = () => {};
    const { container } = render(<Harness onReady={(fn) => (remove = fn)} />);

    // Focus sits on the middle row's select button (in-list) as it would after
    // confirming its delete; removing that row should carry focus onward.
    const rows = () => container.querySelectorAll<HTMLElement>(".research-question-row");
    (rows()[1].querySelector(".research-question-row-main") as HTMLElement).focus();
    act(() => remove("q2"));

    expect(rows()).toHaveLength(2);
    // The row now in slot 1 is Charlie; focus lands on its select button.
    expect(document.activeElement).toBe(rows()[1].querySelector(".research-question-row-main"));
    expect(rows()[1]).toHaveTextContent("Charlie");
  });
});

describe("ResearchRemindersPanel focus-after-delete", () => {
  function Harness({ onReady }: { onReady: (remove: (id: string) => void) => void }) {
    const [reminders, setReminders] = useState([
      reminder("r1", "One"),
      reminder("r2", "Two"),
      reminder("r3", "Three"),
    ]);
    onReady((id) => setReminders((current) => current.filter((r) => r.id !== id)));
    return (
      <ResearchRemindersPanel
        reminders={reminders}
        canAdd
        reminderInFlight={false}
        onAdd={() => {}}
        completeReminder={() => {}}
        snoozeReminder={() => {}}
        reopenReminder={() => {}}
        deleteReminder={() => {}}
        formatTimestamp={() => ""}
        text={(value) => value}
      />
    );
  }

  it("moves focus to the next reminder's leading action when a row is removed", () => {
    let remove: (id: string) => void = () => {};
    const { container } = render(<Harness onReady={(fn) => (remove = fn)} />);

    const rows = () => container.querySelectorAll<HTMLElement>(".research-reminder-row");
    // The row article is not focusable; focus its leading action (in-list).
    (rows()[1].querySelector(".icon-button") as HTMLElement).focus();
    act(() => remove("r2"));

    expect(rows()).toHaveLength(2);
    // The row now in slot 1 is Three; focus lands on its first action button.
    expect(document.activeElement).toBe(rows()[1].querySelector(".icon-button"));
    expect(rows()[1]).toHaveTextContent("Three");
  });
});
