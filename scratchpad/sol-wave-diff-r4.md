The resize/measurement state machine does not show an oscillation path: the variant is captured before `recompute`, layout effects settle the hidden pass before paint, and a zero-period render cannot enter measuring. The browser oracle uses host-provided total-period metadata but independently reimplements the width/capacity calculation; it does not consume `visibleCount` or `data-visible-periods`. I did find that the asserted comment cleanup is incomplete, and one claimed key-regression test is not actually present.

Codex session ID: 01a07c1c-5e62-7f52-b585-4265e8fb0227
Resume in Codex: codex resume 01a07c1c-5e62-7f52-b585-4265e8fb0227
