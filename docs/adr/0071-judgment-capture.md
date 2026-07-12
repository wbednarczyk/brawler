# ADR 0071: Judgment Capture — Early Decision Journal and Pre-Report Expectations

Status: Accepted

An investor's own judgment history cannot be backfilled: every decision made and every expectation held before the tooling records them is lost as data. The full thesis workbench + decision journal is designed (ADR 0043) but scheduled late; this ADR pulls a minimal, forward-compatible slice ahead so the record starts accumulating now, and adds a sibling capture the workbench did not cover — expectations written down before a report lands.

## Context

- ADR 0043 specifies the decision journal inside the thesis workbench (now v0.64). Waiting means months of unrecorded decisions — the exact data the future calibration loop (north star NS2) needs.
- The report-season cockpit (ADR 0044) prepares the user before results but records nothing about what they expected, so hindsight bias goes unchecked.

## Decision

1. **`decision_entries` (early slice of ADR 0043's journal, unchanged in design)**: company, decision kind (`buy` | `pass` | `keep_watching` | `sell_note` — recorded actions/judgments, not advice), Markdown rationale, evidence links (feed item / report document / note / claim / valuation-when-available), decided-at date. Immutable once saved (append corrections as follow-up entries). Surfaces: a per-company journal section + a global chronological list; entries join the research timeline. The v0.64 workbench builds on this table — no migration away, only extension (outcome review, thesis links).
2. **`report_expectations`**: for an upcoming report occurrence (report-season cockpit integration): free-text stance plus optional per-metric expectations (metric key, comparator, value). After the report's facts are confirmed, a read model composes expectation-vs-actual for review — the user records their own verdict; the app never scores judgment automatically.
3. Both entities are owner-durable: import/export coverage in the unified bundle *(deferred with the whole bundle-v2 epic to `v0.67` at the 2026-07-11 planning pass — schema stability first; whole-DB rotating backups cover safety meanwhile)*, retention-exempt, and provenance-stamped like claims (ADR 0040 precedent).

## Consequences

- Data for the calibration loop starts accumulating from v0.51 instead of v0.64+.
- Report-season journey gains a "write expectations" step; the post-report journey gains expectation-vs-actual review.
- Decision-support boundary intact: the app records the user's judgments and mirrors them back; it produces no recommendations (ADR 0042 posture).
- ADR 0043 is amended to note the journal's early landing; its workbench design is unchanged.
