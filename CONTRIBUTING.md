# Contributing

Thanks for considering a contribution to Brawler.

Brawler is maintainer-led and pre-1.0. The project values small, reviewable changes that preserve local-first behavior, clear module boundaries, and user privacy.

## Before You Start

- Read [docs/project-brief.md](docs/project-brief.md) for product intent and the documentation map.
- Read [docs/project-practices.md](docs/project-practices.md) for engineering rules.
- Check the active issue tracker before starting larger work.
- Open a discussion or issue before changing architecture, storage contracts, source policy, licensing behavior, security posture, AI behavior, packaging, or release workflow.

## Contribution Rules

- Keep changes focused and reviewable.
- Keep public behavior, docs, and tests in sync.
- Do not add telemetry, hosted services, cloud dependencies, paid APIs, or broad filesystem access without an accepted ADR.
- Do not commit secrets, API keys, tokens, databases, logs, private signing material, or generated license files.
- Use test samples and mocks for default automated tests. Live provider checks must be opt-in.
- AI output must remain decision support only and must not be presented as buy/sell/hold advice.
- Runtime dependency additions must be justified and license-reviewed.

## Local Checks

Run the normal local checks before submitting:

```bash
nix develop
npm run check
make release-check
```

Browser regression tests require Playwright's browser install:

```bash
npm run test:browser:install
npm run test:browser
```

## Commit Messages

Use Conventional Commit-style messages:

```text
feat(area): describe user-visible capability
fix(area): describe fixed behavior
docs(area): describe documentation change
refactor(area): describe structural change
test(area): describe test coverage
chore(area): describe maintenance work
```

Release commits use:

```text
chore(release): v<version>
```

## Licensing

By contributing, you agree that your contribution is licensed under the same license as Brawler: Mozilla Public License 2.0.
