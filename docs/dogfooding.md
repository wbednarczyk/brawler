# Dogfooding script (per release)

A ~15-minute owner walk of the real app with the real database before every release
(v0.50 U12, [ADR 0074](adr/0074-ux-journeys-and-anti-rot.md)). The journey E2E suite
proves the paths work against the mock; this run proves they work — and feel right —
against reality. Run it after `make release-prepare` (or before `make release`), on the
platform you actually use (Windows build for hands-on, per engineering-workflow).

## Script

Walk the current portion of each journey ([ux-journeys.md](ux-journeys.md)); tick, note
anything that felt slow, confusing, or wrong. A feeling counts as a finding.

| # | Journey | Walk | Minutes |
|---|---------|------|---------|
| 1 | J1 morning review | Open the app → triage the Today stream → open one item → back | 2 |
| 2 | J2 report published | Latest report feed item → extract/review KPIs → confirm one → check Fundamentals | 3 |
| 3 | J3 onboarding | Add (or dry-run) a company via registry lookup → first note "why watching" | 2 |
| 4 | J4 season prep | Report Season → open one pre-report card → review → mark prepared | 2 |
| 5 | J5 claim verification | Claims queue → verdict one due claim against its evidence | 1 |
| 6 | J6 buy/pass (current portion) | Company workspace → fundamentals + quality scorecard read-through | 2 |
| 7 | J7 weekly review | Events week calendar → watchlist overview → note next week's dates | 2 |
| 8 | Sweep | Switch theme + language once; resize to a quarter-ultrawide window; glance for overflow/clipping | 1 |

## Recording findings

- Anything broken or jarring → Radicle issue (`bug` + labels) the same day; P1s block the release.
- UX friction that is not a bug → the milestone retro's **UX section** (journeys shorter/longer)
  and, when it names a defect class, the [guardrail-harvest](../.claude/skills/guardrail-harvest/SKILL.md) loop.
- The run itself is a release-prep step: note date + build + verdict in the release notes draft.
