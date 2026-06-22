//! Report text extraction for the report-over-report diff (v0.47.0, ADR 0052).
//!
//! Pure-Rust, deterministic, dual-format extraction of a stored financial-statement
//! report document into ordered text sections — the substrate the deterministic
//! section diff compares. No AI, no network.
//!
//! Validated against the whole GPW + NewConnect market (613 real reports, ADR 0052),
//! which is why this module is built around three robustness requirements rather
//! than the happy path:
//!   1. **Panic-safe.** `pdf-extract` panics on a small fraction of real PDFs; the
//!      call is wrapped in `catch_unwind` and a panic becomes `ExtractionFailed`,
//!      never crashing the offloaded job.
//!   2. **Dual-format.** PDF (`pdf-extract`) **and** ESEF/iXBRL `.xhtml` (HTML parse,
//!      stripping the inline-XBRL header / hidden facts) — large issuers file
//!      xhtml-only under the EU ESEF mandate.
//!   3. **No-text-layer by density.** A scanned/image report yields little text per
//!      page; below a chars-per-page threshold it is `NoTextLayer` (not-diffable;
//!      OCR is out of scope), independent of alpha-ratio (number-dense statements
//!      are fine).

use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Bump when the extraction heuristic changes so stored sections are rebuilt.
pub const EXTRACTOR_VERSION: i64 = 1;

/// Below this many characters per page a PDF is treated as scanned/no-text-layer.
/// Tuned on the 613-report market corpus, where genuine scans sit far below and
/// real statements far above this line (ADR 0052).
const MIN_CHARS_PER_PAGE: usize = 200;
/// xhtml has no page model; a real ESEF statement is large, an announcement tiny.
const MIN_XHTML_CHARS: usize = 3000;

/// Source format of a stored report document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    Pdf,
    Xhtml,
}

impl SourceFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceFormat::Pdf => "pdf",
            SourceFormat::Xhtml => "xhtml",
        }
    }

    /// Resolve the format from a content type and/or local path/URL.
    pub fn resolve(content_type: Option<&str>, path_or_url: &str) -> SourceFormat {
        let lower = path_or_url.to_ascii_lowercase();
        let ct = content_type.unwrap_or("").to_ascii_lowercase();
        if lower.ends_with(".xhtml")
            || lower.ends_with(".html")
            || ct.contains("xhtml")
            || ct.contains("html")
        {
            SourceFormat::Xhtml
        } else {
            SourceFormat::Pdf
        }
    }
}

/// The outcome of extracting one report document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionState {
    /// Clean, sectionable text — the only diffable state.
    Extracted,
    /// Scanned/image or otherwise text-poor: density below threshold. Not diffable; no OCR.
    NoTextLayer,
    /// A caught `pdf-extract` panic or unreadable input — flagged, never crashes the job.
    ExtractionFailed,
}

impl ExtractionState {
    pub fn as_str(self) -> &'static str {
        match self {
            ExtractionState::Extracted => "extracted",
            ExtractionState::NoTextLayer => "no_text_layer",
            ExtractionState::ExtractionFailed => "extraction_failed",
        }
    }
}

/// One detected section of a document, in document order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub ordinal: i64,
    pub heading: String,
    pub body: String,
}

/// Full result of extracting a document.
#[derive(Debug, Clone)]
pub struct ExtractionOutcome {
    pub format: SourceFormat,
    pub state: ExtractionState,
    pub char_count: usize,
    pub sections: Vec<Section>,
}

/// Extract a stored report document's bytes into ordered text sections.
///
/// Deterministic: the same bytes always produce the same outcome. Panic-safe for
/// PDFs. Never returns an error — an unreadable document yields `ExtractionFailed`
/// with no sections, so callers always get a flagged outcome rather than a crash.
pub fn extract_report(bytes: &[u8], format: SourceFormat) -> ExtractionOutcome {
    match format {
        SourceFormat::Pdf => extract_pdf(bytes),
        SourceFormat::Xhtml => extract_xhtml(bytes),
    }
}

fn extract_pdf(bytes: &[u8]) -> ExtractionOutcome {
    // pdf-extract panics on some valid PDFs; isolate it. It also prints glyph
    // warnings to stdout, which we gag for the duration of the call.
    let result = std::panic::catch_unwind(|| {
        with_silenced_stdout(|| pdf_extract::extract_text_from_mem_by_pages(bytes))
    });

    let pages = match result {
        Ok(Ok(pages)) => pages,
        // Err code path (parse error) or a caught panic both mean "could not extract".
        _ => {
            return ExtractionOutcome {
                format: SourceFormat::Pdf,
                state: ExtractionState::ExtractionFailed,
                char_count: 0,
                sections: Vec::new(),
            };
        }
    };

    let page_count = pages.len().max(1);
    let text = pages.join("\n");
    let char_count = text.chars().count();
    // Density, not alpha-ratio: a scanned report yields few chars per page.
    if char_count / page_count < MIN_CHARS_PER_PAGE {
        return ExtractionOutcome {
            format: SourceFormat::Pdf,
            state: ExtractionState::NoTextLayer,
            char_count,
            sections: Vec::new(),
        };
    }

    ExtractionOutcome {
        format: SourceFormat::Pdf,
        state: ExtractionState::Extracted,
        char_count,
        sections: split_sections(&text),
    }
}

fn extract_xhtml(bytes: &[u8]) -> ExtractionOutcome {
    let raw = match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    };
    let text = html_to_text(&raw);
    let char_count = text.chars().count();
    if char_count < MIN_XHTML_CHARS {
        return ExtractionOutcome {
            format: SourceFormat::Xhtml,
            state: ExtractionState::NoTextLayer,
            char_count,
            sections: Vec::new(),
        };
    }
    ExtractionOutcome {
        format: SourceFormat::Xhtml,
        state: ExtractionState::Extracted,
        char_count,
        sections: split_sections(&text),
    }
}

// ============================================================================
// ESEF / iXBRL xhtml -> text
// ============================================================================

/// ESEF/iXBRL `.xhtml` → readable text. Drops scripts/styles and the hidden
/// inline-XBRL header (`display:none` blocks carry XBRL contexts/units, not report
/// prose), inserts line breaks at block boundaries, strips tags, decodes entities.
fn html_to_text(html: &str) -> String {
    let mut s = html.to_owned();
    for re in drop_block_regexes() {
        s = re.replace_all(&s, "\n").into_owned();
    }
    let hidden = hidden_div_regex();
    for _ in 0..3 {
        s = hidden.replace_all(&s, "\n").into_owned();
    }
    s = block_close_regex().replace_all(&s, "\n").into_owned();
    s = tag_regex().replace_all(&s, " ").into_owned();
    s = s
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    s.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

// ============================================================================
// Section splitting (shared by both formats)
// ============================================================================

/// Split extracted text into ordered sections at detected headings. Deterministic.
pub fn split_sections(text: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut heading = "<preamble>".to_string();
    let mut body = String::new();
    let emit = |heading: &str, body: &str, sections: &mut Vec<Section>| {
        sections.push(Section {
            ordinal: sections.len() as i64,
            heading: heading.to_owned(),
            body: body.to_owned(),
        });
    };
    for line in text.lines() {
        if let Some(key) = heading_key(line) {
            if !body.trim().is_empty() || heading != "<preamble>" {
                emit(&heading, &body, &mut sections);
                body.clear();
            }
            heading = key;
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    emit(&heading, &body, &mut sections);
    sections
}

/// If `line` looks like a section heading, return its normalized key
/// (leading numbering stripped, lowercased); else `None`. Heuristic tuned for
/// Polish financial statements.
fn heading_key(line: &str) -> Option<String> {
    let t = line.trim();
    if t.chars().count() < 3 || t.chars().count() > 90 {
        return None;
    }
    let letters: Vec<char> = t.chars().filter(|c| c.is_alphabetic()).collect();
    let is_caps = letters.len() >= 4 && letters.iter().all(|c| c.is_uppercase());
    if numbered_heading_regex().is_match(t) || known_heading_regex().is_match(t) || is_caps {
        let key = strip_numbering_regex().replace(t, "");
        return Some(key.trim().to_lowercase());
    }
    None
}

// ============================================================================
// Compiled regexes (cached)
// ============================================================================

fn drop_block_regexes() -> &'static [Regex] {
    static RES: OnceLock<Vec<Regex>> = OnceLock::new();
    RES.get_or_init(|| {
        [
            r"(?is)<script\b.*?</script>",
            r"(?is)<style\b.*?</style>",
            r"(?is)<ix:header\b.*?</ix:header>",
            r"(?is)<ix:references\b.*?</ix:references>",
            r"(?is)<ix:resources\b.*?</ix:resources>",
        ]
        .iter()
        .map(|p| Regex::new(p).expect("valid regex"))
        .collect()
    })
}

fn hidden_div_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?is)<div[^>]*display\s*:\s*none[^>]*>.*?</div>").expect("valid regex")
    })
}

fn block_close_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)</(p|div|tr|h[1-6]|li|table|section|article|td|th)>|<br\s*/?>")
            .expect("valid regex")
    })
}

fn tag_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<[^>]+>").expect("valid regex"))
}

fn numbered_heading_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d+\.){1,4}\d*\.?\s+\p{Lu}").expect("valid regex"))
}

fn known_heading_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)^(wybrane dane finansow|sprawozdanie zarząd|sprawozdanie finansow|skonsolidowane sprawozdanie|jednostkowe sprawozdanie|noty objaśniając|informacje? (dodatkow|ogóln)|rachunek (zysków|przepływ)|bilans|zasady (rachunkowości|przyjęte)|pozostałe informacj|oświadczenie zarząd|list (prezesa|do akcjonariusz)|komentarz zarząd|wprowadzenie)",
        )
        .expect("valid regex")
    })
}

fn strip_numbering_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d+\.)+\s*").expect("valid regex"))
}

// ============================================================================
// stdout silencing (pdf-extract prints glyph warnings via raw println!)
// ============================================================================

/// Run `f` with the process stdout fd temporarily redirected to /dev/null, so a
/// third-party library's `println!` noise never reaches the app's stdout. No-op on
/// non-unix targets. ADR 0052 requires extraction not to emit `pdf-extract`'s
/// glyph logging.
#[cfg(unix)]
fn with_silenced_stdout<T>(f: impl FnOnce() -> T) -> T {
    use std::os::fd::RawFd;
    // SAFETY: dup/dup2/close on the stdout fd around a synchronous call, restoring
    // the original fd before returning. The /dev/null fd is closed after restore.
    unsafe {
        let stdout_fd: RawFd = 1;
        let saved = libc::dup(stdout_fd);
        if saved < 0 {
            return f();
        }
        let devnull = libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY);
        if devnull < 0 {
            libc::close(saved);
            return f();
        }
        libc::dup2(devnull, stdout_fd);
        libc::close(devnull);
        let out = f();
        libc::dup2(saved, stdout_fd);
        libc::close(saved);
        out
    }
}

#[cfg(not(unix))]
fn with_silenced_stdout<T>(f: impl FnOnce() -> T) -> T {
    f()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreadable_pdf_is_flagged_not_panicked() {
        // Garbage bytes are not a valid PDF: pdf-extract errors (or, on some inputs,
        // panics). Either way the outcome is ExtractionFailed with no sections —
        // the job must never crash. This is the panic-safety guard (ADR 0052).
        let outcome = extract_report(b"this is not a pdf at all", SourceFormat::Pdf);
        assert_eq!(outcome.state, ExtractionState::ExtractionFailed);
        assert!(outcome.sections.is_empty());
    }

    #[test]
    fn tiny_xhtml_is_no_text_layer() {
        let outcome = extract_report(b"<html><body><p>hi</p></body></html>", SourceFormat::Xhtml);
        assert_eq!(outcome.state, ExtractionState::NoTextLayer);
    }

    #[test]
    fn xhtml_extracts_text_and_sections() {
        let body = format!(
            "<html><body><h1>Bilans</h1><p>{}</p><h1>Rachunek zysków i strat</h1><p>przychody</p></body></html>",
            "aktywa trwałe ".repeat(400)
        );
        let outcome = extract_report(body.as_bytes(), SourceFormat::Xhtml);
        assert_eq!(outcome.state, ExtractionState::Extracted);
        // The two <h1> headings become section boundaries.
        let headings: Vec<&str> = outcome
            .sections
            .iter()
            .map(|s| s.heading.as_str())
            .collect();
        assert!(headings.iter().any(|h| h.contains("bilans")));
        assert!(headings.iter().any(|h| h.contains("rachunek")));
    }

    #[test]
    fn xhtml_drops_inline_xbrl_header_and_hidden_facts() {
        let html = "<html><body>\
            <ix:header><ix:hidden>9999 xbrl context noise</ix:hidden></ix:header>\
            <div style=\"display:none\">hidden fact 12345</div>\
            <p>visible report text</p></body></html>";
        let text = html_to_text(html);
        assert!(text.contains("visible report text"));
        assert!(!text.contains("xbrl context noise"));
        assert!(!text.contains("hidden fact 12345"));
    }

    #[test]
    fn split_sections_is_deterministic() {
        let text = "WPROWADZENIE\nbody one\n1. Bilans\naktywa\nNOTY OBJAŚNIAJĄCE\nnota";
        let a = split_sections(text);
        let b = split_sections(text);
        assert_eq!(a, b);
        // Ordinals are 0-based and contiguous.
        for (i, section) in a.iter().enumerate() {
            assert_eq!(section.ordinal, i as i64);
        }
    }

    #[test]
    fn format_resolves_from_extension_and_content_type() {
        assert_eq!(
            SourceFormat::resolve(None, "x/report.xhtml"),
            SourceFormat::Xhtml
        );
        assert_eq!(
            SourceFormat::resolve(None, "x/report.pdf"),
            SourceFormat::Pdf
        );
        assert_eq!(
            SourceFormat::resolve(Some("application/xhtml+xml"), "x/report"),
            SourceFormat::Xhtml
        );
    }
}
