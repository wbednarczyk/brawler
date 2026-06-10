# ADR 0002: Tauri, React, and Rust Core

## Status

Accepted

## Context

The app needs a responsive desktop UI, modular backend/domain logic, and cross-platform packaging. The project owner wants to avoid mixing backend technologies unless there is a clear benefit.

## Decision

Use Tauri for the desktop shell, React + TypeScript for the UI, and Rust domain modules inside the Tauri application.

Tauri/Rust owns app lifecycle, local permissions, source ingestion, scheduling, storage, deduplication, AI orchestration, notebook workflows, transcription orchestration, and typed command/event boundaries.

## Consequences

- UI responsiveness is protected because ingestion and parsing run outside the renderer.
- Domain modules can be written and tested in Rust without packaging a second backend runtime.
- The command/event boundary keeps the UI separate from domain logic.
- Desktop packaging is simpler because there is no sidecar binary to supervise.
- If a future source or AI workflow strongly benefits from another runtime, it must be justified by a new ADR.
