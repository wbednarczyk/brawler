pub mod bankier_company;
pub mod bankier_rss;
pub mod gpw_company_registry;
pub mod gpw_espi_ebi;

pub const USER_AGENT: &str = concat!("LocalInvestorNewsfeed/", env!("CARGO_PKG_VERSION"));
