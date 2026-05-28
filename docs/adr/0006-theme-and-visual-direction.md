# ADR 0006: Theme and Visual Direction

## Status

Accepted

## Context

The app needs a visual direction before the desktop UI is scaffolded. The project owner wants a user-selectable light/dark theme, with dark mode as the default. The preferred dark palette is inspired by a night landscape reference with deep navy, blue, cyan, pink, and purple tones.

## Decision

Brawler v1 will default to dark theme and support user-selectable light and dark themes through local settings.

The initial palette family is named `night-neon`. It uses deep navy and near-black surfaces, electric blue/cyan primary accents, and pink/purple secondary accents. The palette should create atmosphere through accents, focus states, badges, charts, and selected UI states while keeping the main investor workflow dense, legible, and work-focused.

Light theme must preserve the same accent identity while using readable light surfaces and accessible contrast.

## Consequences

- Theme is part of the persisted settings contract.
- UI implementation should define semantic tokens rather than hard-coded colors in components.
- Dark theme is the first-run default.
- The reference image guides palette and mood, not a requirement for decorative full-screen backgrounds.
