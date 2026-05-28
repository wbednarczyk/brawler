# ADR 0001: Local-First Desktop Application

## Status

Accepted

## Context

The first user needs a personal Windows-first application for aggregating company news and reports. The app should remain cross-platform-capable and leave room for monetization later, but should not depend on cloud infrastructure for v1.

## Decision

Brawler v1 will be a local-first desktop application. Watchlists, feed items, settings, source history, and AI outputs are stored locally.

The app will target Windows first while preserving a path to macOS and Linux builds.

## Consequences

- User data remains private by default.
- V1 can be developed without hosted auth, accounts, sync, or cloud jobs.
- Future paid convenience features such as sync, backups, managed AI configuration, or notifications must be optional additions.
- Multi-device workflows are deferred.
