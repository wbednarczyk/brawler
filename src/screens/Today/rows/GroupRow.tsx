import { StatusChip } from "../../../ui";
import {
  pluralNoun,
  EVENT_FORMS,
  RUN_FORMS,
  ITEM_FORMS,
  type PluralForms,
} from "../../../shared/locale/plural";
import type { StreamCategory, StreamGroup } from "../streamModel";
import type { RowContext, StreamPayload } from "./rowContext";
import { StreamRow } from "./streamRowKit";
import { StreamRowView, describePayload } from "./StreamRowView";
import { AttentionDismissAll, attentionEventIds, everyMemberFromAlertRule } from "./AttentionRow";

/**
 * The count-chip unit for a per-company group so "×4" is not opaque (owner
 * dogfooding 2026-07-23): attention groups count fired events, autopilot groups
 * count runs; every other category falls back to a generic item noun.
 */
function groupCountForms(category: StreamCategory): PluralForms {
  switch (category) {
    case "attention":
      return EVENT_FORMS;
    case "autopilot":
      return RUN_FORMS;
    default:
      return ITEM_FORMS;
  }
}

/**
 * A collapsed group of same category+company items (ADR 0087 dec. 1b). The
 * header row shows the newest member's anatomy plus a ×N count chip and a
 * disclosure that expands the full member list IN PLACE; each member renders as
 * its own compact row (with its own single Review action, so `j`/`k` traverses
 * members inline). The group's severity is its most-severe member's.
 */
export function GroupRow({
  group,
  ctx,
}: {
  group: StreamGroup<StreamPayload>;
  ctx: RowContext;
}) {
  const newest = group.members[0];
  const isAttention = group.category === "attention";
  // The header's alert-rule indicator reflects the WHOLE group, not just the
  // newest member: shown only when every member is rule-fired (owner dogfooding
  // 2026-07-23). Members render their own via `StreamRowView` when expanded.
  const descriptor = {
    ...describePayload(newest.payload, ctx),
    ruleOrigin: everyMemberFromAlertRule(group.members.map((member) => member.payload)),
  };
  const members = (
    <>
      <div className="today-group-members">
        {group.members.map((member) => (
          <StreamRowView key={member.id} item={member} ctx={ctx} variant="member" />
        ))}
      </div>
      {isAttention ? (
        <AttentionDismissAll
          eventIds={attentionEventIds(group.members.map((member) => member.payload))}
          ctx={ctx}
        />
      ) : null}
    </>
  );
  return (
    <StreamRow
      category={group.category}
      severity={group.severity}
      descriptor={descriptor}
      groupChip={
        <StatusChip className="today-group-chip" tone="accent">
          {`×${group.count} ${pluralNoun(ctx.locale, group.count, groupCountForms(group.category))}`}
        </StatusChip>
      }
      expandable={members}
      disclosureLabel={ctx.text("Details")}
    />
  );
}
