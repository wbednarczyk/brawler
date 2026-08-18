//! Tests for Layer 1 raw tagged-fact capture (ADR 0100 decisions 1, 3, 9;
//! epic #398). Companion to `esef/tests.rs` (Layer 2, unchanged) — several
//! tests below run the SAME xml through both `parse_esef` and
//! `extract_tagged_facts` to pin that Layer 2's behaviour never drifts.

use std::collections::HashMap;

use super::*;

fn no_roles() -> HashMap<String, Vec<(String, String)>> {
    HashMap::new()
}

// ---------------------------------------------------------------------------
// Namespace-aware concept identity
// ---------------------------------------------------------------------------

#[test]
fn namespaced_concept_resolves_to_the_expanded_namespace_uri() {
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" id="f1">1000</ix:nonFraction>
    </html>"#;
    let pass = extract_tagged_facts(xml.as_bytes(), "", &no_roles());
    assert_eq!(pass.facts.len(), 1);
    assert_eq!(
        pass.facts[0].concept_namespace_uri, "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
        "must resolve the QName prefix to its EXPANDED namespace, not keep the prefix"
    );
    assert_eq!(pass.facts[0].concept_local_name, "Assets");
}

#[test]
fn two_prefixes_bound_to_the_same_uri_collapse_to_one_identity() {
    // Two document-local aliases for the SAME real namespace: a generic
    // filer alias `a` and a differently-named one `b` (mirrors two real
    // filings using different prefix conventions for identical concepts).
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:a="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full"
      xmlns:b="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <ix:nonFraction name="a:Assets" contextRef="c" id="f1">1000</ix:nonFraction>
      <ix:nonFraction name="b:Assets" contextRef="c" id="f2">1000</ix:nonFraction>
    </html>"#;
    let pass = extract_tagged_facts(xml.as_bytes(), "", &no_roles());
    assert_eq!(pass.facts.len(), 2);
    let uris: std::collections::HashSet<&str> = pass
        .facts
        .iter()
        .map(|f| f.concept_namespace_uri.as_str())
        .collect();
    assert_eq!(
        uris.len(),
        1,
        "two prefixes bound to the same URI must collapse to one identity"
    );
    assert!(uris.contains("http://xbrl.ifrs.org/taxonomy/2023/ifrs-full"));
}

#[test]
fn same_prefix_bound_to_different_uris_stays_distinct() {
    // The `x` prefix is rebound to a different URI inside a nested element —
    // a document-local alias is scoped, not global. One filer's `XTB:` is
    // not another's, and neither is the SAME prefix reused at different
    // scopes within one document.
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:x="http://example.com/uri-one">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <ix:nonFraction name="x:Foo" contextRef="c" id="f1">1</ix:nonFraction>
      <div xmlns:x="http://example.com/uri-two">
        <ix:nonFraction name="x:Foo" contextRef="c" id="f2">2</ix:nonFraction>
      </div>
    </html>"#;
    let pass = extract_tagged_facts(xml.as_bytes(), "", &no_roles());
    assert_eq!(pass.facts.len(), 2);
    let f1 = pass
        .facts
        .iter()
        .find(|f| f.fact_identity == "xml_id:f1")
        .expect("f1");
    let f2 = pass
        .facts
        .iter()
        .find(|f| f.fact_identity == "xml_id:f2")
        .expect("f2");
    assert_eq!(f1.concept_namespace_uri, "http://example.com/uri-one");
    assert_eq!(f2.concept_namespace_uri, "http://example.com/uri-two");
    assert_ne!(f1.concept_namespace_uri, f2.concept_namespace_uri);
}

// ---------------------------------------------------------------------------
// Every supported occurrence, including dimensional and failed-normalization
// ---------------------------------------------------------------------------

#[test]
fn dimensional_fact_is_stored_and_layer2_still_skips_it() {
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:xbrldi="http://xbrl.org/2006/xbrldi"
      xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full">
      <xbrli:context id="total"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:context id="nci">
        <xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period>
        <xbrli:scenario>
          <xbrldi:explicitMember dimension="ifrs-full:ComponentsOfEquityAxis">ifrs-full:NoncontrollingInterestsMember</xbrldi:explicitMember>
        </xbrli:scenario>
      </xbrli:context>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="total" id="f1" scale="0">600</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="nci" id="f2" scale="0">50</ix:nonFraction>
    </html>"#;

    // Layer 1: BOTH occurrences stored; the dimensional one is flagged, not
    // dropped, and carries its dimension members.
    let pass = extract_tagged_facts(xml.as_bytes(), "", &no_roles());
    assert_eq!(pass.facts.len(), 2);
    let total = pass
        .facts
        .iter()
        .find(|f| f.fact_identity == "xml_id:f1")
        .expect("total");
    let nci = pass
        .facts
        .iter()
        .find(|f| f.fact_identity == "xml_id:f2")
        .expect("nci");
    assert!(!total.is_dimensional);
    assert!(nci.is_dimensional);
    assert!(nci.dimensions_json.is_some());
    assert_eq!(pass.dimensional_count, 1);
    assert_eq!(pass.encountered_count, 2);
    assert_eq!(pass.stored_count, 2);

    // Layer 2 (unchanged): the dimensional member never reaches the fact path
    // — only the default-member total survives.
    let layer2 = crate::fundamentals::extraction::esef::parse_esef(xml.as_bytes())
        .expect("layer 2 still parses this sample");
    assert_eq!(layer2.len(), 1);
    assert_eq!(layer2[0].metric_key, "total_equity");
    assert_eq!(layer2[0].value, rust_decimal::Decimal::from(600));
}

#[test]
fn unparseable_value_is_stored_with_null_numeric_and_typed_status() {
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" id="f1">N/A</ix:nonFraction>
    </html>"#;
    let pass = extract_tagged_facts(xml.as_bytes(), "", &no_roles());
    assert_eq!(pass.facts.len(), 1, "decision 9: never a silent drop");
    assert_eq!(pass.facts[0].value_numeric, None);
    assert_eq!(pass.facts[0].parse_status, "unparsed_value");
    assert!(pass.facts[0].parse_error.is_some());
    assert_eq!(pass.encountered_count, 1);
    assert_eq!(pass.stored_count, 1);
}

#[test]
fn a_self_closed_nil_fact_is_stored_never_silently_dropped() {
    // A NIL fact is conventionally self-closed — `Event::Empty`, not a
    // Start/End pair — and must still be counted (decision 9).
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
      xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <ix:nonFraction name="ifrs-full:RetainedEarnings" contextRef="c" id="f1" xsi:nil="true"/>
    </html>"#;
    let pass = extract_tagged_facts(xml.as_bytes(), "", &no_roles());
    assert_eq!(pass.facts.len(), 1);
    assert_eq!(pass.facts[0].value_numeric, None);
    assert_eq!(pass.facts[0].parse_status, "unparsed_value");
    assert_eq!(
        pass.facts[0].parse_error.as_deref(),
        Some("nil fact (xsi:nil=\"true\")")
    );
}

#[test]
fn an_unresolvable_context_is_stored_with_typed_status() {
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full">
      <ix:nonFraction name="ifrs-full:Assets" contextRef="missing" id="f1">1000</ix:nonFraction>
    </html>"#;
    let pass = extract_tagged_facts(xml.as_bytes(), "", &no_roles());
    assert_eq!(pass.facts.len(), 1, "decision 9: never a silent drop");
    assert_eq!(pass.facts[0].parse_status, "unresolved_context");
    assert_eq!(pass.facts[0].value_numeric, None);
}

#[test]
fn an_unresolvable_unit_is_stored_with_typed_status() {
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="missing-unit" id="f1">1000</ix:nonFraction>
    </html>"#;
    let pass = extract_tagged_facts(xml.as_bytes(), "", &no_roles());
    assert_eq!(pass.facts.len(), 1);
    assert_eq!(pass.facts[0].parse_status, "unsupported_unit");
    assert_eq!(pass.facts[0].value_numeric, None);
}

#[test]
fn an_ix_fraction_occurrence_is_counted_never_dropped_though_unparsed() {
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <ix:fraction name="ifrs-full:SomeRatio" contextRef="c" id="f1">
        <ix:numerator>1</ix:numerator><ix:denominator>2</ix:denominator>
      </ix:fraction>
    </html>"#;
    let pass = extract_tagged_facts(xml.as_bytes(), "", &no_roles());
    assert_eq!(pass.facts.len(), 1, "ix:fraction must still be counted");
    assert_eq!(pass.encountered_count, 1);
    assert_eq!(pass.stored_count, 1);
    assert_eq!(pass.facts[0].value_numeric, None);
    assert_eq!(pass.facts[0].parse_status, "unparsed_value");
    assert!(pass.facts[0].parse_error.is_some());
}

// ---------------------------------------------------------------------------
// Repeated tagging (2-3x at an identical concept/context — real, measured)
// ---------------------------------------------------------------------------

#[test]
fn same_concept_and_context_tagged_twice_yields_two_rows_with_distinct_identity() {
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full">
      <xbrli:context id="c"><xbrli:period>
        <xbrli:startDate>2025-01-01</xbrli:startDate><xbrli:endDate>2025-12-31</xbrli:endDate>
      </xbrli:period></xbrli:context>
      <ix:nonFraction name="ifrs-full:ProfitLoss" contextRef="c" id="pl-1" scale="0">100</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:ProfitLoss" contextRef="c" id="pl-2" scale="0">100</ix:nonFraction>
    </html>"#;
    let pass = extract_tagged_facts(xml.as_bytes(), "", &no_roles());
    assert_eq!(pass.facts.len(), 2);
    assert_ne!(pass.facts[0].fact_identity, pass.facts[1].fact_identity);
    assert!(pass.facts.iter().all(|f| f.identity_kind == "xml_id"));
}

#[test]
fn same_concept_and_context_without_an_xml_id_falls_back_to_distinct_occurrence_identities() {
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" scale="0">1</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" scale="0">2</ix:nonFraction>
    </html>"#;
    let pass = extract_tagged_facts(xml.as_bytes(), "", &no_roles());
    assert_eq!(pass.facts.len(), 2);
    assert!(pass.facts.iter().all(|f| f.identity_kind == "occurrence"));
    assert_ne!(pass.facts[0].fact_identity, pass.facts[1].fact_identity);
}

// ---------------------------------------------------------------------------
// Presentation roles (attachment via the concept -> role map)
// ---------------------------------------------------------------------------

#[test]
fn roles_from_the_map_attach_to_the_matching_concept_fact() {
    let mut roles = HashMap::new();
    roles.insert(
        "ProfitLoss".to_owned(),
        vec![
            ("ias_1_role-320000".to_owned(), "income".to_owned()),
            ("ias_1_role-610000".to_owned(), "equity_changes".to_owned()),
        ],
    );
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full">
      <xbrli:context id="c"><xbrli:period>
        <xbrli:startDate>2025-01-01</xbrli:startDate><xbrli:endDate>2025-12-31</xbrli:endDate>
      </xbrli:period></xbrli:context>
      <ix:nonFraction name="ifrs-full:ProfitLoss" contextRef="c" id="f1" scale="0">100</ix:nonFraction>
    </html>"#;
    let pass = extract_tagged_facts(xml.as_bytes(), "", &roles);
    assert_eq!(pass.facts.len(), 1);
    assert_eq!(pass.facts[0].roles.len(), 2);
    let kinds: Vec<&str> = pass.facts[0]
        .roles
        .iter()
        .map(|r| r.role_kind.as_str())
        .collect();
    assert!(kinds.contains(&"income"));
    assert!(kinds.contains(&"equity_changes"));
}

#[test]
fn a_concept_absent_from_the_role_map_gets_no_role_rows() {
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" id="f1">1000</ix:nonFraction>
    </html>"#;
    let pass = extract_tagged_facts(xml.as_bytes(), "", &no_roles());
    assert_eq!(pass.facts.len(), 1);
    assert!(pass.facts[0].roles.is_empty());
}

// ---------------------------------------------------------------------------
// Ship-gate counters
// ---------------------------------------------------------------------------

#[test]
fn encountered_count_equals_stored_count_when_everything_is_representable() {
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="pln" id="f1" scale="3">45 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c" unitRef="pln" id="f2" scale="3">20 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="c" unitRef="pln" id="f3" scale="3">25 000</ix:nonFraction>
    </html>"#;
    let pass = extract_tagged_facts(xml.as_bytes(), "", &no_roles());
    assert_eq!(pass.encountered_count, 3);
    assert_eq!(pass.stored_count, 3);
    assert_eq!(pass.facts.len(), 3);
    assert!(pass.facts.iter().all(|f| f.parse_status == "ok"));
}

#[test]
fn package_entry_path_is_carried_onto_every_fact() {
    let xml = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2023/ifrs-full">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" id="f1">1000</ix:nonFraction>
    </html>"#;
    let pass = extract_tagged_facts(xml.as_bytes(), "reports/instance-a.xhtml", &no_roles());
    assert_eq!(pass.facts.len(), 1);
    assert_eq!(pass.facts[0].package_entry_path, "reports/instance-a.xhtml");
}
