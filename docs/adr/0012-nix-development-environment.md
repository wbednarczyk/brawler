# ADR 0012: Nix Development Environment

Status: Accepted

## Context

The project owner develops primarily in WSL2 with Ubuntu 24.04 on a Windows 11 machine. The app targets Windows first, but development should be reproducible locally and in GitHub. The project owner already uses `nix develop` with flakes and envdir-style environment loading.

## Decision

Brawler will use Nix from the first scaffold. `flake.nix` is the canonical development environment definition.

The primary local development path is WSL2 Ubuntu 24.04 using:

- `nix develop` as the explicit shell entrypoint
- optional `direnv`/`nix-direnv` for automatic shell activation
- documented local build/test commands executed inside the Nix shell

Nix should provide developer tools and native build prerequisites. It should not hide application build commands behind CI-only behavior.

GitHub Actions should either enter the same Nix development shell or run documented local commands with equivalent tool versions. `nix flake check` should be added once the scaffold has meaningful checks and if CI cost remains acceptable.

## Consequences

- The first scaffold should include `flake.nix`.
- Optional `.envrc` may be added for direnv users.
- Local commands remain the source of truth; Nix provides the environment to run them.
- WSL2 Ubuntu 24.04 is the primary development environment.
- Windows packaging and runtime testing remain explicit later milestones.
- Nix-based CI must still respect the cost policy: standard Linux runners, no scheduled jobs, no heavy packaging by default.
