# ADR 0006: Theme and Visual Direction

Status: Accepted

## Context

The app needs a visual direction before the desktop UI is scaffolded. The project owner wants a user-selectable light/dark theme, with dark mode as the default. The preferred dark palette is inspired by a night landscape reference with deep navy, blue, cyan, pink, and purple tones.

## Decision

Brawler v1 will default to dark theme and support user-selectable light and dark brightness modes through local settings.

The initial palette family is named `night-neon`. It uses deep navy and near-black surfaces, electric blue/cyan primary accents, and pink/purple secondary accents. The palette should create atmosphere through accents, focus states, badges, charts, and selected UI states while keeping the main investor workflow dense, legible, and work-focused.

Light theme must preserve the same accent identity while using readable light surfaces and accessible contrast.

As of Milestone 18, brightness mode and accent palette are separate settings. `theme` controls `dark`, `light`, or `system`; `accent_palette` controls the named palette applied to semantic CSS tokens. This keeps system light/dark behavior independent from visual palettes and lets future palettes be added without changing the brightness-mode contract.

The first additional palette is `midnight-horizon`, based on colors sampled from the project owner's preferred reference image:

- background `#00021E`
- surface `#061135`
- primary `#63C0E9`
- secondary `#55388F`
- accent `#C550B9`
- highlight `#FB82C0`
- text `#EAF7FF`

New palettes should be implemented as token adapters over semantic UI variables rather than hard-coded component colors.

## Consequences

- Theme is part of the persisted settings contract.
- Accent palette is part of the persisted settings contract.
- UI implementation should define semantic tokens rather than hard-coded colors in components.
- Dark theme is the first-run default.
- The reference image guides palette and mood, not a requirement for decorative full-screen backgrounds.
- Future palettes should be added by extending the palette registry, Settings options, and validation allow-list, not by adding one-off CSS overrides.
