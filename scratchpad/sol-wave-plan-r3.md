Verdict: **DO-NOT-SHIP** pending one remaining contract contradiction. Everything else is implementation-ready.

1. **PARTIAL — measured period capacity.** The core design is fixed, but stale hard-coded expectations remain: “8 newest / +4” and S=4 ([plan:30](/home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:30), [plan:39](/home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:39), [plan:40](/home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:40)). More importantly, clamping capacity to minimum 2 cannot satisfy the S-tier geometric contract for Pozycje, where one period occupies roughly 240px ([plan:16](/home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:16)). Fix: allow capacity 1, use an exact/conservative width rather than `≈`, and make all expected counts derive from measured capacity.

2. **FIXED — Escape policy.** Picker and palette policies are distinct; consumption includes both propagation mechanisms; Modal behavior and regression tests are explicit ([plan:13](/home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:13)).

3. **FIXED — canonical Claims target.** Main-list row exclusively owns scrolling, persistence, and `aria-current`; the queue twin is visual-only, with timer/“flash” removal and reopen coverage ([plan:21](/home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:21)).

4. **FIXED — corner-control a11y.** Header and control have independent names, expanded wording is defined, and `ActionButton kind="control"` plus keyboard/localization tests are specified ([plan:16](/home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:16)).

5. **FIXED — palette migration.** The headless/domain boundary, host-specific Escape behavior, controller edge cases, exact reddening tests, and non-coverage of `paletteCopy` are all clear ([plan:13](/home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:13)).

6. **PARTIAL — slicing/baselines.** The gated #6 commit and palette visual cell are fixed. Minor ownership omissions remain: assign `Modal.tsx`, `Modal.test.tsx`, and `src/ui/index.ts` explicitly to S2 ([plan:46](/home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:46)). These do not independently block implementation.

Once finding 1’s minimum capacity and stale fixed-count language are corrected, verdict becomes **SHIP**.

Codex session ID: 01a06d22-21f7-7790-9c2f-fe4545e96a7a
Resume in Codex: codex resume 01a06d22-21f7-7790-9c2f-fe4545e96a7a
