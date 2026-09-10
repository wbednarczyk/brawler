use super::*;
use std::cell::RefCell;

const HTML: &str = include_str!("../../../samples/bankier_company_cdr.html");
const JSON: &str = include_str!("../../../samples/bankier_company_cdr_listing.json");

fn target() -> BankierCompanyTarget {
    BankierCompanyTarget {
        company_id: "company_gpw_cdr".to_owned(),
        ticker: "CDR".to_owned(),
        qualified_ticker: "GPW:CDR".to_owned(),
        bankier_slug: None,
        bankier_tag_id: None,
    }
}

struct DetailFilterFetcher {
    fetched_urls: RefCell<Vec<String>>,
}

impl BankierCompanyFetcher for DetailFilterFetcher {
    fn fetch_text(&self, url: &str) -> Result<String, BankierCompanyError> {
        self.fetched_urls.borrow_mut().push(url.to_owned());

        if url.starts_with(API_SOURCE_URL) {
            Ok(JSON.to_owned())
        } else if url.contains("9141553") {
            Ok(r#"
                <html>
                  <head>
                    <script type="application/ld+json">
                      {
                        "@type": "NewsArticle",
                        "articleBody": "First report body"
                      }
                    </script>
                  </head>
                  <body></body>
                </html>
            "#
            .to_owned())
        } else {
            Ok(r#"
                <html>
                  <head>
                    <script type="application/ld+json">
                      {
                        "@type": "NewsArticle",
                        "articleBody": "Second report body"
                      }
                    </script>
                  </head>
                  <body></body>
                </html>
            "#
            .to_owned())
        }
    }
}

#[test]
fn detects_periodic_reports_and_rejects_current_reports() {
    // Periodic / financial report titles and form codes.
    for title in [
        "Skonsolidowany raport kwartalny QSr 1/2026",
        "Raport roczny za 2025 rok",
        "Raport półroczny PSr 2025",
        "Wyniki finansowe za III kwartał 2025",
        "SA-R 2025",
    ] {
        assert!(
            text_marks_periodic_report(title),
            "expected periodic: {title}"
        );
    }

    // Routine current reports must not be treated as periodic.
    for title in [
        "Powiadomienie o transakcjach na akcjach - art. 19 ust. 1 MAR",
        "Zwołanie Zwyczajnego Walnego Zgromadzenia",
        "Rekomendacja Zarządu w sprawie wypłaty dywidendy za rok 2025",
        "Zawarcie znaczącej umowy",
    ] {
        assert!(
            !text_marks_periodic_report(title),
            "expected non-periodic: {title}"
        );
    }
}

#[test]
fn parses_company_page_identifiers() {
    let identifiers = parse_company_identifiers(HTML).expect("HTML should parse");

    assert_eq!(identifiers.slug, "CDPROJEKT");
    assert_eq!(identifiers.tag_id, "722");
}

#[test]
fn builds_listing_api_url() {
    let url = listing_api_url("722", 1, 25).expect("URL should build");

    assert!(url.starts_with("https://api.bankier.pl/articles/listing/1/25?"));
    assert!(url.contains("tags_ids=722"));
    assert!(url.contains("pub_id"));
}

#[test]
fn parses_company_listing_json() {
    let items = parse_company_listing_json(&target(), JSON, "2026-05-31T10:00:00Z").expect("JSON");

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].company_id, "company_gpw_cdr");
    assert_eq!(items[0].qualified_ticker, "GPW:CDR");
    assert_eq!(items[0].pub_id, 3);
    assert_eq!(items[0].article_id, "9141553");
    assert_eq!(items[0].title, "Wyniki finansowe QSr 1/2026");
    assert_eq!(
        items[0].link,
        "https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-Wyniki-finansowe-QSr-1-2026-9141553.html"
    );
    assert_eq!(
        items[0].published_at,
        Some("2026-05-28T17:33:09".to_owned())
    );
    assert_eq!(
        items[0].dedupe_key,
        "bankier-company-komunikaty:article:9141553"
    );
    assert_eq!(items[0].summary, "Komunikat ESPI/EBI");
    assert_eq!(items[0].body_text, None);
    assert!(!items[0].detail_fetch_attempted);
}

#[test]
fn golden_parsed_company_listing_items() {
    // Golden ingestion pin (ADR 0069 / plan v0.55 T2): the sample listing JSON
    // must parse into a byte-stable set of items across the Fetcher migration.
    let items = parse_company_listing_json(&target(), JSON, "2026-05-31T10:00:00Z").expect("JSON");
    insta::assert_debug_snapshot!("golden_bankier_company_listing_items", items);
}

#[test]
fn skips_detail_fetches_when_filter_rejects_item() {
    let fetcher = DetailFilterFetcher {
        fetched_urls: RefCell::new(Vec::new()),
    };
    let target = BankierCompanyTarget {
        bankier_slug: Some("CDPROJEKT".to_owned()),
        bankier_tag_id: Some("722".to_owned()),
        ..target()
    };

    let (_, items) = fetch_company_items_with_detail_filter_at(
        &fetcher,
        &target,
        "2026-05-31T10:00:00Z",
        |item| item.article_id != "9141553",
    )
    .expect("items should fetch");

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].article_id, "9141553");
    assert_eq!(items[0].body_text, None);
    assert!(!items[0].detail_fetch_attempted);
    assert_eq!(items[1].body_text, Some("Second report body".to_owned()));
    assert!(items[1].detail_fetch_attempted);
    assert_eq!(
        fetcher
            .fetched_urls
            .borrow()
            .iter()
            .filter(|url| url.contains("/wiadomosc/"))
            .count(),
        1
    );
}

#[test]
fn parses_company_report_detail_body_and_attachments() {
    let html = r#"
        <html>
          <head>
            <script type="application/ld+json">
              {
                "@context": "https://schema.org",
                "@type": "NewsArticle",
                "headline": "CD PROJEKT SA: Wyniki finansowe QSr 1/2026",
                "articleBody": "Spis treści: 1. STRONA TYTUŁOWA STRONA TYTUŁOWA>>> KOMISJA NADZORU FINANSOWEGO Skonsolidowany raport kwartalny QSr"
              }
            </script>
          </head>
          <body>
            <nav>Giełda Wiadomości</nav>
            <article>
              <h1>CD PROJEKT SA: Wyniki finansowe QSr 1/2026</h1>
              <span>2026-05-28 17:33</span>
              <span>publikacja</span>
              <p>Spis treści:</p>
              <ol><li>STRONA TYTUŁOWA</li></ol>
              <p>Spis załączników:</p>
              <a href="https://bonnier.pl/report.xhtml">Raport XHTML</a>
              <a href="https://www.bankier.pl/regulamin.pdf">Regulamin</a>
              <a href="https://www.bankier.pl/prywatnosc.pdf">Polityka prywatności</a>
              <a href="https://www.bankier.pl/cookies.pdf">Polityka Cookies</a>
              <h4>STRONA TYTUŁOWA&gt;&gt;&gt;</h4>
              <p>KOMISJA NADZORU FINANSOWEGO</p>
              <p>Skonsolidowany raport kwartalny QSr</p>
              <p>Źródło:Komunikaty spółek (ESPI)</p>
              <p>Podziel się</p>
            </article>
          </body>
        </html>
    "#;

    let detail = parse_company_report_detail(
        html,
        "Wyniki finansowe QSr 1/2026",
        "https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-Wyniki-finansowe-QSr-1-2026-9141553.html",
    );

    assert_eq!(
        detail.body_text,
        Some(
            "Spis treści: 1. STRONA TYTUŁOWA STRONA TYTUŁOWA>>> KOMISJA NADZORU FINANSOWEGO Skonsolidowany raport kwartalny QSr"
                .to_owned()
        )
    );
    assert_eq!(
        detail.attachments,
        vec![BankierCompanyAttachment {
            label: "Raport XHTML".to_owned(),
            url: "https://bonnier.pl/report.xhtml".to_owned(),
            incomplete: false,
        }]
    );
}

/// Regression corpus case for #460: the PAS page's two attachments carry a
/// bare-filename href (no directory) alongside one proper `/static/att/`
/// link. The bare ones resolve under the article page, flagged
/// incomplete; the proper one is complete.
#[test]
fn parses_company_report_detail_with_incomplete_source_hrefs() {
    let html = r#"
        <html>
          <body>
            <article>
              <p>Spis załączników:</p>
              <a href="_2410_Passus_2023_PSSF_MSSF_skro%CC%81cone_PL-sig.pdf">Skonsolidowane SF</a>
              <a href="_2410_Passus_2023_PSF_MSSF_skro%CC%81cone_PL-sig.pdf">Jednostkowe SF</a>
              <a href="/static/att/emitent/2023-08/report.xhtml">Raport XHTML</a>
            </article>
          </body>
        </html>
    "#;
    let page_url = "https://www.bankier.pl/wiadomosc/Passus-SA-Wyniki-2023-9200001.html";

    let detail = parse_company_report_detail(html, "Passus H1 2023", page_url);

    assert_eq!(detail.attachments.len(), 3);
    let ssf = &detail.attachments[0];
    assert_eq!(
        ssf.url,
        "https://www.bankier.pl/wiadomosc/_2410_Passus_2023_PSSF_MSSF_skro%CC%81cone_PL-sig.pdf"
    );
    assert!(ssf.incomplete);
    let jsf = &detail.attachments[1];
    assert_eq!(
        jsf.url,
        "https://www.bankier.pl/wiadomosc/_2410_Passus_2023_PSF_MSSF_skro%CC%81cone_PL-sig.pdf"
    );
    assert!(jsf.incomplete);
    let xhtml = &detail.attachments[2];
    assert_eq!(
        xhtml.url,
        "https://www.bankier.pl/static/att/emitent/2023-08/report.xhtml"
    );
    assert!(!xhtml.incomplete);
}

/// Regression for #460: a `mailto:`/`javascript:` anchor passes the
/// coarse extension pre-filter (`mailto:x.pdf` ends in `.pdf`) but must
/// never become attachment metadata — only http(s)-scheme hrefs do.
#[test]
fn non_http_scheme_anchors_are_not_attachments() {
    let html = r#"
        <html>
          <body>
            <article>
              <p>Spis załączników:</p>
              <a href="mailto:x.pdf">Mail</a>
              <a href="javascript:void(0)">JS</a>
              <a href="//bonnier.pl/x.pdf">Protocol relative</a>
            </article>
          </body>
        </html>
    "#;
    let page_url = "https://www.bankier.pl/wiadomosc/Passus-SA-Wyniki-2023-9200001.html";

    let detail = parse_company_report_detail(html, "Passus H1 2023", page_url);

    assert_eq!(detail.attachments.len(), 1);
    assert_eq!(detail.attachments[0].url, "https://bonnier.pl/x.pdf");
}

#[test]
fn filters_company_listing_items_older_than_recent_window() {
    let json = r#"{
      "articles": [
        {
          "title": "CD PROJEKT SA: Recent report",
          "url": "/wiadomosc/recent-1.html",
          "time": "2026-05-30 10:00:00",
          "pub_id": 3,
          "article_id": 1,
          "messages_filters": []
        },
        {
          "title": "CD PROJEKT SA: Old report",
          "url": "/wiadomosc/old-2.html",
          "time": "2026-05-20 10:00:00",
          "pub_id": 3,
          "article_id": 2,
          "messages_filters": []
        }
      ]
    }"#;

    let items = parse_company_listing_json(&target(), json, "2026-05-31T10:00:00Z").expect("JSON");

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Recent report");
}

fn preset_target() -> BankierCompanyTarget {
    BankierCompanyTarget {
        bankier_slug: Some("CDPROJEKT".to_owned()),
        bankier_tag_id: Some("722".to_owned()),
        ..target()
    }
}

/// Every listing page returns one filing dated far in the future (never older
/// than any realistic cutoff) and never empty, so the walk can only end by
/// exhausting the page cap.
struct AlwaysRecentFetcher;
impl BankierCompanyFetcher for AlwaysRecentFetcher {
    fn fetch_text(&self, url: &str) -> Result<String, BankierCompanyError> {
        if url.starts_with(API_SOURCE_URL) {
            Ok(r#"{"articles":[{"title":"CD PROJEKT SA: Raport","url":"/wiadomosc/x-1.html","time":"2999-01-01 10:00:00","pub_id":3,"article_id":1,"messages_filters":["ESPI"]}]}"#.to_owned())
        } else {
            Ok("<html></html>".to_owned())
        }
    }
}

/// Page 1 carries a recent filing, page 2 is empty — the walk ends naturally
/// (out of filings) before the page cap.
struct RecentThenEmptyFetcher;
impl BankierCompanyFetcher for RecentThenEmptyFetcher {
    fn fetch_text(&self, url: &str) -> Result<String, BankierCompanyError> {
        if url.starts_with(API_SOURCE_URL) {
            if url.contains("/listing/1/") {
                return Ok(r#"{"articles":[{"title":"CD PROJEKT SA: Raport","url":"/wiadomosc/x-1.html","time":"2999-01-01 10:00:00","pub_id":3,"article_id":1,"messages_filters":["ESPI"]}]}"#.to_owned());
            }
            return Ok(r#"{"articles":[]}"#.to_owned());
        }
        Ok("<html></html>".to_owned())
    }
}

#[test]
fn backfill_reports_truncation_when_page_cap_ends_the_walk() {
    let (_, items, stats) = fetch_company_backfill_items(
        &AlwaysRecentFetcher,
        &preset_target(),
        "2000-01-01T00:00:00",
        3,
        std::time::Duration::ZERO,
        |_, _| {},
    )
    .expect("backfill fetch should succeed");

    assert_eq!(stats.pages_fetched, 3, "the page cap was exhausted");
    assert!(
        stats.truncated,
        "the page cap ended the walk before the cutoff was reached"
    );
    assert_eq!(items.len(), 3);
}

#[test]
fn backfill_reports_no_truncation_when_walk_ends_naturally() {
    let (_, _items, stats) = fetch_company_backfill_items(
        &RecentThenEmptyFetcher,
        &preset_target(),
        "2000-01-01T00:00:00",
        80,
        std::time::Duration::ZERO,
        |_, _| {},
    )
    .expect("backfill fetch should succeed");

    assert!(
        !stats.truncated,
        "running out of filings before the cap is not truncation"
    );
}
