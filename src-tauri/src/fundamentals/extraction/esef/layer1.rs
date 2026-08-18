//! Layer 1 raw tagged-fact capture (ADR 0100 decisions 1, 3, 9; epic #398).
//!
//! Where [`super::parse_esef`] (Layer 2) keeps only the ~22 concepts the
//! catalog maps, filters out every dimensional/failed occurrence, and strips
//! the QName down to its local name, this module captures **every** supported
//! occurrence unconditionally: namespace-resolved concept identity, dimensional
//! facts, and facts whose value/context/unit failed to normalize (a typed
//! `parse_status`, never a silent drop — decision 9). It runs as an
//! independent second pass over the same bytes so Layer 2's behaviour can
//! never drift by a single byte from this addition.
//!
//! Scope decisions this pass makes explicit (decision 9: "nothing is dropped
//! silently" also means nothing is silently guessed):
//!
//! - `ix:nonFraction` is captured in full: value, scale, sign, decimals,
//!   dimensions, presentation roles.
//! - `ix:fraction` is COUNTED (a row is written, `parse_status =
//!   "unparsed_value"`) but its numerator/denominator are not normalized into
//!   `value_numeric` this slice — no real GPW filing sample exercises it, and
//!   fabricating a value would be a guess, not a capture.
//! - A nil fact (`xsi:nil="true"`) needs no special case: its element carries
//!   no text, so the existing "could not parse an empty value" path already
//!   yields `parse_status = "unparsed_value"` with a nil-specific message.
//! - `ix:continuation`/`continuedAt` chains are NOT followed: a fact's value
//!   is read from its own element text only. A numeric fact that CARRIES
//!   `continuedAt` is therefore stored with `parse_status =
//!   "unsupported_continuation"` and NO `value_numeric` — its local fragment
//!   can parse cleanly while being only part of the disclosed number, so
//!   parsing it would store a silently wrong value under an "ok" status (sol
//!   review finding 4). The occurrence is counted, never dropped.
//! - `ix:nonNumeric` (text-tagged facts — entity names, date blocks, free-text
//!   disclosures) is out of scope entirely: ADR 0100's mandate is "every
//!   disclosed **number**". These are neither encountered nor counted.
//!
//! Presentation-role attachment reads the package's `*_pre.xml` linkbase(s)
//! (via [`super::super::esef_package::extract_presentation_roles`]) and
//! matches purely on concept **local name** (ADR 0100 dec. 1's own carve-out:
//! "the existing 22-concept mapping keeps matching on the local name for
//! now") — a taxonomy-schema-correct match would additionally resolve the
//! linkbase's own namespace declarations, which is out of scope here.

use std::collections::{BTreeMap, HashMap};

use quick_xml::events::Event;
use quick_xml::Reader;

use super::{apply_scale, attr, local_of, local_str, parse_ixbrl_number};
use crate::storage::{NewTaggedFact, NewTaggedFactRole};

/// The result of running Layer 1 capture over one instance document.
#[derive(Debug, Clone, Default)]
pub struct Layer1Pass {
    pub facts: Vec<NewTaggedFact>,
    /// Every `ix:nonFraction`/`ix:fraction` occurrence this pass opened.
    pub encountered_count: i64,
    /// Rows actually written — decision 9's ship gate is `encountered ==
    /// stored`; this pass never drops an occurrence it opened.
    pub stored_count: i64,
    pub dimensional_count: i64,
    /// A reader error aborted the walk mid-document: everything after the
    /// error was never even ENCOUNTERED, so `encountered == stored` holds
    /// vacuously while the capture is incomplete (sol review finding 4).
    /// The caller must mark the extraction truncated, never complete.
    pub reader_error: Option<String>,
}

/// A resolved (or unresolved) context, keyed by its `id`.
#[derive(Default)]
struct ContextInfo {
    instant: Option<String>,
    start: Option<String>,
    end: Option<String>,
    dimensional: bool,
    /// dimension QName (raw, as written) -> member/typed value text.
    dims: BTreeMap<String, String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FactKind {
    NonFraction,
    Fraction,
}

struct PendingFact {
    kind: FactKind,
    concept_namespace_uri: String,
    concept_local_name: String,
    context_ref: String,
    unit_ref: Option<String>,
    scale: Option<i64>,
    sign: Option<String>,
    decimals: Option<String>,
    format: Option<String>,
    xml_id: Option<String>,
    nil: bool,
    /// `continuedAt` attribute — the value text continues in an
    /// `ix:continuation` element this pass does not follow. A fact carrying
    /// it must NEVER parse its local fragment as the whole number (sol
    /// review finding 4: local text `12` + continuation `34` is 1234, and
    /// `12` with `parse_status = "ok"` would be a silently wrong value).
    continued_at: Option<String>,
    text: String,
    open_depth: usize,
}

#[derive(Clone, Copy)]
enum PeriodField {
    Instant,
    Start,
    End,
}

/// Splits a possibly-prefixed QName into `(prefix, local)`. Unlike
/// [`local_of`] (which only needs the local part), Layer 1 needs the prefix
/// too, to resolve it against the in-scope namespace declarations.
fn split_qname(name: &[u8]) -> (Option<String>, String) {
    match name.iter().position(|&b| b == b':') {
        Some(i) => (
            Some(String::from_utf8_lossy(&name[..i]).into_owned()),
            String::from_utf8_lossy(&name[i + 1..]).into_owned(),
        ),
        None => (None, String::from_utf8_lossy(name).into_owned()),
    }
}

/// Resolves a QName's prefix against the CURRENT namespace scope (the top of
/// the scope stack, which already carries every ancestor's declarations
/// merged in). An unresolvable prefix (or an unprefixed name with no default
/// namespace declared) resolves to an empty URI — never a panic, never a
/// guess at intent.
fn resolve_concept(name: &[u8], scope: &HashMap<String, String>) -> (String, String) {
    let (prefix, local) = split_qname(name);
    let uri = scope.get(prefix.as_deref().unwrap_or("")).cloned();
    (uri.unwrap_or_default(), local)
}

/// Placeholder period fields written when a fact's `contextRef` cannot be
/// resolved at all (unknown id, or a context with neither an instant nor a
/// start+end pair). The schema's `period_type`/`period_end` are `NOT NULL` —
/// decision 9 forbids dropping the row, so these are typed sentinels, never a
/// guess at the real period. `parse_status = "unresolved_context"` is the
/// truth signal; callers must not trust `period_end`/`period_type` without
/// checking it first.
const UNRESOLVED_PERIOD_TYPE: &str = "instant";
const UNRESOLVED_PERIOD_END: &str = "";

/// Parses one iXBRL instance document into its Layer 1 tagged-fact rows.
/// `package_entry_path` is the path of this instance inside its package (or
/// `""` for a bare, non-package document — the storage convention `0142`
/// documents). `roles_by_concept` is the package's presentation-linkbase
/// classification (concept local name -> `(role_uri, role_kind)` pairs),
/// already resolved by [`super::super::esef_package::extract_presentation_roles`];
/// pass an empty map when no package/linkbase is available.
pub fn extract_tagged_facts(
    bytes: &[u8],
    package_entry_path: &str,
    roles_by_concept: &HashMap<String, Vec<(String, String)>>,
) -> Layer1Pass {
    let text = String::from_utf8_lossy(bytes);
    let mut reader = Reader::from_str(&text);
    reader.config_mut().trim_text(true);

    let mut ns_stack: Vec<HashMap<String, String>> = vec![HashMap::new()];
    let mut contexts: HashMap<String, ContextInfo> = HashMap::new();
    let mut units: HashMap<String, String> = HashMap::new();

    let mut depth: usize = 0;
    let mut reader_error: Option<String> = None;
    let mut ctx_id: Option<String> = None;
    let mut ctx = ContextInfo::default();
    let mut current_dim_key: Option<String> = None;
    let mut current_dim_text = String::new();
    let mut period_field: Option<PeriodField> = None;
    let mut unit_id: Option<String> = None;
    let mut in_measure = false;
    let mut in_denominator = false;
    let mut pending: Option<PendingFact> = None;
    let mut raw: Vec<PendingFact> = Vec::new();
    let mut encountered_count: i64 = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                depth += 1;
                // Extend the namespace scope BEFORE reading this element's own
                // attributes (e.g. its `name`) — an element may declare the very
                // prefix it uses on itself.
                let mut frame = ns_stack.last().cloned().unwrap_or_default();
                for a in e.attributes().flatten() {
                    let key = a.key.as_ref();
                    let value = a
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map(|v| v.into_owned())
                        .unwrap_or_default();
                    if key == b"xmlns" {
                        frame.insert(String::new(), value);
                    } else if let Some(rest) = key.strip_prefix(b"xmlns:") {
                        frame.insert(String::from_utf8_lossy(rest).into_owned(), value);
                    }
                }
                ns_stack.push(frame);

                let local = local_of(e.name().as_ref()).to_vec();
                match local.as_slice() {
                    b"context" => {
                        ctx_id = attr(&e, b"id");
                        ctx = ContextInfo::default();
                    }
                    b"explicitMember" | b"typedMember" if ctx_id.is_some() => {
                        ctx.dimensional = true;
                        current_dim_key = attr(&e, b"dimension");
                        current_dim_text.clear();
                    }
                    b"instant" => period_field = Some(PeriodField::Instant),
                    b"startDate" => period_field = Some(PeriodField::Start),
                    b"endDate" => period_field = Some(PeriodField::End),
                    b"unit" => unit_id = attr(&e, b"id"),
                    b"unitDenominator" => in_denominator = true,
                    b"measure" => in_measure = true,
                    b"nonFraction" | b"fraction" if pending.is_none() => {
                        let kind = if local.as_slice() == b"nonFraction" {
                            FactKind::NonFraction
                        } else {
                            FactKind::Fraction
                        };
                        let name = attr(&e, b"name").unwrap_or_default();
                        let (concept_namespace_uri, concept_local_name) =
                            resolve_concept(name.as_bytes(), ns_stack.last().unwrap());
                        encountered_count += 1;
                        pending = Some(PendingFact {
                            kind,
                            concept_namespace_uri,
                            concept_local_name,
                            context_ref: attr(&e, b"contextRef").unwrap_or_default(),
                            unit_ref: attr(&e, b"unitRef"),
                            scale: attr(&e, b"scale").and_then(|s| s.parse().ok()),
                            sign: attr(&e, b"sign"),
                            decimals: attr(&e, b"decimals"),
                            format: attr(&e, b"format"),
                            xml_id: attr(&e, b"id"),
                            nil: attr(&e, b"nil").as_deref() == Some("true"),
                            continued_at: attr(&e, b"continuedAt"),
                            text: String::new(),
                            open_depth: depth,
                        });
                    }
                    _ => {}
                }
            }
            // Self-closing element (`<ix:nonFraction .../>`) — no matching End
            // event will ever fire, so it must be resolved+finalized inline. This
            // matters in practice: a NIL fact (`xsi:nil="true"`) is conventionally
            // written self-closed, and decision 9 forbids it disappearing simply
            // because it has no nested content.
            Ok(Event::Empty(e)) => {
                let mut frame = ns_stack.last().cloned().unwrap_or_default();
                for a in e.attributes().flatten() {
                    let key = a.key.as_ref();
                    let value = a
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map(|v| v.into_owned())
                        .unwrap_or_default();
                    if key == b"xmlns" {
                        frame.insert(String::new(), value);
                    } else if let Some(rest) = key.strip_prefix(b"xmlns:") {
                        frame.insert(String::from_utf8_lossy(rest).into_owned(), value);
                    }
                }
                let local = local_of(e.name().as_ref()).to_vec();
                match local.as_slice() {
                    b"context" => {
                        if let Some(id) = attr(&e, b"id") {
                            contexts.insert(id, ContextInfo::default());
                        }
                    }
                    b"explicitMember" | b"typedMember" if ctx_id.is_some() => {
                        if let Some(dim) = attr(&e, b"dimension") {
                            ctx.dims.insert(dim, String::new());
                        }
                        ctx.dimensional = true;
                    }
                    b"nonFraction" | b"fraction" if pending.is_none() => {
                        let kind = if local.as_slice() == b"nonFraction" {
                            FactKind::NonFraction
                        } else {
                            FactKind::Fraction
                        };
                        let name = attr(&e, b"name").unwrap_or_default();
                        let (concept_namespace_uri, concept_local_name) =
                            resolve_concept(name.as_bytes(), &frame);
                        encountered_count += 1;
                        raw.push(PendingFact {
                            kind,
                            concept_namespace_uri,
                            concept_local_name,
                            context_ref: attr(&e, b"contextRef").unwrap_or_default(),
                            unit_ref: attr(&e, b"unitRef"),
                            scale: attr(&e, b"scale").and_then(|s| s.parse().ok()),
                            sign: attr(&e, b"sign"),
                            decimals: attr(&e, b"decimals"),
                            format: attr(&e, b"format"),
                            xml_id: attr(&e, b"id"),
                            nil: attr(&e, b"nil").as_deref() == Some("true"),
                            continued_at: attr(&e, b"continuedAt"),
                            text: String::new(),
                            open_depth: depth,
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                let value = match crate::source_adapters::parsing::decode_xml_text_value(t) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if let Some(field) = period_field {
                    match field {
                        PeriodField::Instant => ctx.instant = Some(value.trim().to_string()),
                        PeriodField::Start => ctx.start = Some(value.trim().to_string()),
                        PeriodField::End => ctx.end = Some(value.trim().to_string()),
                    }
                } else if in_measure && !in_denominator {
                    // Mirrors Layer 2: only the numerator/simple measure defines a
                    // unit's currency — the divide unit's denominator (`xbrli:shares`
                    // in an EPS unit) is not one.
                    if let Some(id) = &unit_id {
                        units.insert(id.clone(), local_str(value.trim().as_bytes()));
                    }
                } else if current_dim_key.is_some() {
                    current_dim_text.push_str(&value);
                } else if let Some(p) = pending.as_mut() {
                    p.text.push_str(&value);
                }
            }
            Ok(Event::End(e)) => {
                let local = local_of(e.name().as_ref()).to_vec();
                if let Some(p) = pending.as_ref() {
                    if matches!(local.as_slice(), b"nonFraction" | b"fraction")
                        && depth == p.open_depth
                    {
                        raw.push(pending.take().unwrap());
                    }
                }
                match local.as_slice() {
                    b"explicitMember" | b"typedMember" => {
                        if let Some(key) = current_dim_key.take() {
                            ctx.dims.insert(key, current_dim_text.trim().to_owned());
                        }
                        current_dim_text.clear();
                    }
                    b"context" => {
                        if let Some(id) = ctx_id.take() {
                            contexts.insert(id, std::mem::take(&mut ctx));
                        }
                    }
                    b"instant" | b"startDate" | b"endDate" => period_field = None,
                    b"unit" => unit_id = None,
                    b"unitDenominator" => in_denominator = false,
                    b"measure" => in_measure = false,
                    _ => {}
                }
                depth = depth.saturating_sub(1);
                ns_stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                // Do NOT pretend the document ended: record the truncation so
                // the extraction is marked incomplete (sol review finding 4 —
                // a silent break here made every later fact "unencountered"
                // and the completeness invariant vacuously green).
                reader_error = Some(format!("XML reader error mid-document: {error}"));
                break;
            }
            _ => {}
        }
    }

    // Resolve each occurrence against contexts/units, never dropping one.
    let mut fallback_ordinal: HashMap<(String, String), usize> = HashMap::new();
    let mut facts: Vec<NewTaggedFact> = Vec::new();
    for p in raw {
        let (fact_identity, identity_kind) = match &p.xml_id {
            Some(id) => (format!("xml_id:{id}"), "xml_id"),
            None => {
                let key = (p.concept_local_name.clone(), p.context_ref.clone());
                let ordinal = fallback_ordinal.entry(key).or_insert(0);
                let identity = format!(
                    "occurrence:{}:{}:{}",
                    p.concept_local_name, p.context_ref, ordinal
                );
                *ordinal += 1;
                (identity, "occurrence")
            }
        };

        let resolved_context = contexts.get(&p.context_ref).and_then(|c| {
            if let Some(instant) = &c.instant {
                Some(("instant".to_owned(), None, instant.clone(), c))
            } else if let (Some(start), Some(end)) = (&c.start, &c.end) {
                Some(("duration".to_owned(), Some(start.clone()), end.clone(), c))
            } else {
                None
            }
        });

        let (
            period_type,
            period_start,
            period_end,
            is_dimensional,
            dimensions_json,
            mut parse_status,
            mut parse_error,
        ) = match resolved_context {
            Some((period_type, period_start, period_end, c)) => {
                let dimensions_json = if c.dims.is_empty() {
                    None
                } else {
                    serde_json::to_string(&c.dims).ok()
                };
                (
                    period_type,
                    period_start,
                    period_end,
                    c.dimensional,
                    dimensions_json,
                    "ok".to_owned(),
                    None,
                )
            }
            None => (
                UNRESOLVED_PERIOD_TYPE.to_owned(),
                None,
                UNRESOLVED_PERIOD_END.to_owned(),
                false,
                None,
                "unresolved_context".to_owned(),
                Some(format!(
                    "contextRef '{}' does not resolve to a context with a period",
                    p.context_ref
                )),
            ),
        };

        let unit_measure = match &p.unit_ref {
            Some(id) => match units.get(id) {
                Some(measure) => Some(measure.clone()),
                None => {
                    if parse_status == "ok" {
                        parse_status = "unsupported_unit".to_owned();
                        parse_error = Some(format!(
                            "unitRef '{id}' does not resolve to a declared unit"
                        ));
                    }
                    None
                }
            },
            None => None,
        };

        let mut value_numeric: Option<String> = None;
        if parse_status == "ok" {
            match p.kind {
                FactKind::Fraction => {
                    parse_status = "unparsed_value".to_owned();
                    parse_error = Some(
                        "ix:fraction numerator/denominator normalization is not implemented \
                         this slice — the occurrence is counted, not dropped"
                            .to_owned(),
                    );
                }
                FactKind::NonFraction if p.continued_at.is_some() => {
                    parse_status = "unsupported_continuation".to_owned();
                    parse_error = Some(
                        "value text continues in an ix:continuation chain this pass does not                          follow — the local fragment alone would be a wrong number, so none is                          stored (sol review finding 4)"
                            .to_owned(),
                    );
                }
                FactKind::NonFraction => {
                    let parsed = parse_ixbrl_number(&p.text, p.format.as_deref())
                        .and_then(|v| apply_scale(v, p.scale.unwrap_or(0) as i32))
                        .map(|mut v| {
                            if p.sign.as_deref() == Some("-") {
                                v.set_sign_negative(!v.is_sign_negative());
                            }
                            v
                        });
                    match parsed {
                        Some(v) => value_numeric = Some(v.to_string()),
                        None => {
                            parse_status = "unparsed_value".to_owned();
                            parse_error = Some(if p.nil {
                                "nil fact (xsi:nil=\"true\")".to_owned()
                            } else {
                                format!("could not parse '{}' as a decimal", p.text.trim())
                            });
                        }
                    }
                }
            }
        }

        let roles = roles_by_concept
            .get(&p.concept_local_name)
            .map(|rs| {
                rs.iter()
                    .map(|(role_uri, role_kind)| NewTaggedFactRole {
                        role_uri: role_uri.clone(),
                        role_kind: role_kind.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        facts.push(NewTaggedFact {
            package_entry_path: package_entry_path.to_owned(),
            fact_identity,
            identity_kind: identity_kind.to_owned(),
            concept_namespace_uri: p.concept_namespace_uri,
            concept_local_name: p.concept_local_name,
            context_ref: p.context_ref,
            period_type,
            period_start,
            period_end,
            unit_ref: p.unit_ref,
            unit_measure,
            value_raw: p.text,
            value_numeric,
            scale: p.scale,
            sign: p.sign,
            decimals: p.decimals,
            is_dimensional,
            dimensions_json,
            parse_status,
            parse_error,
            roles,
        });
    }

    let dimensional_count = facts.iter().filter(|f| f.is_dimensional).count() as i64;
    let stored_count = facts.len() as i64;
    Layer1Pass {
        facts,
        encountered_count,
        stored_count,
        dimensional_count,
        reader_error,
    }
}

#[cfg(test)]
mod tests;
