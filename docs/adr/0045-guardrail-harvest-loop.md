# ADR 0045: Guardrail Harvest — Turning Flagged Defects Into Rules and Gates

Status: Accepted

This ADR makes **continuous learning** a first-class, enforced part of the workflow: every time a defect is flagged (by the user, by review, by a failing gate, or by an agent noticing its own mistake), the **class** of defect must be converted into a durable rule or automated check in the same session — so the next agent cannot repeat it. It extends [ADR 0038](0038-enforcement-as-guardrails.md) ("when you add a capability, add the gate that keeps future changes from violating it") from *new capabilities* to *discovered mistakes*.

## Context

Agents work one epic at a time, with limited context, and rotate across sessions. Without a feedback loop, the same class of mistake recurs every epic and the user is forced to babysit each one. Two concrete examples from the `v0.43.0` report-season cockpit, both caught only by the user after the fact:

1. A new screen was assembled from the primitive catalog in the abstract (double-wrapped panel chrome, no padded scroll body) instead of by copying a sibling screen's scaffold — so the view read "raw" next to the others.
2. A qualified ticker was rendered as plain text, missing the `TickerLabel` domain component every other screen uses — because `TickerLabel` lives in `src/shared/components`, outside the `src/ui` barrel the authoring self-check pointed at.

Both were **blind spots with no gate**: the authoring contract documented "which primitive for which shape" but not the screen scaffold convention or the domain-component layer, and nothing automated halted the wrong move. That is the [ADR 0038](0038-enforcement-as-guardrails.md) failure mode.

## Decisions

### 1. Every flagged defect triggers a guardrail harvest, in the same change

When a defect is flagged, before the slice is considered done the agent must convert the **class** (not just the instance) into one of:

- an **automated gate** (lint rule, contract/guard test, type, layout-contract test) — preferred when the violation is precisely and cheaply detectable; or
- a **documented rule** in the canonical doc for the area (e.g. `docs/ui-authoring.md`, a contract, a data-model rule) and, where useful, a **review-checklist item** — when correctness depends on context/judgment an automated rule cannot capture.

The fix to the instance and the guardrail for the class land together. This is mandatory, not aspirational — see `AGENTS.md` → "Guardrail harvest".

### 2. Choose the right enforcement: a precise gate, or a doc rule — never a noisy gate

A gate is only worth adding if it is **precise**: it fires on the wrong thing and (almost) nothing else. A broad gate that flags legitimate code (false positives) is *worse than no gate* — it pressures the next agent to disable, baseline, or `--no-verify` it, eroding the whole enforcement posture ([ADR 0038](0038-enforcement-as-guardrails.md)).

Worked example from this ADR's own creation: a lint rule banning `qualifiedTicker` in JSX/strings was prototyped and **rejected** — it flagged legitimate `aria-label`/`title`/tooltip strings and `<option>` text (where `TickerLabel` cannot render). The ticker lesson was therefore encoded as a **documented rule + self-check item** in `docs/ui-authoring.md`, not a gate. The screen-scaffold lesson likewise became a documented scaffold + self-check step. Decision rule: **if you cannot express the violation as an AST/test predicate that is clean across the existing codebase, encode it as a doc rule + checklist, not a gate.**

### 3. The harvest is a repeatable ritual

The steps are captured in the repo-owned workflow `.agents/skills/guardrail-harvest.md` so any agent (Claude, Codex, …) runs the same loop: name the defect class → pick gate vs doc → implement the guardrail → link it from the relevant canonical doc/ADR. The ritual runs at slice/epic close and whenever the user flags something.

### 4. Durable guardrails live in the repo, not agent-private memory

Consistent with `AGENTS.md` "Standing Agent Guidance": harvested rules go into `AGENTS.md`, a canonical doc, an ADR, or an automated check — never only into an agent's private memory, so every agent benefits.

## Consequences

- The cost of a flagged defect is paid once: the class is closed, not re-explained each epic. The user stops babysitting recurring mistakes.
- Enforcement stays trustworthy because gates are precise by policy; judgment-dependent rules are documented and reviewed rather than forced into brittle automation.
- `docs/ui-authoring.md` gains a screen-scaffold section, a domain-components catalog, and self-check steps; this ADR and the `guardrail-harvest` skill record the loop itself.

## Out of Scope

- A separate "lessons" datastore. The loop deliberately writes into the existing canonical docs/ADRs/checks, not a parallel log that would drift.
- Retroactively gating every historical inconsistency. The loop is forward-looking: harvest when a defect is flagged.
