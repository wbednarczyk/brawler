# AI Analysis Framework

This document records the M13 implementation boundary for source-grounded AI analysis. Use it with [Architecture](architecture.md), [Contracts](contracts.md), [Data Model](data-model.md), [Project Practices](project-practices.md), and [Kanban](kanban.md).

## M13 Goal

M13 adds source-grounded feed-item analysis using Gemini as the first live provider while keeping the runtime extensible for OpenAI, Anthropic, and other future providers.

The app should analyze local source material as decision support only. Analysis must cite source material and must not be presented as buy/sell/hold advice.

## Accepted Architecture Decisions

- **Provider-neutral interface:** app, command, storage, and UI code use an `AiAnalysisProvider`-style contract. Gemini is an adapter behind that boundary, not the domain model.
- **Async job model:** AI analysis runs through an asynchronous job so the UI is not blocked while a provider request is in progress.
- **Explicit user action:** analysis starts from a visible user action, not automatically during ingestion or feed-detail open.
- **Settings surface:** Settings exposes general AI analysis provider/model configuration and provider disclosure before feed-item analysis is broadly used.
- **Credential boundary:** general analysis credentials stay under the reusable provider credential boundary. Credential records remain purpose-aware, but the same Gemini API key may satisfy multiple Gemini purposes when configured.
- **Initial source context:** M13 analyzes feed-item title, summary/body text, source URL, attribution, company context, and language when available. Attachments and transcript-derived analysis are later slices.
- **Prompt/version provenance:** persisted results store provider ID, model, prompt version, source references, and created timestamp.
- **Recommendation enforcement deferral:** M13 prompt policy says not to generate buy/sell/hold or portfolio advice. Automated post-generation recommendation-language detection is a separate post-v1 task, not an M13 blocker.

## Non-Goals

- No automatic batch analysis during source ingestion.
- No portfolio-aware or personalized advice.
- No opinionated mode.
- No hosted services, telemetry, remote logs, or cloud queues.
- No attachment ingestion or transcript-segment analysis in the first M13 slice.
- No post-generation recommendation-language validator in M13.
- No research brief generation in M13. Future research briefs are governed by the research/evidence boundary and are not stored as ordinary notebook entries.

## Rust Ownership

Expected module ownership:

- `src-tauri/src/providers/analysis/`: provider-neutral request/result types, Gemini adapter, and deterministic test provider.
- `src-tauri/src/jobs/ai_analysis.rs`: async orchestration that loads feed context, calls the selected provider, validates required structure, and persists job/result state.
- `src-tauri/src/storage/ai_analysis.rs`: SQLite persistence for analysis jobs, results, tags, and source references.
- `src-tauri/src/commands/ai_analysis.rs`: thin Tauri commands for starting jobs and listing job/result state.
- `src-tauri/src/storage/settings.rs`: provider/model/timeout settings for general analysis.
- `src-tauri/src/providers/credentials.rs`: credential lookup/status for general analysis provider purposes.

Command modules should not contain provider HTTP details or SQL. Provider modules should not know about Tauri commands or UI state.

## Frontend Ownership

Expected module ownership:

- `src/api/aiAnalysis.ts` and `src/api/types.ts`: typed command calls and DTOs.
- `src/app/AppStateRoot.tsx` or a focused app controller: app-level refresh/reload wiring only if analysis state affects multiple screens.
- `src/screens/Inbox/`: feed-detail analysis action, status, result, and retry UI.
- `src/screens/Companies/`: company feed detail should reuse the same feed-item analysis presentation where practical.
- `src/screens/Settings/`: general analysis provider/model settings and disclosure.
- `src/shared/locale/`: localized app-owned analysis and settings copy.

If analysis display becomes shared across Inbox and Company feed detail, introduce a small shared/domain component rather than duplicating result markup.

## UI/UX Direction

M13 should consider an AI research panel pattern inspired by current AI-powered finance products such as Google Finance, but adapted to Brawler's local-first workflow and visual identity.

Useful behaviors to evaluate:

- **Suggested prompts:** provide a small set of source-grounded prompt presets for common investor-review tasks, such as "summarize what changed", "extract risks", "identify management claims", or "explain why this may matter".
- **Custom question:** let the user type a custom question about the selected feed item while keeping the source context explicit.
- **Follow-up thread:** preserve a short local thread for the selected feed item when the user asks follow-up questions.
- **Visible async progress:** show queued/running state and, where practical, the current job step so the user understands that analysis is still being produced.
- **Cited results:** make source references visible in the result, not hidden behind metadata.
- **Decision-support framing:** keep informational-only copy visible enough that the interaction does not imply trading advice.
- **Animated focus treatment:** an animated accent border or subtle moving color treatment may be used around the AI input/result container if it fits the night-neon palette, respects reduced-motion preferences, and does not distract from dense research workflows.

The implementation should not copy another product's visual design directly. The goal is to borrow the interaction idea of a research panel with prompt presets and custom questions, then express it through Brawler's own compact desktop UI.

## Runtime Flow

1. User opens a feed item and selects a visible analysis action, suggested prompt, or custom question.
2. Frontend calls a typed command to create or reuse an AI analysis job for that feed item and prompt.
3. Rust loads local feed item context and selected general-analysis settings.
4. Rust resolves provider credentials without exposing secrets to React.
5. The async job calls the selected provider and maps provider output into the neutral analysis result contract.
6. Storage persists job status and, on success, result fields plus source references.
7. UI polls or refreshes through typed commands and renders queued/running/succeeded/failed states.
8. User can retry a failed analysis job from visible UI.

## M30 Research Brief Boundary

Company/watchlist research briefs reuse the provider-neutral AI posture without turning feed-item analysis into a catch-all module.

Expected ownership:

- evidence collection belongs to the research/evidence domain
- prompt/context building belongs to a research brief builder
- provider execution uses the provider/job boundary
- citation mapping links generated claims back to research evidence
- rendering converts the stored brief into UI-facing read models
- persistence stores brief provenance, rendered content, citations, provider ID, model, prompt version, and generation timestamps

Research briefs are dedicated entities. They may be converted into notebook entries through an explicit user action later, but they are not notebook entries by default.

M30 accepted implementation decisions:

- Brief generation is explicit and on-demand only.
- Both company and watchlist brief scopes are in scope.
- The initial provider configuration reuses the existing general analysis provider/model settings.
- Evidence collection uses a backend-owned default collector for the selected scope.
- Provider output should be structured into sections with citation IDs, then rendered by the backend.
- Briefs are immutable snapshots. Regeneration creates a new brief.
- Citations store evidence references and short labels/snippets only, not full copied source bodies.
- Briefs and citations are durable research data and are included in research import/export.
- Creating notebook notes from briefs remains out of scope for M30 and must never happen automatically.

## Initial Commands

The exact names can change during implementation if tests reveal a cleaner boundary, but M13 should provide these capabilities:

- `start_ai_analysis(input)`: create or reuse an analysis job for a feed item, start async processing, and return current job/result state.
- `list_ai_analysis(input)`: return current analysis jobs/results for one feed item.
- `retry_ai_analysis(jobId)`: rerun a failed analysis job with current settings.

The start input should leave room for a stable prompt preset ID or user-provided custom question, even if the first implementation ships with only the default summary prompt.

## Settings

M13 settings should include:

- `generalAnalysisProvider`: initially `provider_gemini` or unset until configured.
- `generalAnalysisModel`: provider-specific model ID stored behind the general analysis settings boundary.
- `generalAnalysisTimeoutSeconds`: request timeout for live provider calls.
- `aiAnalysisMode`: remains `source_grounded` in M13.

Settings must disclose that feed item source text and metadata are sent to the configured AI provider when the user starts analysis.

## Test Strategy

Default checks must not require secrets or live external services.

Required M13 test areas:

- storage migration and persistence for jobs/results/source references
- settings defaults, updates, and invalid value handling
- provider-neutral request/result mapping
- deterministic test provider job flow
- Gemini adapter mapping with mocked HTTP responses
- UI workflow for explicit analysis start, loading, success, failure, and retry
- Settings workflow for general AI analysis configuration

Live Gemini analysis smoke testing should be opt-in and documented if it becomes part of M13 closure evidence.
