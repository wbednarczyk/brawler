use super::*;

pub(super) fn registry_entry(
    ticker: &str,
    display_name: &str,
    isin: &str,
) -> GpwCompanyRegistryEntry {
    GpwCompanyRegistryEntry {
        exchange: "GPW".to_owned(),
        ticker: ticker.to_owned(),
        qualified_ticker: format!("GPW:{ticker}"),
        display_name: display_name.to_owned(),
        isin: isin.to_owned(),
        source_url: format!("https://www.gpw.pl/spolka?isin={isin}"),
        sector: None,
    }
}

pub(super) fn sample_cdr_listing() -> GpwReportListing {
    GpwReportListing {
        report_type: "Bieżący".to_owned(),
        system: "ESPI".to_owned(),
        report_number: "1/2026".to_owned(),
        company_ticker: "CDR".to_owned(),
        company_name: "CD PROJEKT S.A.".to_owned(),
        isin: "PLOPTTC00011".to_owned(),
        title: "Current report placeholder for tracked company".to_owned(),
        detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=111111".to_owned(),
        published_at: "2026-05-29T09:12:00Z".to_owned(),
        fetched_at: "2026-05-29T09:15:00Z".to_owned(),
        dedupe_key: "gpw-espi-ebi:test:PLOPTTC00011:1/2026:2026-05-29T09:12:00Z".to_owned(),
        body_text: None,
        attachments: Vec::new(),
    }
}

pub(super) fn sample_bankier_items() -> Vec<BankierRssItem> {
    vec![
        BankierRssItem {
            title: "CD Projekt rośnie po komentarzu zarządu".to_owned(),
            link: "https://www.bankier.pl/wiadomosc/cd-projekt-komentarz-900001.html".to_owned(),
            summary: "Inwestorzy obserwują CD Projekt po nowych informacjach.".to_owned(),
            published_at: Some("2026-05-31T09:15:00+02:00".to_owned()),
            fetched_at: "2026-05-31T10:00:00Z".to_owned(),
            dedupe_key: "bankier-market-rss:bankier-900001".to_owned(),
        },
        BankierRssItem {
            title: "Rynek czeka na decyzje banków centralnych".to_owned(),
            link: "https://www.bankier.pl/wiadomosc/rynek-czeka-900002.html".to_owned(),
            summary: "Przegląd tematów na europejskich parkietach.".to_owned(),
            published_at: Some("2026-05-31T08:45:00+02:00".to_owned()),
            fetched_at: "2026-05-31T10:00:00Z".to_owned(),
            dedupe_key: "bankier-market-rss:bankier-900002".to_owned(),
        },
    ]
}

pub(super) fn sample_bankier_company_items(company: &Company) -> Vec<BankierCompanyItem> {
    vec![BankierCompanyItem {
            company_id: company.id.clone(),
            qualified_ticker: company.qualified_ticker.clone(),
            title: "Wyniki finansowe QSr 1/2026".to_owned(),
            link: "https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-Wyniki-finansowe-QSr-1-2026-9141553.html"
                .to_owned(),
            summary: "raporty okresowe: kwartalne, polroczne, roczne".to_owned(),
            published_at: Some("2026-05-28T17:33:09".to_owned()),
            fetched_at: "2026-05-31T10:00:00Z".to_owned(),
            article_id: "9141553".to_owned(),
            pub_id: 3,
            dedupe_key: "bankier-company-komunikaty:article:9141553".to_owned(),
            duplicate_signature:
                "official-secondary:GPW:CDR:wyniki-finansowe-qsr-1-2026:9141553".to_owned(),
            body_text: Some("Official Bankier report body from the article page.".to_owned()),
            attachments: vec![BankierCompanyAttachment {
                label: "report.xhtml".to_owned(),
                url: "https://bonnier.pl/report.xhtml".to_owned(),
            }],
            detail_fetch_attempted: true,
        }]
}
