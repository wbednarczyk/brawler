# ADR 0014: Portal Analiz Authenticated Source Policy

## Status

Accepted

## Context

Milestone 8 includes authenticated private research sources, starting with Portal Analiz if feasible. This source is different from GPW, Bankier RSS, and other public sources because it requires the user's own paid account and likely does not provide a documented public API for local desktop ingestion.

Brawler is local-first and privacy-preserving. Adding authenticated source support must not create a generic website automation feature, leak credentials, or normalize broad scraping of paid content. The app also needs enough private-research support to make v1 useful without weakening source policy or attribution.

## Decision

Portal Analiz is accepted as a v1 authenticated private research source candidate only under a dedicated adapter with strict boundaries.

The v1 adapter may proceed only if implementation research confirms an acceptable user-account usage path. If Portal Analiz terms, technical controls, or operational behavior make automated access unacceptable, the implementation must stop and return to product discussion instead of bypassing controls.

Implementation boundaries:

- Use a dedicated `portal-analiz` adapter; do not add generic "log in and scrape a website" infrastructure.
- Treat the source as `authenticated_research` with authenticated or paywalled access status in Sources.
- Store credentials, tokens, cookies, or session secrets only through the OS keychain or OS-protected session storage approved by a later implementation note.
- Never store secrets in SQLite, YAML exports, backups, logs, screenshots, test samples, Nix files, `.envrc`, or build artifacts.
- Preserve Portal Analiz attribution and original source URLs on every stored item.
- Store only the minimum source-derived content needed for local research workflows. Do not bulk archive the site.
- Limit scope to user-approved companies or watchlists. Do not crawl unrelated companies, categories, search results, or historical archives by default.
- Prefer user-initiated refresh first. Scheduled refresh is allowed only after the manual workflow is stable and must use conservative limits.
- Use conservative request limits: serialized requests, visible source status, backoff on errors, and no immediate startup polling.
- Tests must use local test samples and mocks; default checks must not require live Portal Analiz credentials.
- The adapter must surface authentication failures, paywall/session expiry, rate-limit warnings, and partial failures in Sources.

Non-goals for v1:

- Hosted activation, account sharing, or credential syncing.
- Browser automation that hides a full website session from the user.
- CAPTCHA bypass, paywall bypass, credential reuse beyond the user's own local app, or automated access after account/session revocation.
- Redistribution of Portal Analiz content through exports meant for sharing.

## Consequences

- M8 may claim the Portal Analiz policy prerequisite without implementing the adapter.
- A later implementation slice still needs source research, UI credential/session flow, keychain integration, storage mapping, and tests.
- If Portal Analiz cannot be implemented within these limits, the v1 scope must either use manual note/import workflows for that research source or pick another authenticated source after a new review.
- Source adapter contracts must continue distinguishing official reports, public media, analysis, and authenticated private research.
- Export/backup work must keep private-source secrets excluded and may need additional controls for source-derived paywalled content.
