# ADR 0026: Reusable UI Foundation

Status: Accepted

## Context

Brawler has repeated UI patterns across screens: subnavigation, panels, field rows, selects, buttons, badges, pills, list rows, and status chips. Repeating these patterns as screen-local CSS and markup has already caused inconsistent behavior, including Settings navigation clipping and horizontal overflow regressions.

The application also has a project-wide extensibility rule. UI code should be structured so common behavior and visual treatment can evolve centrally, and so future adoption of a UI framework does not require rewriting every screen.

## Decision

Brawler will use an app-owned reusable UI primitive boundary under `src/ui`.

The first implementation uses plain React and CSS:

- shared primitive exports live behind `src/ui/index.ts`
- reusable styles live in `src/styles/ui.css`
- screens import Brawler primitives, not implementation-specific framework components
- primitive APIs are semantic and app-facing, with a limited `className` escape hatch
- implementation can later wrap Radix or another library behind the same primitive API where the added behavior or accessibility support is justified

The first migrated primitives are:

- `Panel` and `PanelHeader` for screen-level surfaces and headers
- `Subnav` for icon-supported section navigation with stable active, hover, focus, long-label, and overflow behavior
- `FieldRow` for grouped setting controls
- `SelectField` for native select controls with consistent label and control styling
- `TextField` for single-line text inputs with consistent border, padding, focus ring, placeholder, and disabled treatment (the text-input counterpart to `SelectField`; replaces raw `<input>` elements that previously inherited no shared styling)
- `DetailSection` for self-contained detail-rail cards that establish the rail containment contract (see ADR 0030)
- `Button` for app-owned button variants behind a stable UI boundary
- `StatusChip` for compact status labels and repeated metadata chips
- `StatusPill` for existing membership/status pill treatment while older screens migrate
- `EmptyState` for reusable empty-list and no-result states
- `DenseRow` as a shared selectable-row shell for row state, focus, disabled, unread, and selected styling
- `InlineConfirm` for compact in-flow confirmation prompts
- `ExpandableRow` for older row/detail interactions that still need compatibility during migration

Migration is incremental. New screens should use shared primitives by default. Existing screens should move view by view when touched or when repeated layout defects justify consolidation.

Dense row migration needs a narrower approach than a single universal row body. Existing rows fall into separate families:

- feed and evidence rows, where selection, unread state, and row actions are shared but body metadata differs by source
- entity rows, such as company and watchlist rows, where membership/context cues are domain-specific
- source and transcript rows, where status, enablement, and expandable operational detail are central
- event rows, where date grouping and calendar-specific layout dominate
- settings and shortcut rows, which are form-like and should continue using field/action primitives

`DenseRow` therefore owns only the common shell and state classes. Screen-specific body layout remains local until a row family is migrated intentionally. Migration order should start with the closest families first: source/transcript rows, then feed/evidence rows, then entity rows. Event rows should stay separate unless later evidence shows enough overlap.

## Consequences

- UI consistency improves without a large framework migration.
- Screen code becomes less coupled to low-level CSS class conventions.
- Future Radix or framework adoption remains possible behind stable Brawler-owned primitives.
- Some screen CSS will remain during migration; local CSS should describe layout and composition, not reimplement reusable controls.
- Shared primitives need regression tests for layout behavior that has failed before, especially horizontal overflow, long localized labels, active states, and dense desktop layouts.
