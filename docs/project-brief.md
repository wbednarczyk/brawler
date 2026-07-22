# Brawler Project Brief

Product-intent detail behind the digest in [CLAUDE.md](../CLAUDE.md) § Product Intent. The documentation map lives in CLAUDE.md § Required Reading (one home); engineering/product principles live in CLAUDE.md, [engineering-workflow.md](engineering-workflow.md), and [product-spec.md](product-spec.md).

## Product Intent

Brawler is a personal investor newsfeed application. The first user is an individual investor who follows public companies across multiple markets and wants one place to review important company-specific information.

The first production direction is a local-first Windows desktop app that can later compile for other operating systems and architectures. The app should be built with monetization optionality, but the first version should stay useful for personal use without cloud infrastructure.

## V1 Goal

Build an investor workspace for company news, official reports, and ticker-specific notes:

- maintain multiple watchlists of companies
- pull GPW-focused official reports and selected public/RSS news sources
- show a chronological feed with filters, unread state, source attribution, and company grouping
- maintain a notebook for each ticker
- create notes directly from feed items and transcripts
- track management claims or promises across future quarters
- run local ingestion while the desktop app is open
- gather fundamentals 100% deterministically (layered structured-first extraction, validated or flagged — [ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md))
- expose the whole research domain through a local MCP port so the user's own agent supplies intelligence (**BYOA — bring your own agent**; the in-app AI analysis layer is retired, [ADR 0084](adr/0084-retire-in-app-ai-layer.md)); the only in-app AI is video transcription (data acquisition, provider-neutral trait)

V1 is not a portfolio tracker, trading tool, or investment recommendation engine.

## Target Markets

V1 prioritizes excellent GPW support. Later adapters should support US and European markets without changing the core feed model.

Initial source priorities:

- GPW ESPI/EBI official reports
- selected Polish public/RSS media sources where usage is allowed
- future adapter candidates: SEC EDGAR APIs, Nasdaq RSS feeds, major European exchange sources

## Open-Core Direction

Brawler uses an open-core posture. The desktop core is open source under the Mozilla Public License 2.0 and should stay useful without payment. Future hosted services, premium integrations, official distribution infrastructure, gated features, or support may be licensed separately.

Detailed owner-only strategy and publication operations belong in the private sibling repository `../brawler-private` when it is available locally. Public docs should avoid personal infrastructure details and speculative monetization experiments.

The local entitlement module remains useful for future gated features and official entitlements, but the open desktop core does not depend on a license token for normal use.

Current dependency-license posture for public-opening work: [dependency-licenses.md](dependency-licenses.md) (release/legal reference — not agent reading). Public-vs-owner-only documentation split: [ADR 0023](adr/0023-public-private-documentation-split.md).
