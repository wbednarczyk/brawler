# ADR 0111: Retire Video Transcription

Status: Accepted (2026-09-07, owner decision at the architecture audit, N05; implemented 2026-09-10, #463)

Amends [ADR 0084](0084-retire-in-app-ai-layer.md) (decision 3 "Transcripts stay" is superseded), [ADR 0005](0005-company-notebooks-and-transcripts.md) (the transcripts leg), [ADR 0028](0028-multi-provider-ai-boundary.md) (the last provider the pattern served), [ADR 0040](0040-management-claims-tracker.md) (transcript evidence types), [ADR 0109](0109-activity-center-occurrence-ledger.md) (the `transcript` activity family), [ADR 0022](0022-research-evidence-read-model-boundary.md) / [ADR 0032](0032-search-and-backup-boundaries.md) (transcript segments as evidence / search coverage).

## Context

ADR 0084 retired the in-app AI analysis layer but kept video transcription as "data acquisition, not interpretation": a `VideoTranscriptProvider` trait with one implementation (Gemini over YouTube URLs), a transcript runner, `transcript_jobs`/`transcript_segments` storage, seven IPC commands plus `create_note_from_transcript_selection`, two MCP read tools and one act, a Transcripts screen with a nav entry and `Ctrl+6`, a Settings › Transcripts tab (model, timeout) and a Settings › Credentials tab (the Gemini API key — the only provider credential the app asked for), the `transcript_segment` evidence type across research evidence, claims, notebook origins, quality citations and search, and the `transcript` activity family. The 2026-09-07 audit found the owner's database after 13 months at `transcript_jobs = 0`, `transcript_segments = 0`, zero transcript-typed notes, claims, search rows or activity occurrences — the only residue was three settings rows and a keychain entry. ~3,100 lines of Rust and ~20 frontend files carried a workflow nobody ran, and it kept the app's last API-key dependency alive.

## Decision

1. **Video transcription is removed, not flagged.** The provider trait and its Gemini implementation, the runner, the storage module, the commands, the MCP tools, the Transcripts screen, the nav entry and shortcut (`6` stays unbound), the Settings › Transcripts and › Credentials tabs, and every `transcript*` enum value (research evidence type and source domain, trust category, claim source/extraction types, notebook origin type, search content type, activity family and target) are deleted. A transcript the investor wants in the workspace arrives like any other external document: the user's agent captures it over the MCP port (BYOA — a report document, a note with an `external_url` origin).
2. **Tables stay, readers go.** `transcript_jobs`, `transcript_segments`, the `0017`/`0039` triggers and the legacy `transcript_job_id` column on `jobs` remain in the schema (migrations are append-only, ADR 0032 data-model rules). Migration `0156` deletes the three `youtube_transcription_*` settings rows and any `search_index` row of the retired type; an old settings export that still carries those keys imports with a warning, never an error.
3. **The credential boundary shrinks to the MCP tokens.** `providers/credentials.rs` keeps the keychain boundary for the MCP auth token and the KPI-acquisition token; the provider-credential commands and `CREDENTIAL_PROVIDER_IDS` go. The existing startup cleanup (`clear_legacy_credentials`) also deletes the Gemini key entry from the OS keychain — best-effort, so a Windows install stops holding a secret nothing reads.
4. **Product identity**: Brawler has **no in-app AI**. CLAUDE.md, the brief, product-spec and architecture say so; the ADR 0080 bar (a fresh eval-gated ADR beating the deterministic baseline on real data) is the only way any inference re-enters — including a future transcription engine, local or hosted.

## Consequences

- The app runs with zero API keys and zero provider credentials; Settings has no provider/credential tabs.
- Notes and claims keep `report_document`, `feed_item`, `external_url`, `manual` provenance; an agent-supplied transcript is a document or a URL-backed note, with the same provenance rules.
- Retired names are pinned in `docs/retired-surface.json` (commands, screen, controller, trait, provider id, settings key, make target, tables outside the data-model pointer); the locale keys are pinned in `retiredKeys.test.ts`; `docs-drift` reddens on a resurrected command; the awaited-paths guard no longer lists the transcript path.
- `make smoke-gemini-transcript` is gone; `make smoke-keyring` proves the keychain with the MCP auth-token descriptor.

## Rejected

- **Keep the `VideoTranscriptProvider` trait for a future local engine (Whisper).** A single-impl trait with no caller is the ADR 0110 anti-pattern; a future engine starts from its own eval-gated ADR and a fresh design, not from dormant code.
- **Keep the enum values as legacy.** Unlike the reminder kinds retired in the ADR 0025 amendment (#465), no stored row carries them; keeping them would keep dead validation branches and TS unions alive.
- **Hide the screen behind a flag.** Same reasons as ADR 0110: nothing to gate, everything to maintain.
- **Drop the tables.** Append-only migrations.
