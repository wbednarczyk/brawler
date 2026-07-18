//! The source-adapter registry — the realized `SourceAdapter` port (Architecture
//! v2 / ADR 0050, declared in [ADR 0039] and `architecture.md`).
//!
//! Historically each adapter's identity and capability metadata was scattered:
//! per-adapter `&'static str` constants, a 100-line `CASE` ladder in the catalog
//! SQL (`storage/registry.rs`), and hardcoded id lists for visibility/dispatch.
//! Adding a source meant editing several places. This module is the **single
//! source of truth**: one [`SourceAdapterDescriptor`] per adapter, exposed
//! through the [`SourceAdapter`] trait, with the catalog and the refresh
//! dispatch reading from [`REGISTRY`]. Adding a source becomes one descriptor
//! entry here (plus its parser and the append-only seed migration for the
//! mutable runtime row).
//!
//! The descriptor declares the adapter's full static capability surface
//! (`source_type`, `fetch_mode`, `markets`, …). The mutable runtime state
//! (`enabled`, `last_success_at`, error/attempt counters) stays in the
//! `source_adapters` table; a drift-guard test binds the two so the registry and
//! the seed migrations cannot silently diverge.

/// Catalog visibility tier — how a source is surfaced and whether the user can
/// toggle it. `Required` sources cannot be turned off; `Optional` are
/// user-configurable; `Developer` are hidden unless developer mode is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceVisibility {
    Required,
    Optional,
    Developer,
}

impl SourceVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Optional => "optional",
            Self::Developer => "developer",
        }
    }

    /// Whether a user may toggle this source's enabled state.
    pub fn user_configurable(self) -> bool {
        matches!(self, Self::Optional)
    }
}

/// The role a source plays in the feed pipeline (ADR 0069 decision 2, plan v0.55
/// T3). Most sources are `Primary` — they ingest items into the feed/Inbox. A
/// `Witness` source never ingests; it reconciles its listings against the primary
/// source to close a single-point-of-failure (GPW ESPI/EBI witnesses the Bankier
/// official-report channel). The Sources UI surfaces the role so a witness reads
/// as a health mechanism, not a feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRole {
    Primary,
    Witness,
}

impl SourceRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Witness => "witness",
        }
    }
}

/// The realized source-adapter port: the static identity + capability surface of
/// one source. Implemented by [`SourceAdapterDescriptor`]; the refresh path and
/// the catalog depend on this trait, not on per-adapter constants.
pub trait SourceAdapter {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn source_url(&self) -> &'static str;
    fn source_type(&self) -> &'static str;
    fn fetch_mode(&self) -> &'static str;
    fn markets(&self) -> &'static [&'static str];
    fn visibility(&self) -> SourceVisibility;
    fn role(&self) -> SourceRole;
    fn default_poll_interval_seconds(&self) -> i64;
    fn rate_limit_policy(&self) -> &'static str;
    fn policy_note(&self) -> &'static str;
}

/// Data-driven descriptor for one source adapter. All fields are `&'static`
/// because the catalog metadata is compile-time constant; mutable runtime state
/// lives in the database.
#[derive(Debug, Clone, Copy)]
pub struct SourceAdapterDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub source_url: &'static str,
    pub source_type: &'static str,
    pub fetch_mode: &'static str,
    pub markets: &'static [&'static str],
    pub visibility: SourceVisibility,
    pub role: SourceRole,
    pub default_poll_interval_seconds: i64,
    pub rate_limit_policy: &'static str,
    pub policy_note: &'static str,
}

impl SourceAdapter for SourceAdapterDescriptor {
    fn id(&self) -> &'static str {
        self.id
    }
    fn display_name(&self) -> &'static str {
        self.display_name
    }
    fn source_url(&self) -> &'static str {
        self.source_url
    }
    fn source_type(&self) -> &'static str {
        self.source_type
    }
    fn fetch_mode(&self) -> &'static str {
        self.fetch_mode
    }
    fn markets(&self) -> &'static [&'static str] {
        self.markets
    }
    fn visibility(&self) -> SourceVisibility {
        self.visibility
    }
    fn role(&self) -> SourceRole {
        self.role
    }
    fn default_poll_interval_seconds(&self) -> i64 {
        self.default_poll_interval_seconds
    }
    fn rate_limit_policy(&self) -> &'static str {
        self.rate_limit_policy
    }
    fn policy_note(&self) -> &'static str {
        self.policy_note
    }
}

const GPW: &[&str] = &["GPW"];
const NEWCONNECT: &[&str] = &["NEWCONNECT"];

/// The single source of truth for every registered source adapter. Order is the
/// canonical declaration order; the catalog sorts by display name for the UI.
pub const REGISTRY: &[SourceAdapterDescriptor] = &[
    SourceAdapterDescriptor {
        id: crate::source_adapters::gpw_company_registry::ADAPTER_ID,
        role: SourceRole::Primary,
        display_name: "GPW Company Directory",
        source_url: crate::source_adapters::gpw_company_registry::SOURCE_URL,
        source_type: "company_registry",
        fetch_mode: "public_page",
        markets: GPW,
        visibility: SourceVisibility::Required,
        default_poll_interval_seconds: 86_400,
        rate_limit_policy: "Manual refresh plus daily stale-cache scheduled refresh",
        policy_note: "Fetches the complete public GPW company list and caches ticker and ISIN metadata locally for lookup, autocomplete, and ticker-first matching.",
    },
    SourceAdapterDescriptor {
        id: crate::source_adapters::newconnect_company_directory::ADAPTER_ID,
        role: SourceRole::Primary,
        display_name: "NewConnect Company Directory",
        source_url: crate::source_adapters::newconnect_company_directory::SOURCE_URL,
        source_type: "company_registry",
        fetch_mode: "public_page",
        markets: NEWCONNECT,
        visibility: SourceVisibility::Required,
        default_poll_interval_seconds: 86_400,
        rate_limit_policy: "Manual refresh plus daily stale-cache scheduled refresh",
        policy_note: "Fetches the complete public NewConnect company list and caches ticker and ISIN metadata for lookup, autocomplete, and ticker-first matching.",
    },
    SourceAdapterDescriptor {
        id: "yahoo-eod",
        role: SourceRole::Primary,
        display_name: "Yahoo Finance EOD Quotes",
        source_url: "https://query1.finance.yahoo.com/v8/finance/chart/",
        source_type: "market_data",
        fetch_mode: "public_json",
        markets: GPW,
        visibility: SourceVisibility::Optional,
        default_poll_interval_seconds: 86_400,
        rate_limit_policy: "Watchlist-only; throttle + jitter; 429/999 backoff; aggressive cache; one full-history backfill on add plus one post-session daily pull per company",
        policy_note: "Primary EOD price source (ADR 0082). Yahoo v8 chart API by exchange-qualified <ticker>.WA; keyless, PLN. ToS-gray, accepted narrowly for local-first personal EOD/watchlist use, no redistribution. A pull failure raises source-health and skips the day; self-heal backfill catches history up (no free fallback provider — ADR 0082 amendment 2026-07-14, card ee81afe).",
    },
    SourceAdapterDescriptor {
        id: crate::source_adapters::bankier_rss::ADAPTER_ID,
        role: SourceRole::Primary,
        display_name: crate::source_adapters::bankier_rss::DISPLAY_NAME,
        source_url: crate::source_adapters::bankier_rss::SOURCE_URL,
        source_type: "public_media",
        fetch_mode: "rss",
        markets: GPW,
        visibility: SourceVisibility::Optional,
        default_poll_interval_seconds: 900,
        rate_limit_policy: "Manual refresh plus normal in-app source scheduler; RSS feed only, no article crawling",
        policy_note: "Fetches Bankier.pl public Giełda RSS headlines as public media items; linked article pages are not crawled in this slice.",
    },
    SourceAdapterDescriptor {
        id: crate::source_adapters::bankier_company::ADAPTER_ID,
        role: SourceRole::Primary,
        display_name: crate::source_adapters::bankier_company::DISPLAY_NAME,
        source_url: crate::source_adapters::bankier_company::SOURCE_URL,
        source_type: "official_report",
        fetch_mode: "public_json",
        markets: GPW,
        visibility: SourceVisibility::Optional,
        default_poll_interval_seconds: 900,
        rate_limit_policy: "Manual refresh plus normal in-app source scheduler; tracked GPW companies only; cached Bankier tag ids; one listing page plus matched article pages per company",
        policy_note: "Fetches Bankier.pl per-company public komunikaty JSON and article pages for tracked GPW companies only. Bankier is the active v1 official-report source while GPW ESPI/EBI is disabled.",
    },
    SourceAdapterDescriptor {
        id: crate::source_adapters::gpw_market_events::ADAPTER_ID,
        role: SourceRole::Primary,
        display_name: crate::source_adapters::gpw_market_events::DISPLAY_NAME,
        source_url: crate::source_adapters::gpw_market_events::SOURCE_URL,
        source_type: "official_calendar",
        fetch_mode: "rss",
        markets: GPW,
        visibility: SourceVisibility::Optional,
        default_poll_interval_seconds: 900,
        rate_limit_policy: "Manual refresh plus normal in-app source scheduler; official GPW market-events RSS; exact ticker matching only",
        policy_note: "Fetches GPW official market-events RSS for corporate-action and exchange calendar events. Creates company events only for tracked companies matched by exact ticker.",
    },
    SourceAdapterDescriptor {
        id: crate::source_adapters::bankier_calendar::ADAPTER_ID,
        role: SourceRole::Primary,
        display_name: crate::source_adapters::bankier_calendar::DISPLAY_NAME,
        source_url: crate::source_adapters::bankier_calendar::SOURCE_URL,
        source_type: "public_calendar",
        fetch_mode: "public_page",
        markets: GPW,
        visibility: SourceVisibility::Optional,
        default_poll_interval_seconds: 900,
        rate_limit_policy: "Manual refresh plus normal in-app source scheduler; one public calendar page; tracked GPW companies only; exact ticker matching",
        policy_note: "Active M9 public calendar source for broader GPW event coverage. Creates company events only for tracked companies matched by exact ticker, while preserving Bankier attribution and source URLs.",
    },
    SourceAdapterDescriptor {
        id: crate::source_adapters::knf_short_selling::ADAPTER_ID,
        role: SourceRole::Primary,
        display_name: crate::source_adapters::knf_short_selling::DISPLAY_NAME,
        source_url: crate::source_adapters::knf_short_selling::SOURCE_URL,
        source_type: "disclosure",
        fetch_mode: "public_json",
        markets: GPW,
        visibility: SourceVisibility::Optional,
        default_poll_interval_seconds: 86_400,
        rate_limit_policy: "Manual refresh plus daily scheduled refresh; single POST to the official public KNF register JSON endpoint; current-register method only",
        policy_note: "KNF public national short-selling register (net short positions >= 0.5%) as a disclosure source for tracked GPW companies matched by ISIN (ADR 0069). Stable public JSON endpoint, no HTML scraping. Register changes emit short_position_change signals.",
    },
    SourceAdapterDescriptor {
        id: crate::source_adapters::biznesradar_ownership::ADAPTER_ID,
        role: SourceRole::Primary,
        display_name: crate::source_adapters::biznesradar_ownership::DISPLAY_NAME,
        source_url: crate::source_adapters::biznesradar_ownership::SOURCE_URL,
        source_type: "ownership",
        fetch_mode: "public_page",
        markets: GPW,
        visibility: SourceVisibility::Optional,
        default_poll_interval_seconds: 86_400,
        rate_limit_policy: "Manual refresh plus daily scheduled refresh; one public akcjonariat page per tracked GPW company with a polite inter-request delay; the parsed table is written as a full-picture aggregator ownership basis (never enters the feed)",
        policy_note: "BiznesRadar public \"Akcjonariat\" pages as the automatic ownership BREADTH source (ADR 0072 §2c as amended 2026-07-16, probe 2026-07-16). Chosen over Bankier: BiznesRadar robots.txt is fully permissive and its host is independent of the primary Bankier official-report channel. Server-rendered tables with separate capital % and votes % (WZA); the GPW ticker resolves to the canonical slug via redirect. Each refresh writes the table as an aggregator ownership snapshot (visible source label, no per-row confirmation — a deterministic parse of a machine-published table); disclosed reports/ESPI witness it (reversed comparison) and divergences are recorded as diagnostics and a per-company witness result.",
    },
    SourceAdapterDescriptor {
        id: crate::source_adapters::gpw_espi_ebi::ADAPTER_ID,
        role: SourceRole::Witness,
        display_name: crate::source_adapters::gpw_espi_ebi::DISPLAY_NAME,
        source_url: "https://www.gpw.pl/komunikaty",
        source_type: "official_report",
        fetch_mode: "public_page",
        markets: GPW,
        visibility: SourceVisibility::Optional,
        default_poll_interval_seconds: 900,
        rate_limit_policy: "Manual refresh plus normal in-app source scheduler; witness role only — the official GPW ESPI/EBI listing is reconciled against Bankier-sourced reports, never ingested into the feed",
        policy_note: "Reconciliation second witness for the Bankier official-report channel (ADR 0069 decision 2, plan v0.55 T3). Its ESPI/EBI listings are matched against Bankier-sourced reports for tracked GPW companies; disagreements are recorded and an official report the primary missed (espi_only) raises an attention event. Witness items never enter the feed/Inbox (no dual ingestion).",
    },
    SourceAdapterDescriptor {
        id: "portal-analiz",
        role: SourceRole::Primary,
        display_name: "Portal Analiz",
        source_url: "https://portalanaliz.pl/",
        source_type: "authenticated_research",
        fetch_mode: "authenticated",
        markets: GPW,
        visibility: SourceVisibility::Developer,
        default_poll_interval_seconds: 86_400,
        rate_limit_policy: "Late-v1 disabled placeholder; no automated access until the authenticated-source implementation is explicitly built",
        policy_note: "Late-v1 planned authenticated private research adapter governed by ADR 0014. Credentials must use the OS keychain and no generic login or scraping subsystem is approved.",
    },
    SourceAdapterDescriptor {
        id: "bankier-firma-rss",
        role: SourceRole::Primary,
        display_name: "Bankier Firma RSS",
        source_url: "https://www.bankier.pl/rss/firma.xml",
        source_type: "public_media",
        fetch_mode: "rss",
        markets: GPW,
        visibility: SourceVisibility::Developer,
        default_poll_interval_seconds: 900,
        rate_limit_policy: "Reviewed public RSS candidate; disabled until matching quality is proven against tracked GPW companies",
        policy_note: "Reviewed M8 follow-up candidate. Public and RSS-native, but broader business coverage needs matching-quality tests before runtime enablement.",
    },
    SourceAdapterDescriptor {
        id: "bankier-wiadomosci-rss",
        role: SourceRole::Primary,
        display_name: "Bankier Wiadomosci RSS",
        source_url: "https://www.bankier.pl/rss/wiadomosci.xml",
        source_type: "public_media",
        fetch_mode: "rss",
        markets: GPW,
        visibility: SourceVisibility::Developer,
        default_poll_interval_seconds: 900,
        rate_limit_policy: "Reviewed public RSS candidate; disabled because expected listed-company signal is broad and noisy",
        policy_note: "Reviewed M8 follow-up candidate. Public and RSS-native, but broad news coverage and stale backfill risk make it unsuitable for default v1 ingestion.",
    },
    SourceAdapterDescriptor {
        id: "strefa-report-calendar",
        role: SourceRole::Primary,
        display_name: "Strefa Report Calendar",
        source_url: "https://strefainwestorow.pl/dane/raporty",
        source_type: "public_calendar",
        fetch_mode: "public_page",
        markets: GPW,
        visibility: SourceVisibility::Developer,
        default_poll_interval_seconds: 86_400,
        rate_limit_policy: "Disabled event-source candidate; report-date extraction requires source-specific tests before runtime enablement",
        policy_note: "Fallback candidate for periodic-report publication dates. Disabled until source-specific sample parsing and attribution rules are accepted.",
    },
    SourceAdapterDescriptor {
        id: "money-calendar",
        role: SourceRole::Primary,
        display_name: "Money Calendar",
        source_url: "https://www.money.pl/gielda/raporty/",
        source_type: "public_calendar",
        fetch_mode: "public_page",
        markets: GPW,
        visibility: SourceVisibility::Developer,
        default_poll_interval_seconds: 86_400,
        rate_limit_policy: "Disabled event-source candidate; calendar extraction requires source-specific tests before runtime enablement",
        policy_note: "Fallback/cross-check candidate for calendar and report-date coverage. Disabled until source-specific sample parsing and matching quality are accepted.",
    },
    SourceAdapterDescriptor {
        id: "brawler-red-flags",
        role: SourceRole::Primary,
        display_name: "Brawler — derived red flags",
        source_url: "internal://red-flags",
        source_type: "derived",
        fetch_mode: "internal",
        // No market rows: internal adapters are marketless (migration 0092
        // seeds no source_adapter_markets entries).
        markets: &[],
        visibility: SourceVisibility::Developer,
        default_poll_interval_seconds: 86_400,
        rate_limit_policy: "Internal derived-event owner; never fetches — red-flag detections write their synthetic feed items under this adapter id (v0.57 T7, ADR 0083 D8)",
        policy_note: "Not a real source: owns the synthetic feed items derived red flags raise (report_delay / fund_exit / score_deterioration) so provenance and the KNF-pattern alert path stay uniform. Seeded disabled by migration 0092; no network access, no scheduler participation.",
    },
];

/// Look up an adapter descriptor by its id.
pub fn descriptor(adapter_id: &str) -> Option<&'static SourceAdapterDescriptor> {
    REGISTRY.iter().find(|adapter| adapter.id == adapter_id)
}

/// The catalog visibility tier for an adapter id (defaults to `Developer` for an
/// unregistered id, matching the historical fallback).
pub fn source_visibility(adapter_id: &str) -> SourceVisibility {
    descriptor(adapter_id)
        .map(|adapter| adapter.visibility)
        .unwrap_or(SourceVisibility::Developer)
}
