import { StatusChip } from "../../../ui";
import { pluralNoun, COMPANY_FORMS } from "../../../shared/locale/plural";
import type { StreamAggregate } from "../streamModel";
import type { RowContext, StreamPayload } from "./rowContext";
import { StreamRow } from "./streamRowKit";
import { StreamRowView, describePayload } from "./StreamRowView";
import { AttentionDismissAll, attentionEventIds, everyMemberFromAlertRule } from "./AttentionRow";

/**
 * A cross-company aggregate row (ADR 0087 dec. 1 + the 2026-07-23 amendment):
 * same-category ROUTINE company-rows ("×K spółek" wall fix), or urgent/notable
 * company-rows sharing one systemic cause (a repeated cause reads as one alarm —
 * PILNE when urgent, Uwaga when notable). Renders exactly like a group (header +
 * in-place expandable members), carrying the folded severity so the right severity
 * chip shows; the count chip is "×K spółek" and each member is one company's newest
 * item (keeping its own Review, so `j`/`k` traverses the aggregate and its members).
 * An attention aggregate also carries a two-step "Dismiss all" that dismisses every
 * member event.
 */
export function AggregateRow({
  aggregate,
  ctx,
}: {
  aggregate: StreamAggregate<StreamPayload>;
  ctx: RowContext;
}) {
  // Header anatomy comes from the newest collapsed company's newest item.
  const newestGroup = aggregate.members[0];
  const isAttention = aggregate.category === "attention";
  // Alert-rule indicator on the collapsed head only when every underlying member
  // (across all folded companies) is rule-fired.
  const descriptor = {
    ...describePayload(newestGroup.members[0].payload, ctx),
    ruleOrigin: everyMemberFromAlertRule(
      aggregate.members.flatMap((group) => group.members.map((member) => member.payload)),
    ),
  };
  const members = (
    <>
      <div className="today-group-members">
        {aggregate.members.map((group) => (
          <StreamRowView key={group.key} item={group.members[0]} ctx={ctx} variant="member" />
        ))}
      </div>
      {isAttention ? (
        <AttentionDismissAll
          eventIds={attentionEventIds(
            aggregate.members.flatMap((group) => group.members.map((member) => member.payload)),
          )}
          ctx={ctx}
        />
      ) : null}
    </>
  );
  return (
    <StreamRow
      category={aggregate.category}
      severity={aggregate.severity}
      descriptor={descriptor}
      groupChip={
        <StatusChip className="today-group-chip" tone="accent">
          {`×${aggregate.count} ${pluralNoun(ctx.locale, aggregate.count, COMPANY_FORMS)}`}
        </StatusChip>
      }
      expandable={members}
      disclosureLabel={ctx.text("Details")}
    />
  );
}
