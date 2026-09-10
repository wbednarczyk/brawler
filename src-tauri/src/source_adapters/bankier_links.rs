//! Attachment/listing href resolution for the Bankier company adapter (#460).
//! Every non-absolute href resolves against its page URL via RFC 3986
//! joining (the `url` crate); only an href whose resolved scheme is http(s)
//! becomes a fetchable attachment link.

use url::Url;

/// Outcome of resolving one attachment `href`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedLink {
    pub url: String,
    /// The source's own link was incomplete (a bare filename, `./x`, `../x`,
    /// `dir/x` — anything short of absolute/protocol-relative/root-relative)
    /// and had to be joined against the page URL rather than trusted as-is;
    /// callers register it `metadata_only` and never fetch it (#460).
    pub incomplete: bool,
    /// Whether the resolved URL's scheme is http(s) — false for `mailto:`,
    /// `javascript:`, `data:`, and similar non-fetchable schemes (#460);
    /// callers skip registering such an href as an attachment at all.
    pub scheme_ok: bool,
}

/// Resolve one attachment `href` found on a Bankier article at `page_url`.
/// Absolute (`http(s)://`, case-insensitive scheme) and root-relative
/// (`/...`, including protocol-relative `//host/...`) hrefs resolve to a
/// real URL and are `incomplete: false`; anything else — the defective class
/// the source itself serves without a directory — resolves the same way but
/// is flagged `incomplete: true` so it is never fetched from a guessed
/// location. `scheme_ok` is true only when the resolved URL's scheme is
/// http(s) — callers use it to reject `mailto:`/`javascript:`/`data:`
/// anchors outright. A page URL or join that fails to parse never drops the
/// attachment: it falls back to the href verbatim, `incomplete: true`,
/// `scheme_ok: false`.
pub(crate) fn resolve_attachment_href(href: &str, page_url: &str) -> ResolvedLink {
    let href = href.trim();
    if is_http_scheme(href) {
        return ResolvedLink {
            url: href.to_owned(),
            incomplete: false,
            scheme_ok: true,
        };
    }

    let incomplete = !href.starts_with('/');
    match Url::parse(page_url).and_then(|base| base.join(href)) {
        Ok(joined) => ResolvedLink {
            scheme_ok: matches!(joined.scheme(), "http" | "https"),
            url: joined.to_string(),
            incomplete,
        },
        Err(_) => ResolvedLink {
            url: href.to_owned(),
            incomplete: true,
            scheme_ok: false,
        },
    }
}

/// Whether `value` parses as an absolute URL with an http(s) scheme — the
/// scheme comparison is case-insensitive because `Url::parse` normalizes it
/// to lowercase, so `HTTPS://...`/`Http://...` are detected the same as
/// lowercase (#460 finding: a string-prefix check missed those).
fn is_http_scheme(value: &str) -> bool {
    Url::parse(value)
        .map(|parsed| matches!(parsed.scheme(), "http" | "https"))
        .unwrap_or(false)
}

/// Resolve a Bankier komunikaty-listing item link (#460): absolute passes
/// through, everything else joins against the site root. `incomplete` has no
/// meaning for a listing link (it is not an attachment), so this returns a
/// plain `String`.
pub(crate) fn resolve_listing_link(href: &str) -> String {
    let href = href.trim();
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_owned();
    }

    Url::parse("https://www.bankier.pl/")
        .and_then(|base| base.join(href))
        .map(|joined| joined.to_string())
        .unwrap_or_else(|_| format!("https://www.bankier.pl{href}"))
}

/// Whether an anchor `href` looks like a report attachment worth registering
/// at all (a coarse pre-filter over `bonnier.pl`/`.pdf`/`.xhtml`/`.xades`).
pub(crate) fn is_report_attachment_url(value: &str) -> bool {
    let lower = value.to_lowercase();
    lower.contains("bonnier.pl")
        || lower.ends_with(".pdf")
        || lower.contains(".pdf?")
        || lower.ends_with(".xhtml")
        || lower.contains(".xhtml?")
        || lower.ends_with(".xades")
        || lower.contains(".xades?")
}

/// Whether an attachment URL is an ESEF/iXBRL digital-signature file (`.xades`).
/// A signature carries no financial data, so it is always registered
/// `metadata_only` (kept for audit/attribution) and never fetched (ADR 0061
/// decision 1b).
pub(crate) fn is_signature_attachment_url(value: &str) -> bool {
    let lower = value.to_lowercase();
    lower.ends_with(".xades") || lower.contains(".xades?")
}

/// Whether an attachment URL is a structured ESEF/iXBRL statement (`.xhtml`) —
/// the deterministic structured-extraction pipeline's preferred input (ADR
/// 0061 decision 1b). Structured attachments are always registered as fetch
/// candidates, independent of `is_periodic_report_item`: that classifier is
/// a Polish-language text heuristic over the filing title/body and can miss a
/// filing that is xhtml-only under the EU ESEF mandate.
pub(crate) fn is_structured_attachment_url(value: &str) -> bool {
    let lower = value.to_lowercase();
    lower.ends_with(".xhtml") || lower.contains(".xhtml?")
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const PAGE_URL: &str = "https://www.bankier.pl/wiadomosc/Passus-SA-9141553.html";

    #[test]
    fn signature_attachment_url_matches_only_xades() {
        assert!(is_signature_attachment_url(
            "https://bonnier.pl/static/att/emitent/2026-05/report.xades"
        ));
        assert!(is_signature_attachment_url(
            "https://bonnier.pl/static/att/emitent/2026-05/report.XAdES?v=1"
        ));
        assert!(!is_signature_attachment_url(
            "https://bonnier.pl/static/att/emitent/2026-05/report.xhtml"
        ));
        assert!(!is_signature_attachment_url(
            "https://bonnier.pl/static/att/emitent/2026-05/report.pdf"
        ));
    }

    #[test]
    fn structured_attachment_url_matches_only_xhtml() {
        assert!(is_structured_attachment_url(
            "https://bonnier.pl/static/att/emitent/2026-05/report.xhtml"
        ));
        assert!(is_structured_attachment_url(
            "https://bonnier.pl/static/att/emitent/2026-05/report.XHTML?v=1"
        ));
        assert!(!is_structured_attachment_url(
            "https://bonnier.pl/static/att/emitent/2026-05/report.xades"
        ));
        assert!(!is_structured_attachment_url(
            "https://bonnier.pl/static/att/emitent/2026-05/report.pdf"
        ));
    }

    #[test]
    fn bare_filename_resolves_under_the_page_host_and_is_incomplete() {
        let resolved = resolve_attachment_href("_2410_x.pdf", PAGE_URL);
        assert_eq!(resolved.url, "https://www.bankier.pl/wiadomosc/_2410_x.pdf");
        assert!(resolved.incomplete);
    }

    #[test]
    fn bare_filename_keeps_its_leading_underscore() {
        let resolved = resolve_attachment_href(
            "_2410_Passus_2023_PSSF_MSSF_skro%CC%81cone_PL-sig.pdf",
            PAGE_URL,
        );
        assert_eq!(
            resolved.url,
            "https://www.bankier.pl/wiadomosc/_2410_Passus_2023_PSSF_MSSF_skro%CC%81cone_PL-sig.pdf"
        );
        assert!(resolved.incomplete);
    }

    #[test]
    fn dot_relative_href_is_incomplete() {
        let resolved = resolve_attachment_href("./x.pdf", PAGE_URL);
        assert_eq!(resolved.url, "https://www.bankier.pl/wiadomosc/x.pdf");
        assert!(resolved.incomplete);
    }

    #[test]
    fn dot_dot_relative_href_is_incomplete() {
        let resolved = resolve_attachment_href("../x.pdf", PAGE_URL);
        assert_eq!(resolved.url, "https://www.bankier.pl/x.pdf");
        assert!(resolved.incomplete);
    }

    #[test]
    fn directory_relative_href_is_incomplete() {
        let resolved = resolve_attachment_href("dir/x.pdf", PAGE_URL);
        assert_eq!(resolved.url, "https://www.bankier.pl/wiadomosc/dir/x.pdf");
        assert!(resolved.incomplete);
    }

    #[test]
    fn root_relative_href_is_complete() {
        let resolved = resolve_attachment_href("/static/att/emitent/2026-05/x.pdf", PAGE_URL);
        assert_eq!(
            resolved.url,
            "https://www.bankier.pl/static/att/emitent/2026-05/x.pdf"
        );
        assert!(!resolved.incomplete);
    }

    #[test]
    fn absolute_https_href_passes_through_complete() {
        let resolved = resolve_attachment_href("https://bonnier.pl/x.pdf", PAGE_URL);
        assert_eq!(resolved.url, "https://bonnier.pl/x.pdf");
        assert!(!resolved.incomplete);
    }

    #[test]
    fn absolute_http_href_passes_through_complete() {
        let resolved = resolve_attachment_href("http://bonnier.pl/x.pdf", PAGE_URL);
        assert_eq!(resolved.url, "http://bonnier.pl/x.pdf");
        assert!(!resolved.incomplete);
    }

    #[test]
    fn uppercase_https_scheme_href_passes_through_complete() {
        let resolved = resolve_attachment_href("HTTPS://bonnier.pl/report.pdf", PAGE_URL);
        assert_eq!(resolved.url, "HTTPS://bonnier.pl/report.pdf");
        assert!(!resolved.incomplete);
        assert!(resolved.scheme_ok);
    }

    #[test]
    fn mixed_case_http_scheme_href_passes_through_complete() {
        let resolved = resolve_attachment_href("Http://bonnier.pl/report.pdf", PAGE_URL);
        assert_eq!(resolved.url, "Http://bonnier.pl/report.pdf");
        assert!(!resolved.incomplete);
        assert!(resolved.scheme_ok);
    }

    #[test]
    fn protocol_relative_href_resolves_to_https_and_is_complete() {
        let resolved = resolve_attachment_href("//bonnier.pl/x.pdf", PAGE_URL);
        assert_eq!(resolved.url, "https://bonnier.pl/x.pdf");
        assert!(!resolved.incomplete);
    }

    #[test]
    fn mailto_scheme_href_is_not_scheme_ok() {
        let resolved = resolve_attachment_href("mailto:report.pdf", PAGE_URL);
        assert!(!resolved.scheme_ok);
    }

    #[test]
    fn javascript_scheme_href_is_not_scheme_ok() {
        let resolved = resolve_attachment_href("javascript:void(0)", PAGE_URL);
        assert!(!resolved.scheme_ok);
    }

    #[test]
    fn bare_filename_with_query_string_is_incomplete() {
        let resolved = resolve_attachment_href("x.pdf?v=1", PAGE_URL);
        assert_eq!(resolved.url, "https://www.bankier.pl/wiadomosc/x.pdf?v=1");
        assert!(resolved.incomplete);
    }

    #[test]
    fn unparseable_page_url_falls_back_to_the_href_verbatim() {
        let resolved = resolve_attachment_href("_2410_x.pdf", "not a url");
        assert_eq!(resolved.url, "_2410_x.pdf");
        assert!(resolved.incomplete);
    }

    #[test]
    fn listing_link_resolves_root_relative_hrefs() {
        assert_eq!(
            resolve_listing_link("/wiadomosc/x-1.html"),
            "https://www.bankier.pl/wiadomosc/x-1.html"
        );
    }

    #[test]
    fn listing_link_resolves_bare_hrefs_against_the_site_root() {
        assert_eq!(
            resolve_listing_link("wiadomosc/x-1.html"),
            "https://www.bankier.pl/wiadomosc/x-1.html"
        );
    }

    #[test]
    fn listing_link_passes_through_absolute_hrefs() {
        assert_eq!(
            resolve_listing_link("https://www.bankier.pl/wiadomosc/x-1.html"),
            "https://www.bankier.pl/wiadomosc/x-1.html"
        );
    }

    /// Golden fixture table (ADR 0049) locking the shape of every case above
    /// plus the two listing-link cases, reviewed via `cargo insta review` on
    /// a deliberate shape change.
    #[test]
    fn golden_resolved_links_fixture_table() {
        let attachment_cases: Vec<(&str, &str, ResolvedLink)> = vec![
            (
                "_2410_x.pdf",
                PAGE_URL,
                resolve_attachment_href("_2410_x.pdf", PAGE_URL),
            ),
            (
                "./x.pdf",
                PAGE_URL,
                resolve_attachment_href("./x.pdf", PAGE_URL),
            ),
            (
                "../x.pdf",
                PAGE_URL,
                resolve_attachment_href("../x.pdf", PAGE_URL),
            ),
            (
                "dir/x.pdf",
                PAGE_URL,
                resolve_attachment_href("dir/x.pdf", PAGE_URL),
            ),
            (
                "/static/att/emitent/2026-05/x.pdf",
                PAGE_URL,
                resolve_attachment_href("/static/att/emitent/2026-05/x.pdf", PAGE_URL),
            ),
            (
                "https://bonnier.pl/x.pdf",
                PAGE_URL,
                resolve_attachment_href("https://bonnier.pl/x.pdf", PAGE_URL),
            ),
            (
                "http://bonnier.pl/x.pdf",
                PAGE_URL,
                resolve_attachment_href("http://bonnier.pl/x.pdf", PAGE_URL),
            ),
            (
                "//bonnier.pl/x.pdf",
                PAGE_URL,
                resolve_attachment_href("//bonnier.pl/x.pdf", PAGE_URL),
            ),
            (
                "x.pdf?v=1",
                PAGE_URL,
                resolve_attachment_href("x.pdf?v=1", PAGE_URL),
            ),
            (
                "_2410_x.pdf",
                "not a url",
                resolve_attachment_href("_2410_x.pdf", "not a url"),
            ),
        ];
        insta::assert_debug_snapshot!("golden_resolved_attachment_links", attachment_cases);

        let listing_cases: Vec<(&str, String)> = vec![
            (
                "/wiadomosc/x-1.html",
                resolve_listing_link("/wiadomosc/x-1.html"),
            ),
            (
                "wiadomosc/x-1.html",
                resolve_listing_link("wiadomosc/x-1.html"),
            ),
        ];
        insta::assert_debug_snapshot!("golden_resolved_listing_links", listing_cases);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// (a) For relative hrefs drawn from a safe filename charset (the class
        /// #460 actually hit — no absolute/protocol-relative markers), the
        /// result always parses as a URL whose host is the page's own host:
        /// the href can never again land IN the host, unlike the pre-#460 bug.
        #[test]
        fn relative_hrefs_resolve_under_the_page_host(href in "[A-Za-z0-9_.-]{1,40}") {
            let resolved = resolve_attachment_href(&href, PAGE_URL);
            let parsed = Url::parse(&resolved.url).expect("must always parse to a URL");
            // The host is always the page's fixed host — never derived from
            // the href — so a filename can never again land IN the host the
            // way the pre-#460 string-concatenation bug produced it.
            prop_assert_eq!(parsed.host_str(), Some("www.bankier.pl"));
        }

        /// (b) Totality + determinism: no input, however malformed, panics,
        /// and the function is a pure mapping (same input, same output).
        #[test]
        fn resolve_attachment_href_is_total_and_deterministic(
            href in ".{0,80}",
            page_url in ".{0,80}",
        ) {
            let first = resolve_attachment_href(&href, &page_url);
            let second = resolve_attachment_href(&href, &page_url);
            prop_assert_eq!(first, second);
        }

        /// (b) With an unparseable page URL, a non-absolute href can never be
        /// resolved — the outcome preserves the href verbatim rather than
        /// dropping or mangling it.
        #[test]
        fn unparseable_page_url_preserves_non_absolute_hrefs_verbatim(
            href in "[A-Za-z0-9_./-]{0,60}",
        ) {
            prop_assume!(!href.starts_with("http://") && !href.starts_with("https://"));
            let resolved = resolve_attachment_href(&href, "");
            prop_assert_eq!(resolved.url, href);
            prop_assert!(resolved.incomplete);
        }
    }
}
