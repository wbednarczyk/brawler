import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import label_esef_v2 as lbl
from ixbrl_fixtures import context, explicit_member, make_instance, non_fraction, segment, unit, zip_package


def _write_manifest(out: Path, event: dict, file_bytes: bytes) -> None:
    corpus = out / "corpus"
    corpus.mkdir(parents=True)
    (corpus / event["file"]["name"]).write_bytes(file_bytes)
    manifest = {
        "manifest_version": 1,
        "snapshot": {"source": "test", "taken_at": "2026-01-01"},
        "registry_hash": "x",
        "issuers": [{"issuer_id": "iss_01", "ticker": "ZZZ", "exchange": "GPW", "display_name": "ZZZ S.A."}],
        "events": [event],
        "honest_limitations": [],
    }
    (out / "MANIFEST_v2.json").write_text(json.dumps(manifest), encoding="utf-8")


def _event(basis="consolidated", fiscal_start_month=1) -> dict:
    return {
        "event_id": "iss_01/FY2025/pl/consolidated/v1",
        "issuer_id": "iss_01",
        "role": "floor",
        "language": "pl",
        "basis_scope": basis,
        "vintage": 1,
        "labeled_period": {
            "fiscal_year": 2025,
            "period_type": "FY",
            "period_end": "2025-12-31",
            "period_start": "2025-01-01",
            "fiscal_start_month": fiscal_start_month,
        },
        "file": {"name": "zzz.xhtml", "sha256": "a" * 64, "bytes": 1, "package_member": None},
        "document": {"id": "doc1", "title": "t", "url": "u", "content_type": "application/xhtml+xml", "content_hash": None},
    }


class DuplicateHandlingTests(unittest.TestCase):
    def test_duplicate_currencies_no_conflict(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1", "iso4217:PLN") + unit("u2", "iso4217:EUR"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000")
                + non_fraction("ifrs-full:Revenue", "c1", "u2", "230"),
            )
            _write_manifest(out, _event(), doc)
            gt = lbl.label_esef_v2(str(out))["ground_truth"]
            revenue_slots = [s for s in gt["slots"] if s["concept_local"] == "Revenue"]
            self.assertEqual(len(revenue_slots), 2)
            self.assertTrue(all(s["verification"] == "machine" for s in revenue_slots))
            self.assertEqual({s["currency"] for s in revenue_slots}, {"PLN", "EUR"})

    def test_duplicate_value_conflict_marks_unverified(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000", elem_id="f1")
                + non_fraction("ifrs-full:Revenue", "c1", "u1", "2000", elem_id="f2"),
            )
            _write_manifest(out, _event(), doc)
            gt = lbl.label_esef_v2(str(out))["ground_truth"]
            revenue_slots = [s for s in gt["slots"] if s["concept_local"] == "Revenue"]
            self.assertEqual(len(revenue_slots), 1)
            slot = revenue_slots[0]
            self.assertEqual(slot["verification"], "unverified")
            self.assertEqual(sorted(slot["resolution_ref"]["conflicting_values"]), ["1000000", "2000000"])
            self.assertEqual(len(slot["contributing_occurrence_ids"]), 2)

    def test_agreeing_duplicates_merge_into_one_machine_slot(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000")
                + non_fraction("ifrs-full:Revenue", "c1", "u1", "1000"),
            )
            _write_manifest(out, _event(), doc)
            gt = lbl.label_esef_v2(str(out))["ground_truth"]
            revenue_slots = [s for s in gt["slots"] if s["concept_local"] == "Revenue"]
            self.assertEqual(len(revenue_slots), 1)
            self.assertEqual(revenue_slots[0]["verification"], "machine")
            self.assertEqual(len(revenue_slots[0]["contributing_occurrence_ids"]), 2)


class PeriodClassificationTests(unittest.TestCase):
    def test_nine_month_cumulative_duration_classified_q3_flow(self):
        # GPW convention: a 9-month duration is the Q3 cumulative-YTD
        # filing; window stays "flow" (it is a duration, not an instant).
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-09-30"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "9000"),
            )
            _write_manifest(out, _event(), doc)
            gt = lbl.label_esef_v2(str(out))["ground_truth"]
            self.assertEqual(len(gt["slots"]), 1)
            slot = gt["slots"][0]
            self.assertEqual(slot["period_type"], "Q3")
            self.assertEqual(slot["duration_months"], 9)
            self.assertEqual(slot["window"], "flow")
            self.assertIn("/2025/Q3", slot["slot_id"])


class AttributionMappingTests(unittest.TestCase):
    def test_owners_of_parent_concept_maps_via_key_map(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:ProfitLossAttributableToOwnersOfParent", "c1", "u1", "500"),
            )
            _write_manifest(out, _event(), doc)
            gt = lbl.label_esef_v2(str(out))["ground_truth"]
            self.assertEqual(len(gt["slots"]), 1)
            self.assertEqual(gt["slots"][0]["attribution"], "owners_of_parent")

    def test_nci_dimension_overrides_base_total_attribution(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context(
                    "c1",
                    start="2025-01-01",
                    end="2025-12-31",
                    segment=segment(explicit_member("ifrs-full:ComponentsOfEquityAxis", "ifrs-full:NoncontrollingInterestsMember")),
                ),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:ProfitLoss", "c1", "u1", "50"),
            )
            _write_manifest(out, _event(), doc)
            gt = lbl.label_esef_v2(str(out))["ground_truth"]
            self.assertEqual(len(gt["slots"]), 1)
            self.assertEqual(gt["slots"][0]["attribution"], "nci")


class OccurrenceEmissionTests(unittest.TestCase):
    def test_occurrences_carry_machine_verification_and_no_resolution_ref(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000"),
            )
            _write_manifest(out, _event(), doc)
            lbl.label_esef_v2(str(out))
            occurrences = json.loads((out / "occurrences_v2.json").read_text())
            self.assertEqual(len(occurrences), 1)
            self.assertEqual(occurrences[0]["verification"], "machine")
            self.assertIsNone(occurrences[0]["resolution_ref"])
            self.assertEqual(occurrences[0]["event_id"], "iss_01/FY2025/pl/consolidated/v1")
            self.assertEqual(occurrences[0]["occurrence_id"], "iss_01/FY2025/pl/consolidated/v1#1")


class SlotIdUniquenessTests(unittest.TestCase):
    def test_slot_id_carries_package_member_and_currency(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            pl = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000"),
            )
            pkg = zip_package({"reports/a.xhtml": pl})
            event = dict(_event())
            event["file"] = {"name": "zzz.zip", "sha256": "a" * 64, "bytes": 1, "package_member": "reports/a.xhtml"}
            _write_manifest(out, event, pkg)
            gt = lbl.label_esef_v2(str(out))["ground_truth"]
            self.assertEqual(len(gt["slots"]), 1)
            slot = gt["slots"][0]
            self.assertIn("/reports/a.xhtml/", slot["slot_id"])
            self.assertTrue(slot["slot_id"].endswith("/PLN"))

    def test_different_currencies_never_collide_even_with_same_grouping_prefix(self):
        # Astra r1 finding 5: the old slot_id omitted package_member and
        # currency entirely -- a PLN and an EUR reading of the same concept
        # would silently produce IDENTICAL slot ids and overwrite each other.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1", "iso4217:PLN") + unit("u2", "iso4217:EUR"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000")
                + non_fraction("ifrs-full:Revenue", "c1", "u2", "230"),
            )
            _write_manifest(out, _event(), doc)
            gt = lbl.label_esef_v2(str(out))["ground_truth"]
            slot_ids = {s["slot_id"] for s in gt["slots"]}
            self.assertEqual(len(slot_ids), 2, "PLN and EUR readings must not collide into one slot id")

    def test_decimal_agreement_not_string_agreement(self):
        # "1000" and "1000.00" are the same value -- must merge as `machine`,
        # never flagged `unverified` over a formatting difference.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000", decimals="0", scale="0")
                + non_fraction("ifrs-full:Revenue", "c1", "u1", "1000.00", decimals="2", scale="0"),
            )
            _write_manifest(out, _event(), doc)
            gt = lbl.label_esef_v2(str(out))["ground_truth"]
            self.assertEqual(len(gt["slots"]), 1)
            self.assertEqual(gt["slots"][0]["verification"], "machine")

    def test_unrecognized_dimension_is_occurrence_only_never_a_slot(self):
        # Astra r1 finding 5/amendment F: a segment/geography breakdown must
        # never silently collide with (or be mistaken for) the primary total.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context(
                    "c1",
                    start="2025-01-01",
                    end="2025-12-31",
                    segment=segment(explicit_member("ifrs-full:GeographicalAreasAxis", "ifrs-full:PolandMember")),
                ),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "500"),
            )
            _write_manifest(out, _event(), doc)
            gt = lbl.label_esef_v2(str(out))["ground_truth"]
            self.assertEqual(gt["slots"], [])
            occurrences = json.loads((out / "occurrences_v2.json").read_text())
            self.assertEqual(len(occurrences), 1)  # kept as evidence, just not a slot


class PackageBoundaryTests(unittest.TestCase):
    def test_every_member_of_the_package_is_labeled_not_just_the_manifest_primary(self):
        # Astra r1 finding 7: event boundary = the whole package. The
        # manifest's `file.package_member` names only the PRIMARY member
        # (used for the event's headline classification); every member must
        # still be parsed and contribute occurrences/slots.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            primary = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000"),
            )
            other = make_instance(
                contexts=context("c2", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:GrossProfit", "c2", "u1", "400"),
            )
            pkg = zip_package({"reports/primary.xhtml": primary, "reports/other.xhtml": other})
            event = dict(_event())
            event["file"] = {"name": "zzz.zip", "sha256": "a" * 64, "bytes": 1, "package_member": "reports/primary.xhtml"}
            _write_manifest(out, event, pkg)

            gt = lbl.label_esef_v2(str(out))["ground_truth"]
            concepts = {s["concept_local"] for s in gt["slots"]}
            self.assertEqual(concepts, {"Revenue", "GrossProfit"})
            members = {s["package_member"] for s in gt["slots"]}
            self.assertEqual(members, {"reports/primary.xhtml", "reports/other.xhtml"})


class NamespaceMatchingTests(unittest.TestCase):
    def test_non_ifrs_extension_concept_never_maps_to_a_slot(self):
        # Astra r1 finding 11: matching must use the EXPANDED name against
        # the IFRS namespace family -- an extension taxonomy's own "Revenue"
        # concept must never be scored as if it were ifrs-full:Revenue.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("company-ext:Revenue", "c1", "u1", "999"),
            ).replace(
                b'xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2021-01-01/ifrs-full"',
                b'xmlns:ifrs-full="http://xbrl.ifrs.org/taxonomy/2021-01-01/ifrs-full" '
                b'xmlns:company-ext="https://zzz.example.com/2025-12-31/company-ext"',
            )
            _write_manifest(out, _event(), doc)
            gt = lbl.label_esef_v2(str(out))["ground_truth"]
            self.assertEqual(gt["slots"], [])
            occurrences = json.loads((out / "occurrences_v2.json").read_text())
            self.assertEqual(occurrences[0]["concept_local"], "Revenue")  # still evidence
            self.assertTrue(occurrences[0]["concept_qname"].startswith("{https://zzz.example.com/"))


class IfrsNamespaceGateTests(unittest.TestCase):
    """Real-data run 2026-09-12: the 2024-03-27 ESEF taxonomy publishes under
    https://xbrl.ifrs.org — the gate must accept both schemes and still refuse
    an issuer extension namespace."""

    def test_https_and_http_ifrs_namespaces_are_accepted_extensions_refused(self):
        import label_esef_v2 as lab

        self.assertTrue(lab.is_ifrs_concept("{https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full}Revenue"))
        self.assertTrue(lab.is_ifrs_concept("{http://xbrl.ifrs.org/taxonomy/2023-03-23/ifrs-full}Assets"))
        self.assertFalse(lab.is_ifrs_concept("{http://www.example.pl/xbrl/2025-12-31}Revenue"))
        self.assertFalse(lab.is_ifrs_concept(None))


class DimensionExclusionTests(unittest.TestCase):
    def test_nci_plus_geography_axis_is_not_a_slot(self):
        # Astra r2 finding 5 / amendment Z: a Parent/NCI member ALONGSIDE any
        # other axis (segment/geography/class) must NOT become a slot -- the
        # old rule returned on the first NCI/Parent match and ignored the
        # rest of the dimension set.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context(
                    "c1",
                    start="2025-01-01",
                    end="2025-12-31",
                    segment=segment(
                        explicit_member("ifrs-full:ComponentsOfEquityAxis", "ifrs-full:NoncontrollingInterestsMember"),
                        explicit_member("ifrs-full:GeographicalAreasAxis", "ifrs-full:PolandMember"),
                    ),
                ),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:ProfitLoss", "c1", "u1", "50"),
            )
            _write_manifest(out, _event(), doc)
            result = lbl.label_esef_v2(str(out))
            self.assertEqual(result["ground_truth"]["slots"], [])
            self.assertEqual(result["dimensional_occurrences"], 1)

    def test_bare_nci_member_alone_still_forms_a_slot(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context(
                    "c1",
                    start="2025-01-01",
                    end="2025-12-31",
                    segment=segment(explicit_member("ifrs-full:ComponentsOfEquityAxis", "ifrs-full:NoncontrollingInterestsMember")),
                ),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:ProfitLoss", "c1", "u1", "50"),
            )
            _write_manifest(out, _event(), doc)
            result = lbl.label_esef_v2(str(out))
            self.assertEqual(len(result["ground_truth"]["slots"]), 1)
            self.assertEqual(result["ground_truth"]["slots"][0]["attribution"], "nci")
            self.assertEqual(result["dimensional_occurrences"], 0)


class MemberLanguageTests(unittest.TestCase):
    def test_occurrence_and_slot_carry_member_language(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000"),
                lang="pl",
            )
            _write_manifest(out, _event(), doc)
            result = lbl.label_esef_v2(str(out))
            occurrences = json.loads((out / "occurrences_v2.json").read_text())
            self.assertEqual(occurrences[0]["language"], "pl")
            self.assertEqual(result["ground_truth"]["slots"][0]["language"], "pl")
            self.assertEqual(result["ground_truth"]["slots"][0]["population"], "floor")

    def test_other_language_member_is_twin_diagnostic_population_inside_the_same_event(self):
        # Amendment S: a bilingual PACKAGE -- the EN member's slots are
        # twin_diagnostic population INSIDE the same event, not a separate
        # event and not silently merged into the floor population.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            pl = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000"),
                lang="pl",
            )
            en = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000"),
                lang="en",
            )
            pkg = zip_package({"reports/a_pl.xhtml": pl, "reports/b_en.xhtml": en})
            event = dict(_event())
            event["file"] = {"name": "zzz.zip", "sha256": "a" * 64, "bytes": 1, "package_member": "reports/a_pl.xhtml"}
            _write_manifest(out, event, pkg)
            result = lbl.label_esef_v2(str(out), pin_language="pl")
            slots = result["ground_truth"]["slots"]
            self.assertEqual(len(slots), 2)
            by_lang = {s["language"]: s["population"] for s in slots}
            self.assertEqual(by_lang, {"pl": "floor", "en": "twin_diagnostic"})


class MemberBasisContradictionTests(unittest.TestCase):
    def test_member_basis_contradicting_headline_stays_unknown_and_queued(self):
        # Astra r2 finding 8 / amendment T: the OLD code discarded the
        # contradiction flag and substituted the event's headline basis --
        # a standalone member inside a nominally-consolidated package would
        # silently read "consolidated".
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000"),
            )
            event = dict(_event(basis="consolidated"))
            event["file"] = {"name": "zzz.zip", "sha256": "a" * 64, "bytes": 1, "package_member": "reports/sprawozdanie_jednostkowe.xhtml"}
            pkg = zip_package({"reports/sprawozdanie_jednostkowe.xhtml": doc})
            _write_manifest(out, event, pkg)
            result = lbl.label_esef_v2(str(out))
            self.assertEqual(len(result["ground_truth"]["slots"]), 1)
            self.assertEqual(result["ground_truth"]["slots"][0]["basis"], "unknown")

            review_queue = json.loads((out / "review-queue.json").read_text())
            entry = next(r for r in review_queue if r["package_member"] == "reports/sprawozdanie_jednostkowe.xhtml")
            self.assertIn("member basis contradicts the event's headline basis", entry["reasons"])


class MemberParseFailureTests(unittest.TestCase):
    def test_member_parse_error_counted_and_queued(self):
        # Astra r2 finding 9 / amendment U: a malformed member alongside a
        # valid primary member used to disappear with zero unresolved
        # occurrences to show for it.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            good = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000"),
            )
            # sniff_ixbrl requires valid XML, so simulate a member that sniffs
            # as iXBRL-shaped (has ix:/xbrli: tags) but is malformed enough
            # to fail the SECOND (real) parse -- an unterminated CDATA/tag
            # injected after a well-formed-looking prefix cannot happen with
            # a single ET.fromstring pass, so this test drives the parse
            # path directly instead of round-tripping through sniff.
            broken = b"<html><body><ix:nonFraction>unterminated</body></html>"
            event = dict(_event())
            event["file"] = {"name": "zzz.zip", "sha256": "a" * 64, "bytes": 1, "package_member": "reports/good.xhtml"}
            pkg = zip_package({"reports/good.xhtml": good, "reports/broken.xhtml": broken})
            _write_manifest(out, event, pkg)

            import esef_ixbrl as ix

            # Confirm the fixture actually reproduces a parse error via the
            # real code path label_esef_v2 uses (parse_instance, not sniff).
            self.assertIsNotNone(ix.parse_instance(broken)["parse_error"])

            result = lbl.label_esef_v2(str(out))
            self.assertEqual(result["unresolved_members"], 1)
            review_queue = json.loads((out / "review-queue.json").read_text())
            entry = next(r for r in review_queue if r["package_member"] == "reports/broken.xhtml")
            self.assertTrue(any("member parse error" in reason for reason in entry["reasons"]))


class FiscalCalendarTests(unittest.TestCase):
    def test_march_year_end_instant_classified_fy_not_q1_with_shifted_fiscal_start(self):
        # Astra r2 finding 10 / amendment V: the frame's persisted
        # fiscal_start_month must classify EVERY occurrence, instants
        # included -- a March year-end balance sheet is FY when the fiscal
        # year starts in April, never Q1 by raw calendar month.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", instant="2025-03-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Assets", "c1", "u1", "5000"),
            )
            _write_manifest(out, _event(fiscal_start_month=4), doc)
            result = lbl.label_esef_v2(str(out))
            slots = result["ground_truth"]["slots"]
            self.assertEqual(len(slots), 1)
            self.assertEqual(slots[0]["period_type"], "FY")

    def test_calendar_year_instant_unaffected(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", instant="2025-03-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Assets", "c1", "u1", "5000"),
            )
            _write_manifest(out, _event(fiscal_start_month=1), doc)
            result = lbl.label_esef_v2(str(out))
            self.assertEqual(result["ground_truth"]["slots"][0]["period_type"], "Q1")


class UnmappedConceptSlotTests(unittest.TestCase):
    def test_unmapped_ifrs_concept_still_gets_a_slot_flagged_unmapped(self):
        # Amendment Q / astra r2 finding 14 (labeler half): EVERY IFRS
        # concept gets a slot now, mapped through gt_key_map.json or not.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:ResearchAndDevelopmentExpense", "c1", "u1", "500"),
            )
            _write_manifest(out, _event(), doc)
            result = lbl.label_esef_v2(str(out))
            slots = result["ground_truth"]["slots"]
            self.assertEqual(len(slots), 1)
            self.assertFalse(slots[0]["mapped"])
            self.assertEqual(slots[0]["attribution"], "total")

    def test_mapped_concept_flagged_mapped_true(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000"),
            )
            _write_manifest(out, _event(), doc)
            result = lbl.label_esef_v2(str(out))
            self.assertTrue(result["ground_truth"]["slots"][0]["mapped"])

    def test_normalized_by_tags_interim_duration_and_cash_flow_concept(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-06-30"),
                units=unit("u1"),
                facts=(
                    non_fraction("ifrs-full:Revenue", "c1", "u1", "1000")
                    + non_fraction("ifrs-full:CashFlowsFromUsedInOperatingActivities", "c1", "u1", "200")
                ),
            )
            _write_manifest(out, _event(), doc)
            result = lbl.label_esef_v2(str(out))
            by_concept = {s["concept_local"]: s["normalized_by"] for s in result["ground_truth"]["slots"]}
            self.assertEqual(by_concept["Revenue"], ["cumulative_context_to_flow"])
            self.assertEqual(by_concept["CashFlowsFromUsedInOperatingActivities"], ["cumulative_context_to_flow", "cash_flow_outflow_sign"])

    def test_normalized_by_empty_for_fy_non_cash_flow_slot(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            doc = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000"),
            )
            _write_manifest(out, _event(), doc)
            result = lbl.label_esef_v2(str(out))
            self.assertEqual(result["ground_truth"]["slots"][0]["normalized_by"], [])


if __name__ == "__main__":
    unittest.main()
