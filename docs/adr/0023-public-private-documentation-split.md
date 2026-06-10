# ADR 0023: Public And Private Documentation Split

## Status

Accepted

## Context

Brawler uses a public open-core posture. Some project context is useful for users and contributors, while some context is owner-only operations or strategy that should not be exposed in the public repository.

Gitignored files are not a durable home for owner-only documentation because they are easy to lose and invisible to future agents. A separate private repository gives that context versioning and backup without publishing it.

## Decision

Brawler uses two sibling repositories:

- `brawler`: the public-ready application repository.
- `brawler-private`: owner-only operational context, kept private on GitHub.

The public repository may mention that agents can read `../brawler-private` when available, but private content must not be copied into public docs, issues, patches, pull requests, or release notes unless the project owner explicitly asks for that specific content to become public.

The private repository stores private context and procedures, not plaintext secrets. Private signing keys, raw generated license tokens, API keys, `.env` files, local app databases, logs, and generated release artifacts stay outside Git or in a future explicitly approved encrypted secret store.

Public docs should describe open-core posture only at a high level. Owner-only monetization strategy, publication operations, seed-host trust notes, and license-token operations belong in `brawler-private`. Public code may contain the extensible entitlement module, but private signing operations and detailed business experiments stay out of public docs.

## Consequences

- Public docs can become suitable for users and contributors without leaking personal or operational detail.
- Agents can still access fuller owner context on the project owner's machine.
- Private repo availability becomes part of local agent context, but public repo correctness must not depend on it.
- If encrypted secret backup is needed later, it requires a separate decision and tool choice.
