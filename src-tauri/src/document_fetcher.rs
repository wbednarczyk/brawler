use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};
use std::time::Duration;
use thiserror::Error;

const DEFAULT_USER_AGENT: &str = "Brawler/1.0 (+https://github.com/wbednarczyk/brawler)";
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_DOCUMENT_SIZE: usize = 50 * 1024 * 1024; // 50 MB — ingest path, unchanged.

/// ADR 0093 decision 5 (epic #285 T8): the MCP `capture_report_document` act
/// lets a connected agent hand Brawler an arbitrary URL, so that path alone
/// gets the gates below. Every existing caller (source refresh, backfill,
/// autopilot, structured extraction, the UI capture command, report-diff
/// fetch-on-demand) keeps fetching ESPI/EBI/issuer URLs — some legacy `http://`
/// — with today's behavior byte-for-byte: `HttpDocumentFetcher::new()` is
/// unchanged and stays on [`FetchPolicy::Ingest`].
const AGENT_CAPTURE_MAX_DOCUMENT_SIZE: usize = 30 * 1024 * 1024; // 30 MiB (ADR 0093 dec. 5).
const AGENT_CAPTURE_ALLOWED_CONTENT_TYPES: &[&str] =
    &["application/pdf", "text/html", "application/xhtml+xml"];
const AGENT_CAPTURE_MAX_REDIRECTS: usize = 10; // matches reqwest's own default cap.

#[derive(Debug, Clone)]
pub struct FetchedDocument {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
}

#[derive(Debug, Error)]
pub enum DocumentFetcherError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("document size {size} exceeds maximum {max}")]
    DocumentTooLarge { size: usize, max: usize },
    #[error("invalid content type: {0}")]
    InvalidContentType(String),
    #[error("URL scheme {0:?} is not allowed for agent-captured documents; only https is")]
    DisallowedScheme(String),
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    #[error(
        "refusing to fetch {host}: resolves to a private/loopback/link-local/unspecified \
         address ({address}) — SSRF guard (ADR 0093 dec. 5)"
    )]
    PrivateAddress { host: String, address: String },
    #[error("DNS resolution failed for {host}: {source}")]
    DnsResolution {
        host: String,
        source: std::io::Error,
    },
    #[error("too many redirects (max {0}) while fetching an agent-captured document")]
    TooManyRedirects(usize),
    #[error("reading the response body failed: {0}")]
    StreamRead(#[from] std::io::Error),
}

pub trait DocumentFetcher {
    fn fetch(&self, url: &str) -> Result<FetchedDocument, DocumentFetcherError>;
}

/// Which gate set a fetch runs under (ADR 0093 dec. 5). `Ingest` is every
/// pre-existing caller's behavior, preserved byte-for-byte; `AgentCapture` is
/// the MCP `capture_report_document` act's gated path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FetchPolicy {
    Ingest,
    AgentCapture,
}

pub struct HttpDocumentFetcher {
    policy: FetchPolicy,
}

impl HttpDocumentFetcher {
    pub fn new() -> Self {
        Self {
            policy: FetchPolicy::Ingest,
        }
    }

    /// The MCP agent-capture path (ADR 0093 dec. 5): https-only, SSRF-guarded
    /// (every resolved address, on the initial request AND every redirect
    /// hop), content-type allowlisted, capped at 30 MiB enforced during
    /// streaming. Used ONLY by the `capture_report_document` MCP handler.
    pub fn agent_capture() -> Self {
        Self {
            policy: FetchPolicy::AgentCapture,
        }
    }
}

impl Default for HttpDocumentFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentFetcher for HttpDocumentFetcher {
    fn fetch(&self, url: &str) -> Result<FetchedDocument, DocumentFetcherError> {
        match self.policy {
            FetchPolicy::Ingest => fetch_ingest(url),
            FetchPolicy::AgentCapture => fetch_agent_capture(url),
        }
    }
}

/// Every pre-existing caller's behavior, untouched: any scheme, no SSRF
/// guard, whatever content-type the server sends, 50 MB cap checked after
/// the full body is buffered.
fn fetch_ingest(url: &str) -> Result<FetchedDocument, DocumentFetcherError> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(DEFAULT_USER_AGENT)
        .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
        .build()?;

    let response = client.get(url).send()?;
    let response = response.error_for_status()?;

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned());

    let bytes = response.bytes()?;
    let size = bytes.len();

    if size > MAX_DOCUMENT_SIZE {
        return Err(DocumentFetcherError::DocumentTooLarge {
            size,
            max: MAX_DOCUMENT_SIZE,
        });
    }

    Ok(FetchedDocument {
        bytes: bytes.to_vec(),
        content_type,
    })
}

/// The MCP agent-capture path (ADR 0093 dec. 5). Gates, in order: an
/// https-only and SSRF guard on the initial URL (no network yet), a redirect
/// policy that re-runs the SAME guard on every hop (reqwest follows
/// redirects by default — see [`agent_capture_redirect_policy`]), a
/// content-type allowlist checked on the response header before any body
/// bytes are read (except the deferred fallback below), and a 30 MiB cap
/// enforced while streaming (never trusting `Content-Length`, which the
/// server can lie about or omit).
///
/// `application/octet-stream` or a missing header defers the accept/refuse
/// decision until after the capped body is read, then accepts iff a
/// [`sniff_strict_allowed_container`] match (#455): the maintainer's corpus
/// proves octet-stream is the normal label for mislabelled ESEF/pdf2htmlEX
/// documents, so refusing on that header alone loses legitimate captures.
/// Every OTHER content type (e.g. `application/zip`, `text/xml`) is still
/// refused immediately, before any body byte is read — the fallback exists
/// only for the mislabelled/unlabelled case.
fn fetch_agent_capture(url: &str) -> Result<FetchedDocument, DocumentFetcherError> {
    validate_url_target(url, &SystemDnsResolver)?;

    let client = reqwest::blocking::Client::builder()
        .user_agent(DEFAULT_USER_AGENT)
        .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
        .redirect(agent_capture_redirect_policy())
        .build()?;

    let response = client.get(url).send()?;
    let response = response.error_for_status()?;

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned());
    let deferred = is_octet_stream_or_missing(content_type.as_deref());
    if !deferred {
        validate_content_type(content_type.as_deref())?;
    }

    let bytes = read_capped(response, AGENT_CAPTURE_MAX_DOCUMENT_SIZE)?;

    if deferred && !sniff_strict_allowed_container(&bytes) {
        return Err(DocumentFetcherError::InvalidContentType(
            content_type.unwrap_or_else(|| "missing content-type header".to_owned()),
        ));
    }

    Ok(FetchedDocument {
        bytes,
        content_type,
    })
}

/// Whether a response's content-type header defers to the strict body sniff
/// (`application/octet-stream`, with or without parameters, or absent).
fn is_octet_stream_or_missing(content_type: Option<&str>) -> bool {
    match content_type {
        None => true,
        Some(raw) => raw
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .eq_ignore_ascii_case("application/octet-stream"),
    }
}

/// Strict, offset-0-only sniff for the agent-capture mislabelled/unlabelled
/// fallback (#455). Deliberately narrower than
/// [`crate::fundamentals::extraction::container::detect_container`]: no 1 KiB
/// scan window for the PDF magic and no "any leading `<`" catch-all — only
/// the exact markers below, checked right after an optional BOM/leading
/// whitespace, so a mislabelled document is recognised without opening the
/// fallback to arbitrary bytes.
fn sniff_strict_allowed_container(bytes: &[u8]) -> bool {
    let trimmed = strip_bom_and_leading_whitespace(bytes);
    if trimmed.starts_with(b"%PDF-") || trimmed.starts_with(b"PK\x03\x04") {
        return true;
    }
    let lower: Vec<u8> = trimmed
        .iter()
        .take(16)
        .map(u8::to_ascii_lowercase)
        .collect();
    lower.starts_with(b"<?xml")
        || lower.starts_with(b"<!doctype html")
        || lower.starts_with(b"<html")
}

/// Strip a leading UTF-8/UTF-16 byte-order mark and ASCII whitespace.
fn strip_bom_and_leading_whitespace(bytes: &[u8]) -> &[u8] {
    const BOMS: [&[u8]; 3] = [b"\xEF\xBB\xBF", b"\xFF\xFE", b"\xFE\xFF"];
    let mut rest = bytes;
    for bom in BOMS {
        if let Some(stripped) = rest.strip_prefix(bom) {
            rest = stripped;
            break;
        }
    }
    let start = rest
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(rest.len());
    &rest[start..]
}

/// reqwest's blocking client follows redirects by default (up to 10 hops,
/// unvalidated) — several existing source adapters rely on exactly that
/// (BiznesRadar 301-redirects a ticker to its canonical slug). A naive SSRF
/// guard that only checks the ORIGINAL url is a no-op against a public URL
/// that 302s to `http://169.254.169.254/`. This custom policy re-runs
/// [`validate_url_target`] against every redirect target before following it
/// — the crux of the guard — and caps the hop count the same as reqwest's
/// own default.
fn agent_capture_redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= AGENT_CAPTURE_MAX_REDIRECTS {
            return attempt.error(DocumentFetcherError::TooManyRedirects(
                AGENT_CAPTURE_MAX_REDIRECTS,
            ));
        }
        match validate_url_target(attempt.url().as_str(), &SystemDnsResolver) {
            Ok(()) => attempt.follow(),
            Err(error) => attempt.error(error),
        }
    })
}

/// Resolve a hostname to its candidate addresses — abstracted so the SSRF
/// guard is unit-testable without DNS/network (inject a fake resolver).
trait DnsResolver {
    fn resolve(&self, host: &str) -> std::io::Result<Vec<IpAddr>>;
}

struct SystemDnsResolver;

impl DnsResolver for SystemDnsResolver {
    fn resolve(&self, host: &str) -> std::io::Result<Vec<IpAddr>> {
        // Port is irrelevant for a resolve-only lookup.
        Ok((host, 0u16)
            .to_socket_addrs()?
            .map(|addr| addr.ip())
            .collect())
    }
}

/// The SSRF guard (ADR 0093 dec. 5): https-only, then every address the host
/// resolves to must be public — a host resolving to BOTH a public and a
/// private address is rejected (checking only the first address a resolver
/// returns is not a guard). Called both for the initial URL and, from
/// [`agent_capture_redirect_policy`], for every redirect hop.
fn validate_url_target(url: &str, resolver: &dyn DnsResolver) -> Result<(), DocumentFetcherError> {
    let parsed = url::Url::parse(url)
        .map_err(|error| DocumentFetcherError::InvalidUrl(format!("{url}: {error}")))?;
    if parsed.scheme() != "https" {
        return Err(DocumentFetcherError::DisallowedScheme(
            parsed.scheme().to_owned(),
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| DocumentFetcherError::InvalidUrl(format!("{url}: missing host")))?
        .to_owned();

    let addresses =
        resolver
            .resolve(&host)
            .map_err(|source| DocumentFetcherError::DnsResolution {
                host: host.clone(),
                source,
            })?;
    if addresses.is_empty() {
        return Err(DocumentFetcherError::DnsResolution {
            host,
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "no addresses resolved"),
        });
    }
    for address in &addresses {
        if is_disallowed_address(*address) {
            return Err(DocumentFetcherError::PrivateAddress {
                host,
                address: address.to_string(),
            });
        }
    }
    Ok(())
}

/// Private (10/8, 172.16/12, 192.168/16), loopback (127/8, ::1), link-local
/// (169.254/16, fe80::/10), or unspecified — the exact ranges ADR 0093 dec. 5
/// names. An IPv4-mapped IPv6 address (`::ffff:a.b.c.d`) is checked against
/// the same IPv4 rules — otherwise it would be a bypass for the v4 ranges.
fn is_disallowed_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => is_disallowed_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_disallowed_v4(mapped);
            }
            v6.is_loopback() // ::1
                || v6.is_unspecified() // ::
                || (v6.segments()[0] & 0xffc0) == 0xfe80 // fe80::/10 (link-local)
        }
    }
}

fn is_disallowed_v4(v4: Ipv4Addr) -> bool {
    let octets = v4.octets();
    v4.is_loopback() // 127/8
        || v4.is_unspecified() // 0.0.0.0
        || v4.is_link_local() // 169.254/16
        || octets[0] == 10 // 10/8
        || (octets[0] == 172 && (16..=31).contains(&octets[1])) // 172.16/12
        || (octets[0] == 192 && octets[1] == 168) // 192.168/16
}

/// The content-type allowlist (ADR 0093 dec. 5), checked on the RESPONSE
/// header before any body bytes are read. Parameters are tolerated (e.g.
/// `text/html; charset=utf-8`) — only the media type before the `;` matters.
fn validate_content_type(content_type: Option<&str>) -> Result<(), DocumentFetcherError> {
    let raw = content_type.ok_or_else(|| {
        DocumentFetcherError::InvalidContentType("missing content-type header".to_owned())
    })?;
    let media_type = raw
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if AGENT_CAPTURE_ALLOWED_CONTENT_TYPES.contains(&media_type.as_str()) {
        Ok(())
    } else {
        Err(DocumentFetcherError::InvalidContentType(raw.to_owned()))
    }
}

/// Read `reader` into memory, aborting the moment the cumulative byte count
/// exceeds `cap` — enforced against the REAL bytes read, never a
/// `Content-Length` header (which can lie or be absent). Reads in bounded
/// chunks so an over-cap stream never has its full body pulled into memory.
fn read_capped(mut reader: impl Read, cap: usize) -> Result<Vec<u8>, DocumentFetcherError> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() > cap {
            return Err(DocumentFetcherError::DocumentTooLarge {
                size: buffer.len(),
                max: cap,
            });
        }
    }
    Ok(buffer)
}

#[cfg(test)]
pub struct FakeDocumentFetcher {
    pub response: Result<FetchedDocument, DocumentFetcherError>,
    /// How many times `fetch` was called — proves a healed/matched-identity
    /// capture (#455) never re-hits the network.
    pub calls: std::cell::Cell<usize>,
}

#[cfg(test)]
impl FakeDocumentFetcher {
    pub fn new_success(bytes: Vec<u8>, content_type: Option<String>) -> Self {
        Self {
            response: Ok(FetchedDocument {
                bytes,
                content_type,
            }),
            calls: std::cell::Cell::new(0),
        }
    }

    pub fn new_error(error: DocumentFetcherError) -> Self {
        Self {
            response: Err(error),
            calls: std::cell::Cell::new(0),
        }
    }
}

#[cfg(test)]
impl DocumentFetcher for FakeDocumentFetcher {
    fn fetch(&self, _url: &str) -> Result<FetchedDocument, DocumentFetcherError> {
        self.calls.set(self.calls.get() + 1);
        // Since DocumentFetcherError is not Clone, we can't clone the response.
        // The fake fetcher is only used in capture tests where we don't reuse it.
        match &self.response {
            Ok(doc) => Ok(FetchedDocument {
                bytes: doc.bytes.clone(),
                content_type: doc.content_type.clone(),
            }),
            Err(err) => Err(DocumentFetcherError::InvalidContentType(err.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A resolver that panics on any lookup not explicitly seeded — proves a
    /// gate short-circuits BEFORE reaching DNS when it should (e.g. the
    /// scheme check), and lets other tests assert exactly the host(s) looked
    /// up.
    struct FakeResolver {
        answers: HashMap<String, Vec<IpAddr>>,
    }

    impl FakeResolver {
        fn new(answers: &[(&str, &[IpAddr])]) -> Self {
            Self {
                answers: answers
                    .iter()
                    .map(|(host, addrs)| ((*host).to_owned(), addrs.to_vec()))
                    .collect(),
            }
        }

        fn empty() -> Self {
            Self::new(&[])
        }
    }

    impl DnsResolver for FakeResolver {
        fn resolve(&self, host: &str) -> std::io::Result<Vec<IpAddr>> {
            self.answers.get(host).cloned().ok_or_else(|| {
                panic!("unexpected DNS lookup for {host:?} — not seeded in this test")
            })
        }
    }

    const PUBLIC_V4: IpAddr = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));

    // ---- https-only ---------------------------------------------------

    #[test]
    fn http_scheme_is_rejected_without_a_dns_lookup() {
        // An empty resolver panics on ANY lookup — proving the scheme gate
        // runs (and refuses) before DNS is ever consulted.
        let resolver = FakeResolver::empty();
        let error = validate_url_target("http://example.com/doc.pdf", &resolver).unwrap_err();
        assert!(
            matches!(&error, DocumentFetcherError::DisallowedScheme(scheme) if scheme == "http"),
            "expected DisallowedScheme(\"http\"), got {error:?}"
        );
    }

    #[test]
    fn file_and_ftp_and_data_schemes_are_rejected() {
        let resolver = FakeResolver::empty();
        for url in [
            "file:///etc/passwd",
            "ftp://example.com/doc.pdf",
            "data:text/plain;base64,aGVsbG8=",
        ] {
            let error = validate_url_target(url, &resolver).unwrap_err();
            assert!(
                matches!(error, DocumentFetcherError::DisallowedScheme(_)),
                "{url}: expected DisallowedScheme, got {error:?}"
            );
        }
    }

    #[test]
    fn https_scheme_with_a_public_address_is_allowed() {
        let resolver = FakeResolver::new(&[("example.com", &[PUBLIC_V4])]);
        let result = validate_url_target("https://example.com/doc.pdf", &resolver);
        assert!(
            result.is_ok(),
            "public https URL must be allowed: {result:?}"
        );
    }

    // ---- SSRF guard: every resolved address ----------------------------

    #[test]
    fn private_ip_resolution_is_rejected() {
        let resolver = FakeResolver::new(&[(
            "internal.example",
            &[IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))],
        )]);
        let error = validate_url_target("https://internal.example/doc.pdf", &resolver).unwrap_err();
        assert!(
            matches!(error, DocumentFetcherError::PrivateAddress { .. }),
            "expected PrivateAddress, got {error:?}"
        );
    }

    /// The crux: a host resolving to BOTH a public and a private address must
    /// be rejected — checking only the first resolved address is not a guard.
    #[test]
    fn a_host_resolving_to_both_public_and_private_addresses_is_rejected() {
        let resolver = FakeResolver::new(&[(
            "mixed.example",
            &[PUBLIC_V4, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))],
        )]);
        let error = validate_url_target("https://mixed.example/doc.pdf", &resolver).unwrap_err();
        assert!(
            matches!(error, DocumentFetcherError::PrivateAddress { .. }),
            "expected PrivateAddress, got {error:?}"
        );
    }

    /// The exact function [`agent_capture_redirect_policy`] calls for every
    /// redirect hop — reqwest's `Attempt` has no public test constructor, so
    /// this is the faithful seam: prove the SAME guard rejects a redirect
    /// TARGET that resolves to a private address.
    #[test]
    fn redirect_target_resolving_to_a_private_address_is_rejected() {
        let resolver = FakeResolver::new(&[(
            "evil-redirect-target.example",
            &[IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))],
        )]);
        let error = validate_url_target("https://evil-redirect-target.example/metadata", &resolver)
            .unwrap_err();
        assert!(
            matches!(error, DocumentFetcherError::PrivateAddress { .. }),
            "a redirect hop resolving to a link-local address must be rejected: {error:?}"
        );
    }

    #[test]
    fn every_named_disallowed_range_is_rejected() {
        let addresses: &[IpAddr] = &[
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),      // loopback
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),        // unspecified
            IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1)),    // link-local
            IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3)),       // 10/8
            IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1)),     // 172.16/12 (low edge)
            IpAddr::V4(Ipv4Addr::new(172, 31, 255, 255)), // 172.16/12 (high edge)
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),    // 192.168/16
            "::1".parse().unwrap(),                       // loopback v6
            "::".parse().unwrap(),                        // unspecified v6
            "fe80::1".parse().unwrap(),                   // link-local v6
            "::ffff:10.0.0.1".parse().unwrap(),           // v4-mapped-v6 bypass attempt
        ];
        for address in addresses {
            assert!(
                is_disallowed_address(*address),
                "{address} must be disallowed"
            );
        }
    }

    #[test]
    fn addresses_just_outside_the_named_ranges_are_allowed() {
        let addresses: &[IpAddr] = &[
            PUBLIC_V4,
            IpAddr::V4(Ipv4Addr::new(172, 15, 255, 255)), // just below 172.16/12
            IpAddr::V4(Ipv4Addr::new(172, 32, 0, 0)),     // just above 172.16/12
            IpAddr::V4(Ipv4Addr::new(192, 169, 0, 1)),    // just outside 192.168/16
            "2001:4860:4860::8888".parse().unwrap(),      // a real public v6 (Google DNS)
        ];
        for address in addresses {
            assert!(
                !is_disallowed_address(*address),
                "{address} must be allowed"
            );
        }
    }

    // ---- content-type allowlist -----------------------------------------

    #[test]
    fn allowed_content_types_pass_with_or_without_parameters() {
        for content_type in [
            "application/pdf",
            "text/html",
            "text/html; charset=utf-8",
            "application/xhtml+xml",
            "APPLICATION/PDF", // case-insensitive
        ] {
            validate_content_type(Some(content_type))
                .unwrap_or_else(|error| panic!("{content_type} should be allowed: {error}"));
        }
    }

    #[test]
    fn disallowed_content_type_is_a_typed_refusal() {
        let error = validate_content_type(Some("application/json")).unwrap_err();
        assert!(
            matches!(error, DocumentFetcherError::InvalidContentType(_)),
            "expected InvalidContentType, got {error:?}"
        );
    }

    #[test]
    fn missing_content_type_is_a_typed_refusal() {
        let error = validate_content_type(None).unwrap_err();
        assert!(matches!(error, DocumentFetcherError::InvalidContentType(_)));
    }

    // ---- streaming size cap ------------------------------------------------

    #[test]
    fn read_capped_returns_bytes_under_the_cap() {
        let bytes = read_capped(std::io::Cursor::new(vec![1u8, 2, 3]), 10).expect("under cap");
        assert_eq!(bytes, vec![1, 2, 3]);
    }

    #[test]
    fn read_capped_allows_exactly_the_cap() {
        let data = vec![7u8; 10];
        let bytes = read_capped(std::io::Cursor::new(data.clone()), 10).expect("exactly at cap");
        assert_eq!(bytes, data);
    }

    /// Enforced against the ACTUAL bytes read, not a header: `std::io::repeat`
    /// is an unbounded stream (no `Content-Length` exists at all). If
    /// `read_capped` buffered the whole body before checking size, this test
    /// would never return; instead it must abort within a few chunks past the
    /// cap, proving the check runs during streaming.
    #[test]
    fn read_capped_aborts_an_unbounded_stream_promptly_after_the_cap() {
        let cap = 200_000;
        let result = read_capped(std::io::repeat(0u8), cap);
        match result {
            Err(DocumentFetcherError::DocumentTooLarge { size, max }) => {
                assert_eq!(max, cap);
                assert!(size > cap, "aborts once the cap is exceeded: {size}");
                assert!(
                    size < cap + 10 * 1024 * 1024,
                    "aborts promptly (within a few chunks), not after draining megabytes: {size}"
                );
            }
            other => panic!("expected DocumentTooLarge, got {other:?}"),
        }
    }

    #[test]
    fn agent_capture_cap_constant_is_30_mib() {
        assert_eq!(AGENT_CAPTURE_MAX_DOCUMENT_SIZE, 30 * 1024 * 1024);
    }

    #[test]
    fn ingest_cap_constant_is_unchanged_at_50_mb() {
        assert_eq!(MAX_DOCUMENT_SIZE, 50 * 1024 * 1024);
    }

    // ---- strict sniff fallback (#455): octet-stream/missing header defers
    // to a strict offset-0 sniff, never the broad `detect_container` window.

    #[test]
    fn octet_stream_header_is_deferred_missing_header_too() {
        assert!(is_octet_stream_or_missing(Some("application/octet-stream")));
        assert!(is_octet_stream_or_missing(Some(
            "APPLICATION/OCTET-STREAM; charset=binary"
        )));
        assert!(is_octet_stream_or_missing(None));
    }

    #[test]
    fn allowed_and_disallowed_headers_are_not_deferred() {
        assert!(!is_octet_stream_or_missing(Some("application/pdf")));
        assert!(!is_octet_stream_or_missing(Some("application/zip")));
    }

    #[test]
    fn sniff_strict_allowed_container_accepts_pdf_magic_at_offset_zero() {
        assert!(sniff_strict_allowed_container(b"%PDF-1.7\n..."));
    }

    #[test]
    fn sniff_strict_allowed_container_accepts_zip_magic() {
        assert!(sniff_strict_allowed_container(b"PK\x03\x04\x14\x00"));
    }

    #[test]
    fn sniff_strict_allowed_container_accepts_xml_declaration() {
        assert!(sniff_strict_allowed_container(
            b"<?xml version=\"1.0\"?><html></html>"
        ));
    }

    #[test]
    fn sniff_strict_allowed_container_accepts_html_doctype_and_bare_html_tag() {
        assert!(sniff_strict_allowed_container(
            b"<!DOCTYPE html><html></html>"
        ));
        assert!(sniff_strict_allowed_container(b"<html><body/></html>"));
        // Case-insensitive, matching the pipeline's own container sniff.
        assert!(sniff_strict_allowed_container(b"<!doctype html><HTML>"));
    }

    #[test]
    fn sniff_strict_allowed_container_tolerates_a_bom_and_leading_whitespace() {
        let mut bytes = b"\xEF\xBB\xBF".to_vec();
        bytes.extend_from_slice(b"\n\n  <?xml version=\"1.0\"?>");
        assert!(sniff_strict_allowed_container(&bytes));
    }

    #[test]
    fn sniff_strict_allowed_container_refuses_garbage_bytes() {
        assert!(!sniff_strict_allowed_container(b"not a real document"));
        assert!(!sniff_strict_allowed_container(b""));
    }

    /// The crux of "strict": unlike `detect_container`'s 1 KiB scan window,
    /// `%PDF-` appearing after junk bytes (not at true offset 0) must be
    /// refused — this fallback exists only for the mislabelled/unlabelled
    /// case the corpus proves, not as a general-purpose sniffer.
    #[test]
    fn sniff_strict_allowed_container_requires_offset_zero_unlike_the_broad_pdf_window() {
        let mut bytes = vec![0u8; 200];
        bytes.extend_from_slice(b"%PDF-1.4 rest");
        assert!(!sniff_strict_allowed_container(&bytes));
    }
}
