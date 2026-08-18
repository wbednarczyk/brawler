//! ESEF report-package (ZIP) handling — locate the inline-XBRL instance (ADR 0061
//! decision 1; the T7-C follow-up the ADR flagged as out of scope for the initial
//! slice: "unpacking a ZIP package is a separate follow-up").
//!
//! A GPW ESEF annual filing is delivered as an **ESEF report package**: a ZIP
//! whose extension is `.xbri` (per the xbrl.org report-package spec) or `.zip`,
//! bundling a taxonomy plus the inline-XBRL **instance document** under a
//! top-level `reports/` folder. The rest of the pipeline (`esef::parse_esef`)
//! parses the bare instance xHTML; this module is the thin seam that pulls that
//! instance out of the container so the deterministic ESEF tier can see it. It is
//! pure over `&[u8]` (an in-memory ZIP read) — no filesystem IO, fully testable.

use std::collections::HashMap;
use std::io::Read;

use super::esef::{attr, is_inline_xbrl, local_of};

/// PKZIP local-file-header magic — the first bytes of every ZIP container.
const ZIP_MAGIC: &[u8] = b"PK\x03\x04";

/// Upper bound on an unpacked instance we will read into memory. Real ESEF
/// instances run to a few MB (CBF's FY2025 is ~10 MB); this cap keeps a hostile
/// or corrupt package from ballooning memory while staying far above any real
/// filing.
const MAX_INSTANCE_BYTES: u64 = 64 * 1024 * 1024;

/// True when the stored bytes/extension denote an ESEF report package (a ZIP
/// container). The extension is the primary signal (`.xbri`/`.zip`); the ZIP
/// magic is a fallback for a package delivered with a misleading extension or a
/// generic `application/octet-stream` content type (exactly how the maintainer's
/// real `.xbri` was stored).
pub fn is_report_package(path: &str, bytes: &[u8]) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".xbri") || lower.ends_with(".zip") || bytes.starts_with(ZIP_MAGIC)
}

/// Extract the inline-XBRL instance document bytes from an ESEF report package.
///
/// Per the xbrl.org report-package spec the instance document(s) live in the
/// top-level `reports/` folder, so an entry under a `reports/` path wins; among
/// candidates the **largest** `.xhtml`/`.html` entry is chosen (the primary
/// statement document dwarfs any auxiliary page). Falls back to the largest
/// xhtml anywhere when nothing sits under `reports/`. `None` when the bytes are
/// not a readable ZIP, hold no xhtml entry, or the instance exceeds
/// [`MAX_INSTANCE_BYTES`].
pub fn extract_instance(bytes: &[u8]) -> Option<Vec<u8>> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader).ok()?;

    // Pick the best candidate index first (an immutable scan), then read it —
    // `ZipArchive::by_index` borrows the archive mutably, so we cannot hold a
    // file handle open across the scan.
    let mut best: Option<(bool, u64, usize)> = None; // (under reports/, size, index)
    for i in 0..archive.len() {
        let file = archive.by_index(i).ok()?;
        if !file.is_file() {
            continue;
        }
        let name = file.name().to_ascii_lowercase();
        if !(name.ends_with(".xhtml") || name.ends_with(".html")) {
            continue;
        }
        let in_reports = name.starts_with("reports/") || name.contains("/reports/");
        let size = file.size();
        // Order candidates by (under reports/, size); `false < true` so a
        // reports/ entry outranks a sibling, larger outranks smaller within.
        let candidate = (in_reports, size, i);
        if best.is_none_or(|b| (b.0, b.1) < (candidate.0, candidate.1)) {
            best = Some(candidate);
        }
    }

    let (_, size, idx) = best?;
    if size > MAX_INSTANCE_BYTES {
        return None;
    }
    let mut file = archive.by_index(idx).ok()?;
    let mut out = Vec::with_capacity(size as usize);
    file.read_to_end(&mut out).ok()?;
    Some(out)
}

// ---------------------------------------------------------------------------
// Layer 1 (ADR 0100 decisions 1, 3, 9; epic #398): every instance, every role
// ---------------------------------------------------------------------------

/// Every readable iXBRL instance in the package, path + bytes, for Layer 1
/// capture (ADR 0100 decision 1's "the package reader loses whole filings"
/// defect fix). A package legitimately contains several instances — one
/// issuer files standalone and consolidated separately, another ships a
/// tagged management report alongside the statements — where [`extract_instance`]
/// (Layer 2, unchanged by this) keeps only the single largest one. An `.xhtml`/
/// `.html` entry that is not itself inline-XBRL (a decoy page, a rendered
/// cover) is filtered out by the same [`is_inline_xbrl`] sniff the container
/// router uses, so routing and this enumeration can never disagree on what
/// counts as an instance. Deterministic order (sorted by entry name).
pub fn extract_all_instances(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    extract_all_instances_counted(bytes).0
}

/// [`extract_all_instances`] plus the count of candidate entries that were
/// SKIPPED (unreadable, or over [`MAX_INSTANCE_BYTES`]) — sol review finding
/// 4: a silently skipped entry made "encountered = stored" pass while whole
/// instances went missing; the caller records the count so the extraction is
/// marked truncated, never complete-looking.
pub(crate) fn extract_all_instances_counted(bytes: &[u8]) -> (Vec<(String, Vec<u8>)>, i64) {
    let reader = std::io::Cursor::new(bytes);
    let Ok(mut archive) = zip::ZipArchive::new(reader) else {
        return (Vec::new(), 0);
    };

    let mut names: Vec<String> = Vec::new();
    let mut skipped: i64 = 0;
    for i in 0..archive.len() {
        let Ok(file) = archive.by_index(i) else {
            // An unreadable entry might have been an instance — count it, so
            // the extraction is marked truncated, never complete-looking
            // (sol round 2, finding 4).
            skipped += 1;
            continue;
        };
        if !file.is_file() {
            continue;
        }
        let lower = file.name().to_ascii_lowercase();
        if lower.ends_with(".xhtml") || lower.ends_with(".html") {
            names.push(file.name().to_owned());
        }
    }
    names.sort();

    let mut out = Vec::with_capacity(names.len());
    for name in names {
        let Ok(mut file) = archive.by_name(&name) else {
            skipped += 1;
            continue;
        };
        if file.size() > MAX_INSTANCE_BYTES {
            skipped += 1;
            continue;
        }
        let mut buf = Vec::with_capacity(file.size() as usize);
        if file.read_to_end(&mut buf).is_err() {
            skipped += 1;
            continue;
        }
        let prefix = &buf[..buf.len().min(64 * 1024)];
        if is_inline_xbrl(prefix) {
            out.push((name, buf));
        }
    }
    (out, skipped)
}

/// Role-family classification table (ADR 0100 decision 3): standard IFRS
/// role numbers (checked against the URI's trailing `-NNNNNN` segment) plus
/// the vendor-generated Polish role names several filers share verbatim. An
/// unrecognised role classifies as `"other"` — explicitly, never by guess.
///
/// The 320000/410000/420000 family is split income vs. comprehensive income
/// by the illustrative IFRS taxonomy's own convention (310000/320000 =
/// statement of profit or loss, 410000/420000 = statement of comprehensive
/// income); the ADR text groups them together prose-wise ("income /
/// comprehensive income") without spelling out the split, so this is this
/// slice's concrete reading of that decision, flagged for owner review.
const ROLE_RULES: &[(&str, &str)] = &[
    ("-210000", "balance"),
    ("-220000", "balance"),
    ("-320000", "income"),
    ("-410000", "comprehensive_income"),
    ("-420000", "comprehensive_income"),
    ("-520000", "cash_flow"),
    ("-610000", "equity_changes"),
    ("SprawozdanieZSytuacjiFinansowej", "balance"),
    ("WynikFinansowy", "income"),
    ("SprawozdanieZCalkowitychDochodow", "comprehensive_income"),
    ("SprawozdanieZPrzeplywowPienieznych", "cash_flow"),
    ("SprawozdanieZeZmianWKapitaleWlasnym", "equity_changes"),
];

/// Classifies a presentation-linkbase role URI into its statement family
/// (ADR 0100 decision 3). `"other"` for anything unrecognised.
fn classify_role(role_uri: &str) -> &'static str {
    ROLE_RULES
        .iter()
        .find(|(needle, _)| role_uri.contains(needle))
        .map(|(_, kind)| *kind)
        .unwrap_or("other")
}

/// The taxonomy-schema element-id convention (`{prefix}_{LocalName}`, e.g.
/// `ifrs-full_Assets`) a presentation linkbase locator's `xlink:href`
/// fragment follows. Matches purely on local name — the same carve-out ADR
/// 0100 decision 1 makes for the existing 22-concept mapping.
fn concept_local_from_href(href: &str) -> Option<String> {
    let fragment = href.rsplit('#').next()?;
    if fragment.is_empty() {
        return None;
    }
    match fragment.split_once('_') {
        Some((_, local)) if !local.is_empty() => Some(local.to_owned()),
        _ => Some(fragment.to_owned()),
    }
}

/// Parses one presentation-linkbase (`*_pre.xml`) document into
/// `(concept local name, role URI)` pairs — one per `link:loc` inside a
/// `link:presentationLink`. Malformed/unreadable bytes yield an empty result
/// (best-effort: a broken linkbase must never fail the whole extraction).
fn parse_presentation_linkbase(bytes: &[u8]) -> Vec<(String, String)> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let text = String::from_utf8_lossy(bytes);
    let mut reader = Reader::from_str(&text);
    reader.config_mut().trim_text(true);

    let mut current_role: Option<String> = None;
    let mut pairs = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local = local_of(e.name().as_ref()).to_vec();
                match local.as_slice() {
                    b"presentationLink" => current_role = attr(&e, b"role"),
                    b"loc" => {
                        if let Some(role) = &current_role {
                            if let Some(href) = attr(&e, b"href") {
                                if let Some(concept) = concept_local_from_href(&href) {
                                    pairs.push((concept, role.clone()));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) if local_of(e.name().as_ref()) == b"presentationLink" => {
                current_role = None;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    pairs
}

/// Reads every `*_pre.xml` presentation linkbase in the package and returns
/// the concept -> role classification map Layer 1 attaches to each fact
/// (ADR 0100 decision 3). Concept local name -> deduplicated `(role_uri,
/// role_kind)` pairs. Empty when the package holds no presentation linkbase
/// or is unreadable — a fact simply gets no role rows, never a guess.
pub fn extract_presentation_roles(bytes: &[u8]) -> HashMap<String, Vec<(String, String)>> {
    let reader = std::io::Cursor::new(bytes);
    let Ok(mut archive) = zip::ZipArchive::new(reader) else {
        return HashMap::new();
    };

    let mut result: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for i in 0..archive.len() {
        let Ok(mut file) = archive.by_index(i) else {
            continue;
        };
        if !file.is_file() {
            continue;
        }
        if !file.name().to_ascii_lowercase().ends_with("_pre.xml") {
            continue;
        }
        let mut buf = Vec::new();
        if file.read_to_end(&mut buf).is_err() {
            continue;
        }
        for (concept, role_uri) in parse_presentation_linkbase(&buf) {
            let kind = classify_role(&role_uri);
            let entries = result.entry(concept).or_default();
            if !entries.iter().any(|(r, _)| r == &role_uri) {
                entries.push((role_uri, kind.to_owned()));
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Label linkbase (ADR 0100 decision 10, epic #398): the owner-promotion name
// source
// ---------------------------------------------------------------------------

/// Resolves a locator/label pair's `xlink:label` (the labelArc endpoint) into
/// the `link:loc`/`link:label` element the leaf-tag handler needs — factored
/// out so both `Event::Start` and `Event::Empty` (a `loc`/`labelArc` element
/// is always empty; a real filing never self-closes `link:label` because it
/// always carries text) can share the same attribute reads.
fn record_label_linkbase_leaf(
    local: &[u8],
    e: &quick_xml::events::BytesStart<'_>,
    loc_to_concept: &mut HashMap<String, String>,
    arcs: &mut Vec<(String, String)>,
) {
    match local {
        b"loc" => {
            if let (Some(href), Some(label)) = (attr(e, b"href"), attr(e, b"label")) {
                if let Some(concept) = concept_local_from_href(&href) {
                    loc_to_concept.insert(label, concept);
                }
            }
        }
        b"labelArc" => {
            if let (Some(from), Some(to)) = (attr(e, b"from"), attr(e, b"to")) {
                arcs.push((from, to));
            }
        }
        _ => {}
    }
}

/// The standard XBRL label role — wins when a concept carries more than one
/// Polish label (e.g. a `terseLabel` alongside the standard one).
const STANDARD_LABEL_ROLE: &str = "http://www.xbrl.org/2003/role/label";

/// Parses one label-linkbase document (`*-lab-pl.xml` / `*_lab-pl.xml`) into a
/// concept-local-name -> Polish label-text map (ADR 0100 decision 10): a
/// `link:loc` resolves to a concept the same way the presentation linkbase's
/// locator does; `link:label` elements carry the text, kept only for
/// `xml:lang="pl"`; `link:labelArc` connects the two. Malformed/unreadable
/// bytes yield an empty map — best-effort, same doctrine as
/// [`parse_presentation_linkbase`].
fn parse_label_linkbase(bytes: &[u8]) -> HashMap<String, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let text = String::from_utf8_lossy(bytes);
    let mut reader = Reader::from_str(&text);
    reader.config_mut().trim_text(true);

    let mut loc_to_concept: HashMap<String, String> = HashMap::new();
    let mut arcs: Vec<(String, String)> = Vec::new();
    // label id -> (role, text) — the best Polish label text seen so far.
    let mut label_text: HashMap<String, (String, String)> = HashMap::new();
    let mut capturing: Option<(String, String)> = None; // (label id, role)
    let mut buf_text = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Empty(e)) => {
                let local = local_of(e.name().as_ref()).to_vec();
                record_label_linkbase_leaf(&local, &e, &mut loc_to_concept, &mut arcs);
            }
            Ok(Event::Start(e)) => {
                let local = local_of(e.name().as_ref()).to_vec();
                record_label_linkbase_leaf(&local, &e, &mut loc_to_concept, &mut arcs);
                if local == b"label" {
                    let lang = attr(&e, b"lang").unwrap_or_default();
                    let id = attr(&e, b"label").unwrap_or_default();
                    if lang.eq_ignore_ascii_case("pl") && !id.is_empty() {
                        capturing = Some((id, attr(&e, b"role").unwrap_or_default()));
                        buf_text.clear();
                    }
                }
            }
            Ok(Event::Text(t)) if capturing.is_some() => {
                if let Ok(unescaped) = t.decode() {
                    buf_text.push_str(&unescaped);
                }
            }
            Ok(Event::End(e)) if local_of(e.name().as_ref()) == b"label" => {
                if let Some((id, role)) = capturing.take() {
                    let value = buf_text.trim().to_owned();
                    if !value.is_empty() {
                        let is_standard = role == STANDARD_LABEL_ROLE;
                        let already_standard = label_text
                            .get(&id)
                            .is_some_and(|(existing_role, _)| existing_role == STANDARD_LABEL_ROLE);
                        if is_standard || !already_standard {
                            label_text.insert(id, (role, value));
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // concept -> (role, text) — resolved per-concept the same way per-label
    // resolution is above: the standard role wins over any other Polish label
    // an issuer's extension concept carries (e.g. a terseLabel), regardless of
    // arc order.
    let mut by_concept: HashMap<String, (String, String)> = HashMap::new();
    for (from, to) in arcs {
        let (Some(concept), Some((role, text))) = (loc_to_concept.get(&from), label_text.get(&to))
        else {
            continue;
        };
        let is_standard = role == STANDARD_LABEL_ROLE;
        let already_standard = by_concept
            .get(concept)
            .is_some_and(|(existing_role, _)| existing_role == STANDARD_LABEL_ROLE);
        if is_standard || !already_standard {
            by_concept.insert(concept.clone(), (role.clone(), text.clone()));
        }
    }
    by_concept.into_iter().map(|(c, (_, t))| (c, t)).collect()
}

/// Reads every `*-lab-pl.xml`/`*_lab-pl.xml` label linkbase in the package and
/// returns the concept -> Polish label map (ADR 0100 decision 10). Empty when
/// the package holds no Polish label linkbase or is unreadable — the caller's
/// promotion action falls back to the technical concept name, never a guess.
pub fn extract_label_linkbase(bytes: &[u8]) -> HashMap<String, String> {
    let reader = std::io::Cursor::new(bytes);
    let Ok(mut archive) = zip::ZipArchive::new(reader) else {
        return HashMap::new();
    };

    let mut result: HashMap<String, String> = HashMap::new();
    for i in 0..archive.len() {
        let Ok(mut file) = archive.by_index(i) else {
            continue;
        };
        if !file.is_file() {
            continue;
        }
        let lower = file.name().to_ascii_lowercase();
        if !(lower.ends_with("-lab-pl.xml") || lower.ends_with("_lab-pl.xml")) {
            continue;
        }
        let mut buf = Vec::new();
        if file.read_to_end(&mut buf).is_err() {
            continue;
        }
        for (concept, label) in parse_label_linkbase(&buf) {
            result.entry(concept).or_insert(label);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    /// Builds a minimal ESEF report package: a ZIP with the named entries, each
    /// holding the given bytes. Mirrors the real container layout (a `reports/`
    /// instance plus taxonomy siblings) without shipping a real filing.
    fn build_package(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            for (name, body) in entries {
                zip.start_file(*name, opts).expect("start entry");
                zip.write_all(body).expect("write entry");
            }
            zip.finish().expect("finish zip");
        }
        buf
    }

    const INSTANCE: &[u8] = b"<html><ix:nonFraction/></html>";

    #[test]
    fn recognizes_a_package_by_extension_and_magic() {
        let pkg = build_package(&[("CBF/reports/inst.xhtml", INSTANCE)]);
        assert!(is_report_package("something.xbri", &[]));
        assert!(is_report_package("something.zip", &[]));
        // Extension lies (octet-stream, no hint) → the ZIP magic still catches it.
        assert!(is_report_package("something.bin", &pkg));
        // A bare xhtml / pdf is not a package.
        assert!(!is_report_package("report.xhtml", b"<html></html>"));
        assert!(!is_report_package("report.pdf", b"%PDF-1.4"));
    }

    #[test]
    fn extracts_the_reports_instance_over_siblings() {
        // The taxonomy xsd is larger junk, and there is a stray xhtml outside
        // reports/ — the reports/ instance must still win.
        let pkg = build_package(&[
            ("CBF-2025/META-INF/reportPackage.json", b"{}"),
            ("CBF-2025/www/CBF.xsd", &vec![b'x'; 5000]),
            ("CBF-2025/decoy.xhtml", b"<html>decoy</html>"),
            ("CBF-2025/reports/CBF-2025-12-31-1-pl.xhtml", INSTANCE),
        ]);
        let instance = extract_instance(&pkg).expect("instance found");
        assert_eq!(instance, INSTANCE);
    }

    #[test]
    fn falls_back_to_largest_xhtml_without_reports_folder() {
        let pkg = build_package(&[
            ("bundle/small.xhtml", b"<html>s</html>"),
            ("bundle/big.xhtml", INSTANCE),
        ]);
        let instance = extract_instance(&pkg).expect("instance found");
        assert_eq!(instance, INSTANCE);
    }

    #[test]
    fn none_for_a_package_without_xhtml() {
        let pkg = build_package(&[("bundle/data.xml", b"<x/>"), ("bundle/notes.txt", b"hi")]);
        assert_eq!(extract_instance(&pkg), None);
    }

    #[test]
    fn none_for_non_zip_bytes() {
        assert_eq!(extract_instance(b"%PDF-1.4 not a zip"), None);
    }

    // -----------------------------------------------------------------
    // Layer 1 (ADR 0100 dec. 1/3/9, epic #398): every instance, every role.
    // -----------------------------------------------------------------

    #[test]
    fn extract_all_instances_returns_every_real_instance_and_skips_a_decoy() {
        // Real shape: one issuer files standalone AND consolidated statements
        // as separate instances in the same package, alongside an unrelated
        // decoy xhtml that is NOT inline XBRL (must be excluded).
        let pkg = build_package(&[
            ("CBF-2025/META-INF/reportPackage.json", b"{}"),
            (
                "CBF-2025/decoy.xhtml",
                b"<html>decoy, not inline XBRL</html>",
            ),
            ("CBF-2025/reports/standalone.xhtml", INSTANCE),
            ("CBF-2025/reports/consolidated.xhtml", INSTANCE),
        ]);
        let instances = extract_all_instances(&pkg);
        assert_eq!(
            instances.len(),
            2,
            "the decoy (non-inline-XBRL) xhtml must be excluded"
        );
        let paths: Vec<&str> = instances.iter().map(|(p, _)| p.as_str()).collect();
        assert!(paths.contains(&"CBF-2025/reports/standalone.xhtml"));
        assert!(paths.contains(&"CBF-2025/reports/consolidated.xhtml"));
    }

    #[test]
    fn extract_all_instances_is_empty_for_a_package_without_any_real_instance() {
        let pkg = build_package(&[("bundle/decoy.xhtml", b"<html>plain, not tagged</html>")]);
        assert!(extract_all_instances(&pkg).is_empty());
    }

    /// A synthetic presentation linkbase covering both role families the real
    /// corpus uses (ADR 0100 decision 3): standard IFRS role numbers, a vendor
    /// Polish role name, and one unrecognised role (must classify `"other"`).
    const PRE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<link:linkbase xmlns:link="http://www.xbrl.org/2003/linkbase" xmlns:xlink="http://www.w3.org/1999/xlink">
  <link:presentationLink xlink:type="extended" xlink:role="http://www.example.com/role/ias_1_role-210000">
    <link:loc xlink:type="locator" xlink:href="ifrs-full-2023.xsd#ifrs-full_Assets" xlink:label="loc_assets"/>
  </link:presentationLink>
  <link:presentationLink xlink:type="extended" xlink:role="http://www.example.com/role/ias_1_role-320000">
    <link:loc xlink:type="locator" xlink:href="ifrs-full-2023.xsd#ifrs-full_ProfitLoss" xlink:label="loc_pl"/>
  </link:presentationLink>
  <link:presentationLink xlink:type="extended" xlink:role="http://www.example.com/role/ias_7_role-520000">
    <link:loc xlink:type="locator" xlink:href="ifrs-full-2023.xsd#ifrs-full_CashFlowsFromUsedInOperatingActivities" xlink:label="loc_cfo"/>
  </link:presentationLink>
  <link:presentationLink xlink:type="extended" xlink:role="http://xtb.pl/role/WynikFinansowy">
    <link:loc xlink:type="locator" xlink:href="xtb.xsd#xtb_NetResult" xlink:label="loc_nr"/>
  </link:presentationLink>
  <link:presentationLink xlink:type="extended" xlink:role="http://www.example.com/role/some_note_disclosure">
    <link:loc xlink:type="locator" xlink:href="ifrs-full-2023.xsd#ifrs-full_SomeNoteConcept" xlink:label="loc_note"/>
  </link:presentationLink>
</link:linkbase>"#;

    #[test]
    fn extract_presentation_roles_classifies_standard_ifrs_role_numbers() {
        let pkg = build_package(&[
            ("CBF-2025/reports/instance.xhtml", INSTANCE),
            ("CBF-2025/www/xtb-2025-12-31_pre.xml", PRE_XML.as_bytes()),
        ]);
        let roles = extract_presentation_roles(&pkg);
        assert_eq!(
            roles.get("Assets").expect("Assets classified")[0].1,
            "balance"
        );
        assert_eq!(
            roles.get("ProfitLoss").expect("ProfitLoss classified")[0].1,
            "income"
        );
        assert_eq!(
            roles
                .get("CashFlowsFromUsedInOperatingActivities")
                .expect("cash-flow concept classified")[0]
                .1,
            "cash_flow"
        );
    }

    #[test]
    fn extract_presentation_roles_classifies_vendor_polish_role_names() {
        let pkg = build_package(&[
            ("CBF-2025/reports/instance.xhtml", INSTANCE),
            ("CBF-2025/www/xtb-2025-12-31_pre.xml", PRE_XML.as_bytes()),
        ]);
        let roles = extract_presentation_roles(&pkg);
        assert_eq!(
            roles.get("NetResult").expect("NetResult classified")[0].1,
            "income"
        );
    }

    #[test]
    fn extract_presentation_roles_classifies_unrecognised_roles_as_other() {
        let pkg = build_package(&[
            ("CBF-2025/reports/instance.xhtml", INSTANCE),
            ("CBF-2025/www/xtb-2025-12-31_pre.xml", PRE_XML.as_bytes()),
        ]);
        let roles = extract_presentation_roles(&pkg);
        assert_eq!(
            roles
                .get("SomeNoteConcept")
                .expect("note concept classified")[0]
                .1,
            "other",
            "an unrecognised role must classify explicitly as other, never by guess"
        );
    }

    #[test]
    fn extract_presentation_roles_is_empty_without_a_pre_xml() {
        let pkg = build_package(&[("CBF-2025/reports/instance.xhtml", INSTANCE)]);
        assert!(extract_presentation_roles(&pkg).is_empty());
    }

    const LAB_PL_XML: &str = r#"<?xml version="1.0"?>
<link:linkbase
    xmlns:link="http://www.xbrl.org/2003/linkbase"
    xmlns:xlink="http://www.w3.org/1999/xlink"
    xmlns:xml="http://www.w3.org/XML/1998/namespace">
  <link:labelLink>
    <link:loc xlink:type="locator" xlink:href="issuer-2025.xsd#issuer_PozostaleUslugiObce" xlink:label="loc_pozostale"/>
    <link:label xlink:type="resource" xlink:label="lab_pozostale_pl"
        xlink:role="http://www.xbrl.org/2003/role/label" xml:lang="pl">Pozostałe usługi obce</link:label>
    <link:label xlink:type="resource" xlink:label="lab_pozostale_en"
        xlink:role="http://www.xbrl.org/2003/role/label" xml:lang="en">Other external services</link:label>
    <link:labelArc xlink:type="arc" xlink:from="loc_pozostale" xlink:to="lab_pozostale_pl"
        xlink:arcrole="http://www.xbrl.org/2003/arcrole/concept-label"/>
    <link:labelArc xlink:type="arc" xlink:from="loc_pozostale" xlink:to="lab_pozostale_en"
        xlink:arcrole="http://www.xbrl.org/2003/arcrole/concept-label"/>
  </link:labelLink>
</link:linkbase>"#;

    #[test]
    fn extract_label_linkbase_reads_the_issuer_polish_label() {
        let pkg = build_package(&[
            ("CBF-2025/reports/instance.xhtml", INSTANCE),
            ("CBF-2025/www/issuer-2025_lab-pl.xml", LAB_PL_XML.as_bytes()),
        ]);
        let labels = extract_label_linkbase(&pkg);
        assert_eq!(
            labels.get("PozostaleUslugiObce").map(String::as_str),
            Some("Pozostałe usługi obce"),
            "must resolve the Polish label, never the English sibling"
        );
    }

    #[test]
    fn extract_label_linkbase_prefers_the_standard_label_role_over_a_terse_one() {
        // The terse (non-standard) label is captured FIRST, proving the
        // standard role wins regardless of document order.
        let combined = r#"<?xml version="1.0"?>
<link:linkbase
    xmlns:link="http://www.xbrl.org/2003/linkbase"
    xmlns:xlink="http://www.w3.org/1999/xlink"
    xmlns:xml="http://www.w3.org/XML/1998/namespace">
  <link:labelLink>
    <link:loc xlink:type="locator" xlink:href="issuer-2025.xsd#issuer_PozostaleUslugiObce" xlink:label="loc_pozostale"/>
    <link:label xlink:type="resource" xlink:label="lab_terse"
        xlink:role="http://www.xbrl.org/2003/role/terseLabel" xml:lang="pl">Usługi obce</link:label>
    <link:label xlink:type="resource" xlink:label="lab_standard"
        xlink:role="http://www.xbrl.org/2003/role/label" xml:lang="pl">Pozostałe usługi obce</link:label>
    <link:labelArc xlink:type="arc" xlink:from="loc_pozostale" xlink:to="lab_terse"
        xlink:arcrole="http://www.xbrl.org/2003/arcrole/concept-label"/>
    <link:labelArc xlink:type="arc" xlink:from="loc_pozostale" xlink:to="lab_standard"
        xlink:arcrole="http://www.xbrl.org/2003/arcrole/concept-label"/>
  </link:labelLink>
</link:linkbase>"#
            .to_string();
        let pkg = build_package(&[
            ("CBF-2025/reports/instance.xhtml", INSTANCE),
            ("CBF-2025/www/issuer-2025_lab-pl.xml", combined.as_bytes()),
        ]);
        let labels = extract_label_linkbase(&pkg);
        assert_eq!(
            labels.get("PozostaleUslugiObce").map(String::as_str),
            Some("Pozostałe usługi obce")
        );
    }

    #[test]
    fn extract_label_linkbase_is_empty_without_a_lab_pl_file() {
        let pkg = build_package(&[("CBF-2025/reports/instance.xhtml", INSTANCE)]);
        assert!(extract_label_linkbase(&pkg).is_empty());
    }
}
