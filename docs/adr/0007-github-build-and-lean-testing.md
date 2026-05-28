# ADR 0007: GitHub Build and Lean Testing

## Status

Accepted

## Context

Brawler should be easy to build, prototype, and test locally first, while also being easy to run in the GitHub ecosystem. Testing is important for long-term maintainability, but the project should avoid slow or overblown test suites that make iteration painful.

## Decision

Brawler will be designed so local build/test commands are the primary interface. GitHub Actions must run the same commands or thin documented wrappers around them. Default CI should run on pushes and pull requests to `master`, require no secrets, keep feedback fast, and minimize GitHub Actions usage while the repository is private.

Testing will follow a lean layered strategy:

- Rust unit tests for domain logic, contracts, parsing, dedupe, migrations, and provider mapping.
- Frontend unit/component tests for critical UI behavior.
- Fixture-based adapter tests for external sources.
- A small number of desktop smoke tests for startup, command availability, and local SQLite connectivity.

Live network tests and provider/API-key tests are excluded from default CI. They may be added later as manual jobs.

Default workflows should use standard GitHub-hosted Linux runners only. Larger runners, scheduled workflows, macOS runners, and full packaging builds are excluded from default CI unless a later decision accepts the cost and value tradeoff. Because the repository is currently private, GitHub Actions minutes and artifact storage should be treated as constrained resources.

## Consequences

- Source adapters must be designed around fixtures.
- AI providers must be mockable.
- CI workflows are part of the scaffold milestone.
- Every default CI check must have an equivalent local command.
- CI-only build/test logic should be avoided.
- Packaging jobs can be slower and separate from the default PR loop.
- Test coverage should protect behavior and contracts, not implementation details.
- Workflow design should use path filters, concurrency cancellation, short artifact retention, and manual packaging triggers to avoid unnecessary free-tier usage.
