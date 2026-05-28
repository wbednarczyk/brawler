# Architecture

See also [Project Brief](project-brief.md), [Product Spec](product-spec.md), [Contracts](contracts.md), and the accepted ADRs in [docs/adr](adr/).

## Chosen Stack

- Desktop shell: Tauri
- UI: React + TypeScript
- Backend/domain runtime: Rust inside the Tauri application
- Storage: SQLite
- Data posture: local-only for v1
- Scheduling: in-app scheduler while the desktop app is running

## Runtime Shape

Tauri owns the desktop shell, window lifecycle, app packaging, local permissions, background tasks, and command/event bridge between the UI and Rust domain modules.

Rust owns domain behavior:

- company registry
- watchlists
- per-company notebooks
- source adapters
- ingestion scheduler
- article/report normalization
- deduplication
- AI provider abstraction
- video/transcript processing orchestration
- transcript-to-note selection workflows
- local settings and secrets
- SQLite persistence
- Tauri command handlers and event streams

The React UI talks to Rust through typed Tauri commands. Feed, job, transcription, and notebook updates should be emitted through Tauri events.

## Data Flow

1. The user starts the desktop app.
2. Tauri starts Rust application state and opens the SQLite database.
3. Rust schedules source polling while the app is running.
4. Source adapters fetch official/public/RSS data according to source policy.
5. Raw source records are normalized into feed items.
6. Deduplication prevents repeated reports or articles from appearing multiple times.
7. The UI displays the investor inbox and receives job/feed updates through Tauri events.
8. The user can create notebook entries from feed items.
9. AI analysis, transcription, and note extraction can be requested once provider configuration exists.

## Local Storage

SQLite stores local app state and fetched content. The initial database should be migration-managed from the first code milestone.

Data must include enough provenance to audit a feed item:

- source adapter ID
- source URL
- fetched timestamp
- publication timestamp when available
- original language
- matched company identity
- attribution/display source
- raw source reference or checksum
- notebook note provenance links
- transcript segment provenance links

## Extensibility Boundaries

Source adapters should return normalized records through a common interface. Adapters must declare source type, rate limits, supported markets, and allowed fetch mode.

AI providers should implement provider-neutral interfaces. General AI analysis, summarization, significance labeling, and note extraction have no preferred provider yet.

Gemini is preferred only for the YouTube press conference transcription workflow because the Gemini API currently has native video/audio understanding and YouTube URL support. The implementation must still keep provider boundaries pluggable.

Notebook entries should be source-linked. A note can originate from manual entry, a feed item, an AI summary, a transcript segment, or a selected AI-suggested claim.

Premium or hosted convenience features must be added behind explicit interfaces, not by making local-first behavior depend on cloud services.
