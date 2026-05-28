# ADR 0004: Source and AI Policy

## Status

Accepted

## Context

Brawler aggregates financial information and may later monetize. Source usage, attribution, and AI phrasing need conservative defaults from the start.

## Decision

V1 source adapters should prefer official, public, or RSS-based sources. Restricted or fragile scraping is not allowed unless a source-specific ADR approves the risk and constraints.

AI analysis must be provider-neutral and API-first. The first AI-enabled milestones are summarization, tagging, significance classification, reasoning with source references, YouTube press conference transcription, and user-confirmed note extraction. Gemini is preferred only for YouTube video transcription because of native vendor support for video/audio and YouTube URL input. Other AI workflows have no preferred provider yet. AI output must be decision support only and must not contain direct buy/sell/hold recommendations.

## Consequences

- Source attribution and source URLs are required in feed contracts.
- Adapter implementations must document fetch mode and rate limits.
- AI providers can be swapped without changing the feed model.
- Hosted provider settings must disclose limits and privacy implications before sending video, transcript, or source text outside the local app.
- Product copy and AI prompts must avoid regulated-investment-advice framing.
