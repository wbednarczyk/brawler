import { ListChecks } from "lucide-react";

import type { ActivitySummary } from "../../../api/generated/ActivitySummary";
import { Button, Figure } from "../../../ui";
import { useLocale } from "../../locale";
import { formatListTimestamp } from "../../format/datetime";

export type ActivityIndicatorProps = {
  summary: ActivitySummary;
  onOpen: () => void;
};

// Topbar work-in-progress signal (ADR 0109 dec. 5/6, plan § D4 item 2): NEVER
// a danger tone, NEVER a failure count — that stays Today's job. `ListChecks`
// rather than the Sources pill's `Activity` glyph (both live in the same
// topbar cluster; reusing the identical icon there would read as one control
// twice).
export function ActivityIndicator({ summary, onOpen }: ActivityIndicatorProps) {
  const { locale, text } = useLocale();
  const { active, queued, lastFinishedAt } = summary;

  const title =
    active > 0
      ? `${active} ${text("running")} · ${queued} ${text("queued")}`
      : queued > 0
        ? `${queued} ${text("queued")}`
        : lastFinishedAt
          ? `${text("Last finished")}: ${formatListTimestamp(lastFinishedAt, locale)}`
          : text("Nothing in the background");

  return (
    <Button
      variant="icon"
      className={active > 0 ? "icon-button-spinning" : undefined}
      aria-label={text("Open activity")}
      onClick={onOpen}
      title={title}
    >
      <ListChecks size={18} aria-hidden="true" />
      {active > 0 ? (
        <Figure kind="count" value={active} />
      ) : queued > 0 ? (
        <Figure kind="count" value={queued} />
      ) : null}
    </Button>
  );
}
