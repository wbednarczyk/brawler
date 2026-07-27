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

- Anything broken or jarring → GitHub issue (`bug` + labels) the same day; P1s block the release.
- UX friction that is not a bug → the milestone retro's **UX section** (journeys shorter/longer)
  and, when it names a defect class, the [guardrail-harvest](../.claude/skills/guardrail-harvest/SKILL.md) loop.
- The run itself is a release-prep step: note date + build + verdict in the release notes draft.

## Earlier exploratory checkpoints (ADR 0081)

Three cadences move real-app validation earlier than release closure. In every one, **automation collects mechanics + evidence; a human answers whether the journey is clear, useful, and trustworthy** — automation never prints a quality verdict. Windows-native behavior is the desktop authority.

| Cadence | When | Budget | Charter |
| --- | --- | --- | --- |
| **First vertical slice** | Before the slice expands | ~3–5 min | One exploratory question tied to the served journey |
| **Mid-milestone** | Once integration seams exist | ~10 min | Do the seams hang together for a real user? |
| **Release dogfood** | Release prep (the walk above) | ~15 min | The full journey walk |

**Run it** (needs an intentionally running/rebuilt Windows app; not part of `make check`):

```bash
BRAWLER_UX_JOURNEY="J1 morning review" BRAWLER_UX_CARD=<hex7> BRAWLER_UX_STAGE=vertical \
  make live-cycle LIVE_SPEC=tests/live/ux-checkpoint.live.spec.ts
```

The spec (`tests/live/ux-checkpoint.live.spec.ts`) drives the **mechanical** path — Today renders an attention stream **or** an explicit quiet state (never a blank pane or the error fallback), a visible Review action opens a company-scoped cockpit, return works — and writes evidence under gitignored `test-results/live/checkpoints/`. It is **not** a scripted happy-path replay to rubber-stamp; it frees the human to explore.

**The human charter** (the part automation cannot do) names: one **exploratory question**, findings graded **P1/P2/P3**, a **verdict** `proceed | revise | block`, and **which judgments stayed human**. A **P1 blocks expansion** of the slice. Lower-severity friction enters GitHub Issues (`bug` + labels) or the milestone retro's UX section honestly — never silently dropped. Only non-sensitive verdict metadata reaches the active GitHub issue/board card ([ADR 0090](adr/0090-github-canonical-forge-and-continuous-release.md) § privacy, continuing ADR 0081); screenshots + `manifest.json` stay local (the manifest carries a dataset **label**, never the DB path/contents). Details + privacy contract: [testing.md § Live drive](testing.md#live-drive-real-app-via-cdp).
