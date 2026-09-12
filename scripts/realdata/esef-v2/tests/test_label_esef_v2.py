import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import label_esef_v2 as lbl
from ixbrl_fixtures import context, explicit_member, make_instance, non_fraction, segment, unit


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


def _event(basis="consolidated") -> dict:
    return {
        "event_id": "iss_01/FY2025/pl/consolidated/v1",
        "issuer_id": "iss_01",
        "role": "floor",
        "language": "pl",
        "basis_scope": basis,
        "vintage": 1,
        "labeled_period": {"fiscal_year": 2025, "period_type": "FY", "period_end": "2025-12-31", "period_start": "2025-01-01"},
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
            gt = lbl.label_esef_v2(str(out))
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
            gt = lbl.label_esef_v2(str(out))
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
            gt = lbl.label_esef_v2(str(out))
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
            gt = lbl.label_esef_v2(str(out))
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
            gt = lbl.label_esef_v2(str(out))
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
            gt = lbl.label_esef_v2(str(out))
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


if __name__ == "__main__":
    unittest.main()
