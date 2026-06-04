# Licensing Strategy Assessment

This note records the current licensing and monetization assessment for Brawler as of 2026-06-04. It is a planning reference, not legal advice and not the final public license decision.

Canonical current decision: [ADR 0008](adr/0008-license-and-project-governance.md) still applies. Brawler is all rights reserved for now, the GitHub repository remains private, and no open-source license should be added until a future ADR resolves the public license and commercial boundary.

M17 implementation decision: [ADR 0017](adr/0017-friend-test-license-gate.md) defines the local offline author/friend-test entitlement gate. It does not decide the final public license.

## Current Owner Preferences

Captured preferences:

- Giving back means public code and the possibility of contributions, not only sponsoring dependencies or publishing articles.
- A future forkable community build may be acceptable, but only after the monetization path is already working.
- Likely monetization candidates are cloud sync, cloud backup, premium features, official packaged builds/updates, support, managed AI convenience, and premium integrations.
- External contributions are not expected soon. If they happen, they should wait until the license and contribution posture are settled.
- The project owner is uncomfortable with a large company taking the app, building on it, and monetizing it heavily without sharing value back.

## Important Open-Source Reality

Real OSI open-source licenses allow commercial use. A license that blocks commercial competitors, requires profit sharing, or discriminates against a field of endeavor is not OSI open source.

That means the project cannot have all three of these properties at the same time:

- OSI open source.
- A legal restriction that prevents commercial exploitation by a large company.
- A requirement that commercial users share revenue or profits.

If commercial restriction or profit sharing is a hard requirement, Brawler would need a proprietary or source-available model instead of an OSI open-source license.

## Pathways Considered

### Private / Proprietary For Now

Keep the repository private and all rights reserved. Distribute friend-test builds only through a local offline license gate.

Benefits:

- Preserves maximum future flexibility.
- Avoids accidental open-source commitments before monetization is understood.
- Fits M17 friend testing without requiring cloud accounts, billing, telemetry, or hosted activation.

Costs:

- No public code or outside contribution path yet.
- Less community trust until a public license exists.

This remains the current path.

### Permissive Open Source: MIT Or Apache-2.0

Publish the app or core under a permissive OSI license.

Benefits:

- Maximum adoption and contribution friendliness.
- Very low friction for users and companies.
- Apache-2.0 includes an explicit patent grant, which is useful for larger commercial users.

Costs:

- Anyone can fork, redistribute, commercialize, and remove a local license gate from their fork, subject to notice and license compliance.
- It does little to require that commercial improvements return to the project.

This is probably too permissive for the owner's current concerns.

### MPL-2.0 Core

Publish the core under the Mozilla Public License 2.0.

Benefits:

- OSI open source.
- File-level copyleft: changes to MPL-covered files must be shared under MPL, while proprietary modules can be combined in a larger work.
- Better fit for open core than GPL when proprietary premium modules may exist later.
- Encourages improvements to the shared core to remain public.

Costs:

- Companies may still commercialize the app or combine it with proprietary modules.
- It does not create a profit-sharing right.
- It is more complex than MIT/Apache and should be reviewed before public launch.

This is the strongest current candidate for a future open-core Brawler core if the project chooses real open source later.

### GPLv3 Or AGPLv3

Publish the app under a strong copyleft license.

Benefits:

- Strongest give-back pressure for distributed derivative works.
- AGPLv3 adds network-use source-sharing obligations for modified server-side versions.

Costs:

- Harder to combine with proprietary premium modules.
- Can deter commercial users and some contributors.
- Still does not create a right to profit sharing.
- Dual licensing later becomes harder once outside contributions exist unless contribution agreements are in place.

This is probably too restrictive for the likely open-core monetization direction.

### Open Core

Publish a useful core under an open-source license and keep selected commercial features proprietary or service-backed.

Good paid candidates:

- Encrypted cloud sync across devices.
- Cloud backup and restore.
- Official signed Windows installers, update channels, and support.
- Premium source adapters or paid-data integrations where redistribution terms allow it.
- Advanced AI workflows or managed AI provider convenience.
- Mobile companion features after sync exists.

Core features that should remain useful without payment if Brawler later wants open-source goodwill:

- Local desktop app shell.
- Local watchlists, companies, inbox, notes, claims, events, transcripts, settings, local logs, diagnostics, and metrics.
- Bring-your-own-key AI/provider configuration where practical.
- Local export/import and backup basics.

Benefits:

- Good alignment with public code plus sustainable maintenance.
- Paid value can live in services, packaging, support, or proprietary modules rather than crippling the local app.

Costs:

- Requires clean module and entitlement boundaries.
- Reputational risk if the free core feels intentionally incomplete.
- Needs careful contribution and trademark posture before public release.

This is the best current strategic direction, but it is not yet a committed license decision.

### Source-Available / Fair Source / BSL / FSL

Publish source code under terms that restrict competing commercial use or delay open-source conversion.

Benefits:

- Stronger protection from commercial free-riding.
- Some variants eventually convert old code to MIT or Apache-2.0.

Costs:

- Not OSI open source while restrictions apply.
- Can confuse contributors and users if marketed as open source.
- May reduce ecosystem trust.

This remains an option only if commercial protection becomes more important than OSI open-source status.

## Recommended Direction

Short term:

- Keep Brawler private and all rights reserved.
- Implement M17 as a local author/friend-test entitlement gate.
- Do not decide the final public license in M17.
- Do not accept outside contributions until the contribution model is defined.

Medium term:

- Build commercial optionality around cloud sync, cloud backup, official builds/updates, support, premium source integrations, managed AI convenience, and future mobile/sync surfaces.
- Keep the local desktop app useful without cloud infrastructure.
- Preserve modularity so open-core and proprietary/service-backed boundaries can be introduced without rewriting core workflows.

Likely future public posture:

- Consider MPL-2.0 for the core if the project chooses real open source.
- Keep cloud services, premium modules, commercial source integrations, official distribution infrastructure, and trademarks outside the open core unless a later ADR says otherwise.
- Use trademark/brand control and service quality as the practical moat, not a license-key gate inside public open-source code.

## M17 Friend-Test Gate Implications

M17 should implement a reversible entitlement boundary, not final DRM.

The licensing module should separate:

- License token parsing.
- Offline signature verification.
- Entitlement policy evaluation.
- Feature or edition claims.
- Local storage of accepted license state.
- UI presentation and recovery flows.
- Redaction for logs, diagnostics, settings export, and tests.

The first local policy can require a valid signed token for normal packaged app use. Future policies should be able to support:

- friend-test builds
- personal builds
- community/open-core builds
- paid subscription entitlements
- feature-specific entitlements
- version or channel-specific entitlements

Recommended license claims to keep the model extensible:

- `license_id`
- `holder`
- `channel`
- `edition`
- `features`
- `issued_at`
- `expires_at`
- `app_version_range`
- `key_id`

The app should embed only public verification material. Private signing material and release-owner key generation workflows must stay outside the repository and build outputs.

## Contribution Posture Before Public Code

Before accepting external contributions, decide and document:

- Public license.
- Contributor agreement posture: DCO, CLA, or no outside contributions.
- Whether contributor terms must preserve future dual licensing or commercial modules.
- Trademark and brand usage rules.
- Third-party notices and dependency license scanning.
- Which modules are open core and which modules are commercial/service-backed.

Without this, accepting contributions can make future relicensing or commercial boundary changes harder.

## Future ADR Triggers

Add or update an ADR before:

- Publishing the repository publicly.
- Accepting any outside contribution.
- Publishing public release artifacts.
- Changing from all-rights-reserved to any open-source or source-available license.
- Adding hosted activation, billing, cloud sync, cloud backup, telemetry, or commercial service infrastructure.
- Introducing proprietary modules into a public/open-core repository.

## Reference Sources

- Open Source Initiative, Open Source Definition: https://opensource.org/osd
- Open Source Initiative, FAQ: https://opensource.org/faq/
- Mozilla Public License 2.0 FAQ: https://www.mozilla.org/en-US/MPL/2.0/FAQ/
- Apache License 2.0: https://www.apache.org/licenses/LICENSE-2.0
- GNU Affero General Public License v3: https://www.gnu.org/licenses/agpl.html
- Open Source Guides, Legal Side of Open Source: https://opensource.guide/legal/
- Business Source License 1.1: https://mariadb.com/bsl11/
- Functional Source License: https://fsl.software/
