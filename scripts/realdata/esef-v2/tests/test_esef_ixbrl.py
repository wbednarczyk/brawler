import sys
import unittest
from decimal import Decimal
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import esef_ixbrl as ix
from ixbrl_fixtures import (
    context,
    divide_unit,
    explicit_member,
    make_instance,
    non_fraction,
    scenario,
    segment,
    unit,
    zip_package,
)


class NamespaceResolutionTests(unittest.TestCase):
    def test_prefix_resolves_via_document_xmlns(self):
        doc = make_instance(
            contexts=context("c1", start="2025-01-01", end="2025-12-31"),
            units=unit("u1"),
            facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1.234"),
        )
        nsmap = ix.namespace_map(doc)
        local, resolved = ix.qname_local("ifrs-full:Revenue", nsmap)
        self.assertEqual(local, "Revenue")
        self.assertTrue(resolved)
        self.assertIn("ifrs-full", nsmap)

    def test_unresolved_prefix_still_yields_local_name(self):
        # A prefix never declared via xmlns -- a filing defect. Local-name
        # extraction still works (it is a pure string split); qname_local
        # reports it as unresolved rather than raising.
        nsmap = {}
        local, resolved = ix.qname_local("ghost:Revenue", nsmap)
        self.assertEqual(local, "Revenue")
        self.assertFalse(resolved)


class ContextParsingTests(unittest.TestCase):
    def test_entity_segment_dimensions(self):
        # v1 bug: looked up `segment` directly under `context`, not under
        # `entity` -- so entity-segment dimensions were always dropped.
        doc = make_instance(
            contexts=context(
                "c1",
                start="2025-01-01",
                end="2025-12-31",
                segment=segment(explicit_member("ifrs-full:ComponentsOfEquityAxis", "ifrs-full:NoncontrollingInterestsMember")),
            ),
            units=unit("u1"),
            facts=non_fraction("ifrs-full:ProfitLoss", "c1", "u1", "100"),
        )
        result = ix.parse_instance(doc)
        occ = result["occurrences"][0]
        self.assertEqual(
            occ["dimensions"],
            [{"axis": "ifrs-full:ComponentsOfEquityAxis", "member": "ifrs-full:NoncontrollingInterestsMember"}],
        )

    def test_nested_segment_and_scenario_dimensions_combine(self):
        doc = make_instance(
            contexts=context(
                "c1",
                start="2025-01-01",
                end="2025-12-31",
                segment=segment(explicit_member("axis:A", "member:X")),
                scenario=scenario(explicit_member("axis:B", "member:Y")),
            ),
            units=unit("u1"),
            facts=non_fraction("ifrs-full:ProfitLoss", "c1", "u1", "1"),
        )
        occ = ix.parse_instance(doc)["occurrences"][0]
        axes = {d["axis"] for d in occ["dimensions"]}
        self.assertEqual(axes, {"axis:A", "axis:B"})

    def test_unresolvable_context_marks_unparsed(self):
        doc = make_instance(units=unit("u1"), facts=non_fraction("ifrs-full:Revenue", "missing", "u1", "1"))
        occ = ix.parse_instance(doc)["occurrences"][0]
        self.assertEqual(occ["parse_status"], "unparsed")
        self.assertIsNone(occ["period"])


class UnitAndCurrencyTests(unittest.TestCase):
    def test_duplicate_currencies_same_concept_two_units(self):
        doc = make_instance(
            contexts=context("c1", start="2025-01-01", end="2025-12-31"),
            units=unit("u1", "iso4217:PLN") + unit("u2", "iso4217:EUR"),
            facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000")
            + non_fraction("ifrs-full:Revenue", "c1", "u2", "230"),
        )
        occs = ix.parse_instance(doc)["occurrences"]
        self.assertEqual(len(occs), 2)
        self.assertEqual({o["unit"] for o in occs}, {"iso4217:PLN", "iso4217:EUR"})

    def test_divide_unit_recorded_as_numerator_over_denominator(self):
        doc = make_instance(
            contexts=context("c1", start="2025-01-01", end="2025-12-31"),
            units=divide_unit("u1", "iso4217:PLN", "shares"),
            facts=non_fraction("ifrs-full:BasicEarningsLossPerShare", "c1", "u1", "1.5", scale="0", decimals="2"),
        )
        occ = ix.parse_instance(doc)["occurrences"][0]
        self.assertEqual(occ["unit"], "iso4217:PLN/shares")


class TransformTests(unittest.TestCase):
    def test_num_dot_decimal(self):
        value, note = ix.apply_transform("1,234.5", "ixt:num-dot-decimal")
        self.assertIsNone(note)
        self.assertEqual(value, Decimal("1234.5"))

    def test_num_comma_decimal(self):
        value, note = ix.apply_transform("1 234,5", "ixt:num-comma-decimal")
        self.assertIsNone(note)
        self.assertEqual(value, Decimal("1234.5"))

    def test_zerodash(self):
        value, note = ix.apply_transform("-", "ixt:zerodash")
        self.assertEqual(value, Decimal(0))
        self.assertIsNone(note)

    def test_negative_sign_applied_to_scaled_value(self):
        doc = make_instance(
            contexts=context("c1", start="2025-01-01", end="2025-12-31"),
            units=unit("u1"),
            facts=non_fraction(
                "ifrs-full:CashFlowsFromUsedInInvestingActivities", "c1", "u1", "500",
                sign="-", scale="3", decimals="-3", fmt="ixt:num-dot-decimal",
            ),
        )
        occ = ix.parse_instance(doc)["occurrences"][0]
        self.assertEqual(occ["value"], "-500000")
        self.assertEqual(occ["sign"], "-")

    def test_unrecognized_format_marks_unparsed_but_keeps_the_row(self):
        doc = make_instance(
            contexts=context("c1", start="2025-01-01", end="2025-12-31"),
            units=unit("u1"),
            facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1234", fmt="ixt:some-unknown-transform"),
        )
        occ = ix.parse_instance(doc)["occurrences"][0]
        self.assertEqual(occ["parse_status"], "unparsed")
        self.assertIsNone(occ["value"])
        self.assertEqual(occ["raw_text"], "1234")


class ZipMemberTests(unittest.TestCase):
    def test_all_zip_members_read_not_just_first(self):
        pl = make_instance(
            contexts=context("c1", start="2025-01-01", end="2025-12-31"),
            units=unit("u1"),
            facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "100"),
            lang="pl",
        )
        en = make_instance(
            contexts=context("c1", start="2025-01-01", end="2025-12-31"),
            units=unit("u1"),
            facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "100"),
            lang="en",
        )
        non_ixbrl = b"<html><body>auditor opinion, no facts here</body></html>"
        pkg = zip_package({"reports/a_pl.xhtml": pl, "reports/b_en.xhtml": en, "reports/opinion.html": non_ixbrl})
        result = ix.parse_document(pkg)
        self.assertEqual(result["total_members"], 3)
        self.assertEqual(result["non_ixbrl_members"], 1)
        members = {inst["package_member"] for inst in result["instances"]}
        self.assertEqual(members, {"reports/a_pl.xhtml", "reports/b_en.xhtml"})

    def test_loose_non_ixbrl_file_yields_no_instances(self):
        result = ix.parse_document(b"<html><body>just html</body></html>")
        self.assertEqual(result["instances"], [])
        self.assertEqual(result["non_ixbrl_members"], 1)


class ParseErrorTests(unittest.TestCase):
    def test_malformed_xml_reports_parse_error_not_a_crash(self):
        result = ix.parse_instance(b"<html><body><unclosed></body></html>")
        self.assertEqual(result["occurrences"], [])
        self.assertIsNotNone(result["parse_error"])


if __name__ == "__main__":
    unittest.main()
