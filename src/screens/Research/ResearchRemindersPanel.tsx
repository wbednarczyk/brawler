import { Check, Clock3, Plus, RotateCcw, Trash2 } from "lucide-react";
import type { ResearchReminder } from "../../api/researchTypes";
import { useFocusAfterRemove } from "../../shared/focus/focusAfterRemove";
import { ActionRow, Button, EmptyState, SectionHeader } from "../../ui";
import { formatReminderKind, formatReminderStatus } from "./researchFormatters";

type ResearchRemindersPanelProps = {
  reminders: ResearchReminder[];
  canAdd: boolean;
  reminderInFlight: boolean;
  onAdd: () => void;
  completeReminder: (reminderId: string) => void;
  snoozeReminder: (reminderId: string) => void;
  reopenReminder: (reminderId: string) => void;
  deleteReminder: (reminderId: string) => void;
  formatTimestamp: (value: string | null | undefined) => string;
  text: (value: string) => string;
};

export function ResearchRemindersPanel({
  reminders,
  canAdd,
  reminderInFlight,
  onAdd,
  completeReminder,
  snoozeReminder,
  reopenReminder,
  deleteReminder,
  formatTimestamp,
  text,
}: ResearchRemindersPanelProps) {
  const visibleReminders = reminders.slice(0, 6);
  // After deleting a reminder, keep focus in the queue by landing on the next
  // row's leading action button (ADR 0076 D9); the row article is not focusable.
  const { listRef } = useFocusAfterRemove<HTMLDivElement>(
    visibleReminders.map((reminder) => reminder.id),
    { rowSelector: ".research-reminder-row", focusSelector: ".icon-button" },
  );
  return (
    <section className="research-reminders" aria-label={text("Research reminders")}>
      <SectionHeader
        actions={
          <Button className="compact-button" disabled={reminderInFlight || !canAdd} onClick={onAdd}>
            <Plus size={14} />
            {text("Add reminder")}
          </Button>
        }
        className="research-section-review"
        description={text("Items that need a concrete follow-up action.")}
        meta={reminders.length}
        title={text("Review queue")}
        variant="accent"
      />
      <div className="research-reminder-list" ref={listRef}>
        {visibleReminders.map((reminder) => (
          <article className="research-reminder-row" key={reminder.id}>
            <div>
              <span>{text(formatReminderKind(reminder.reminderKind))}</span>
              <strong>{reminder.title}</strong>
              {reminder.dueAt ? <time dateTime={reminder.dueAt}>{formatTimestamp(reminder.dueAt)}</time> : null}
              {reminder.status !== "open" ? <em>{text(formatReminderStatus(reminder.status))}</em> : null}
            </div>
            <ActionRow className="research-reminder-actions">
              {reminder.status === "open" ? (
                <>
                  <Button
                    className="icon-button"
                    disabled={reminderInFlight}
                    onClick={() => completeReminder(reminder.id)}
                    title={text("Complete reminder")}
                  >
                    <Check size={15} />
                  </Button>
                  <Button
                    className="icon-button"
                    disabled={reminderInFlight}
                    onClick={() => snoozeReminder(reminder.id)}
                    title={text("Snooze reminder")}
                  >
                    <Clock3 size={15} />
                  </Button>
                </>
              ) : (
                <Button
                  className="icon-button"
                  disabled={reminderInFlight}
                  onClick={() => reopenReminder(reminder.id)}
                  title={text("Reopen reminder")}
                >
                  <RotateCcw size={15} />
                </Button>
              )}
              <Button
                className="icon-button"
                disabled={reminderInFlight}
                onClick={() => deleteReminder(reminder.id)}
                title={text("Delete reminder")}
              >
                <Trash2 size={15} />
              </Button>
            </ActionRow>
          </article>
        ))}
        {reminders.length === 0 ? <EmptyState>{text("No research reminders.")}</EmptyState> : null}
      </div>
    </section>
  );
}
