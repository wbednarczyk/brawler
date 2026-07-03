# ADR 0005: Company Notebooks and Transcripts

Status: Accepted

## Context

The app should not only aggregate news. It should help maintain durable company-specific research notes, especially management claims that can be checked after future quarters. The project owner also wants to process YouTube press conferences with AI, review transcription output, and save selected information into a company's notebook.

## Decision

V1 includes company notebooks as a core product area. Notes belong to a canonical company and can be created manually, from feed items, from transcript segments, or from AI-suggested note drafts.

Gemini is preferred only for YouTube transcription and transcript-like extraction because official Gemini API docs currently describe video understanding, YouTube URL input, and free-tier usage limits. Milestone 10 must finish with real Gemini transcript generation working for at least one supported public YouTube URL. Other AI workflows have no preferred provider yet, and the AI integration remains provider-neutral.

## Consequences

- Notebook storage, origin, and note contracts are part of the first schema design.
- The feed detail UI must include a create-note flow.
- Video transcript jobs and transcript segments are first-class local records.
- Transcript jobs may remain unlinked to any company; company resolution is required only before saving selected transcript material into a company notebook.
- AI-suggested notes require user confirmation before saving.
- Settings must clearly disclose provider configuration, free-tier limits, and privacy implications before sending video or transcript content to a hosted provider.
