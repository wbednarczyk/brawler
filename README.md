# Brawler

Brawler is a local-first investor research workspace for company news, watchlists, notebooks, source-backed review workflows, and evidence timelines.

The application is built as a desktop app first, with Windows as the primary target. It is intended for personal research and decision support. It is not a trading platform, portfolio tracker, or investment recommendation engine.

## Status

Brawler is pre-1.0 software. The codebase is actively changing and the data model may still evolve between releases.

The core desktop application is open source under the Mozilla Public License 2.0. Future hosted services, premium integrations, official distribution infrastructure, or support offerings may be licensed separately.

## Privacy

Brawler is local-first:

- tracked companies, watchlists, feed items, notes, settings, and AI outputs stay on the user's machine by default
- secrets are stored through the operating-system keychain
- the app does not require cloud accounts, hosted activation, telemetry, or remote entitlement checks for core use
- AI/provider integrations are explicit user-configured workflows

## Source And Forge Policy

Radicle is the canonical public forge once the project is published there. GitHub is intended to act as a read-only mirror and backup copy.

Radicle publication is in progress:

- RID: `rad:z3yTYrLFsFx5qcPtV3XiFYFBpQWuh`
- Planned public seed: `seed.mikolajczyk.org:8776`

Until public Radicle publication is complete, repository location and mirror details may change.

## Build And Test

Brawler uses Nix for the development environment.

```bash
nix develop
npm install
npm run check
make release-check
```

Useful local commands:

```bash
npm run dev
npm run test
npm run test:browser
make package-windows-from-linux
```

See [Engineering Workflow](docs/engineering-workflow.md) for the full local workflow.

## Portable Windows Candidate

Build from WSL:

```bash
make package-windows-from-linux
```

Run the copied candidate:

```bash
make package-windows-smoke-run
```

Default output:

```text
D:\Brawler\Builds\latest\brawler-<version>-windows-x64-portable.exe
```

Portable runtime data is stored in a `data` folder next to the executable.

Notes:

- Windows 10/11 are the target platforms for the portable candidate.
- The executable is unsigned and may show Windows trust prompts.
- The executable relies on the system WebView2 runtime.
- Secrets stay in the OS keychain and may need to be entered again after moving the portable folder to another machine.

See [Packaging Smoke Tests](docs/packaging-smoke-tests.md) for the manual validation checklist.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Contributions are welcome after the public contribution process is active, but project direction remains maintainer-led.

## Maintainer

Brawler is authored and maintained by Wojciech Bednarczyk. See [MAINTAINERS.md](MAINTAINERS.md).

## Security

See [SECURITY.md](SECURITY.md) for supported reporting rules.

## License

Brawler is licensed under the [Mozilla Public License 2.0](LICENSE).
