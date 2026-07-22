pub mod bankier_calendar;
pub mod bankier_company;
pub mod bankier_rss;
pub mod biznesradar_fundamentals;
pub mod biznesradar_ownership;
pub mod biznesradar_recommendations;
pub mod company_directory;
pub mod gpw_company_registry;
pub mod gpw_espi_ebi;
pub mod gpw_market_events;
pub mod knf_short_selling;
pub mod market_data_fetch;
pub mod newconnect_company_directory;
pub(crate) mod parsing;
pub mod registry;
pub mod yahoo_eod;

pub const USER_AGENT: &str = concat!("LocalInvestorNewsfeed/", env!("CARGO_PKG_VERSION"));
