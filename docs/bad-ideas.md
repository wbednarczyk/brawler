# Bad Ideas (Retreat Ledger)

Anti-archaeology rule: when an approach is tried and retired, this file gets **one line** — what
· when · why in ~5 words · ADR link. The history (context, evidence, alternatives) lives in the
ADR; this file exists so a dead end is scannable and never re-proposed from scratch. Do not expand
rows into prose here — add the detail to the ADR instead.

| What | When | Why (short) | ADR |
| --- | --- | --- | --- |
| In-app AI analysis layer | 2026-07 | BYOA via MCP beats bundled inference | [0084](adr/0084-retire-in-app-ai-layer.md) |
| Deterministic PDF fact extraction | 2026-07-21 | every issuer's PDF a new parser fight | [0086](adr/0086-aggregator-primary-fundamentals.md) |
| `html_positional` extraction tier | 2026-08-05 | 5.6%/2.0% precision/recall, currency-blind | [0095](adr/0095-retire-html-positional-tier.md) |
| Epic-as-gate (`make check-epic`) | 2026-08-05 | continuous release: PR is the only gate | [0096](adr/0096-quality-gate-architecture-under-continuous-release.md) |
| Blocking local git hooks (pre-commit/pre-push) | 2026-08-05 | a local commit ships nothing under continuous release | [0096](adr/0096-quality-gate-architecture-under-continuous-release.md) |
| Committed reference-machine bench baseline | 2026-08-05 | wall-clock is machine-dependent, never a hard gate | [0096](adr/0096-quality-gate-architecture-under-continuous-release.md) |
| System-event toasts (persistent attention pop-ups) | 2026-08-06 | batch-arriving chrome nobody used | [0097](adr/0097-toasts-are-action-feedback-only.md) |
| CBF 0.30/0.98 extraction ratchet | 2026-08-05 | superseded by #182 ground-truth measurement | [0095](adr/0095-retire-html-positional-tier.md) |
| Facts review queue (ratification workflow) | 2026-07-21 | manual review kills usability; facts are review-free | [0086](adr/0086-aggregator-primary-fundamentals.md) dec. 5 |
| AI-era witness seam (aggregator-as-witness) | 2026-07-21 | aggregator promoted primary, AI gap-fill retired | [0084](adr/0084-retire-in-app-ai-layer.md) |
