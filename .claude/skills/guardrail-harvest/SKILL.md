---
name: guardrail-harvest
description: Convert a flagged defect's class into a durable guardrail — a precise automated gate or a documented rule + checklist line — in the same change. Use whenever the user, a review, or a failing gate flags a defect.
---

# Guardrail Harvest

Use this workflow whenever a defect is flagged — by the user, by a code review, by a failing gate, or by noticing your own mistake — and at the close of every slice/epic. Its job is to convert the **class** of defect into a durable rule or automated check so the next agent (any agent) cannot repeat it. Policy: [ADR 0045](../../../docs/adr/0045-guardrail-harvest-loop.md), extending [ADR 0038](../../../docs/adr/0038-enforcement-as-guardrails.md).

This is not optional polish. A flagged defect is not "done" when the instance is fixed — it is done when the class is closed.

## Steps

1. **Name the class, not the instance.** Write one sentence: "Agents tend to <do X> when they should <do Y>." Generalize past the specific file. (Example: "Agents render a qualified ticker as plain text instead of using `TickerLabel`.")

2. **Find the root cause — why was nothing stopping it?** Usually one of: an undocumented convention, a component/rule that lives somewhere the self-check doesn't point at, or a missing gate. Fix that blind spot, not just the symptom.

3. **Choose the enforcement (gate vs doc) deliberately.** Per [ADR 0045](../../../docs/adr/0045-guardrail-harvest-loop.md) Decision 2:
   - **Automated gate** (lint rule, guard/contract test, type, layout-contract test) — only if the violation is **precisely** detectable, i.e. you can write a predicate that fires on the wrong thing and is clean across the *entire existing codebase*. Run it repo-wide before committing to it.
   - **If a gate would produce false positives, do NOT add it.** A noisy gate gets disabled and erodes the whole posture. Encode the lesson as a **documented rule** in the canonical doc for the area, plus a self-check / review-checklist item.
   - When unsure, prototype the gate, run it across `src/**`, and look at every hit. If legitimate code is flagged, fall back to a doc rule.

4. **Implement the guardrail in the same change as the fix.** Land the instance fix and its class guardrail together.

5. **Put it where every agent reads it.** Durable rules go in `CLAUDE.md`, the relevant canonical doc (`docs/ui-authoring.md`, `docs/contracts.md`, `docs/data-model.md`, …), an ADR, or an automated check — never only in agent-private memory ([ADR 0045](../../../docs/adr/0045-guardrail-harvest-loop.md) Decision 4). Link the new rule/gate from the canonical doc so it is discoverable. **If the lesson is a "remember to check X" rather than a code rule, add it as a line to the [Definition of Done](../../../docs/engineering-workflow.md) checklist** — that checklist is the living home for harvested verification steps.

6. **State the harvest in your handoff.** Note what class was closed and how (gate or doc), so the user can see the loop ran.

## Anti-patterns

- Fixing only the instance the user pointed at and moving on.
- Adding a broad lint/test gate that flags legitimate code "to be safe" — this is the failure this loop exists to prevent.
- Writing the lesson into private memory instead of the repo.
- Deferring the guardrail to "later" — later never comes; the next agent repeats the mistake first.
