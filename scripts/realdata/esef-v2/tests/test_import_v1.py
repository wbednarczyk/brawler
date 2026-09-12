import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import import_v1 as iv


def _key_map():
    return iv.load_key_map(Path(__file__).resolve().parent.parent / "gt_key_map.json")


class OwnersSuffixMappingTests(unittest.TestCase):
    def setUp(self):
        self.metric_key_to_entry = _key_map()
        self.issuer_map = {"ZZZ": "iss_01"}

    def test_owners_suffix_maps_to_owners_of_parent_concept(self):
        v1_gt = [
            {
                "file": "doc_zzz_fy2025.zip",
                "ticker": "ZZZ",
                "tier": "esef",
                "mapped_key": "net_profit__owners",
                "period_end": "2025-12-31",
                "period_start": "2025-01-01",
                "statement_basis": "consolidated",
                "value": "500000",
                "currency": "PLN",
                "source": "ixbrl",
                "verification": "machine",
                "uncertain": False,
            }
        ]
        reference, mapped = iv.import_slots(v1_gt, self.issuer_map, self.metric_key_to_entry)
        self.assertEqual(len(reference), 1)
        self.assertEqual(mapped, [])
        self.assertEqual(reference[0]["concept_local"], "ProfitLossAttributableToOwnersOfParent")
        self.assertEqual(reference[0]["attribution"], "owners_of_parent")

    def test_slot_id_follows_the_shared_amendment_f_template(self):
        # Astra r1 finding 5: import_v1.py used to build its own slot_id
        # string by hand, independent of label_esef_v2.build_slot_id -- the
        # two templates drifted the moment the shared one gained
        # package_member/currency. A v1 slot has no package_member ("-").
        v1_gt = [
            {
                "file": "doc_zzz_fy2025.zip", "ticker": "ZZZ", "tier": "esef", "mapped_key": "revenue",
                "period_end": "2025-12-31", "period_start": "2025-01-01", "statement_basis": "consolidated",
                "value": "1000", "currency": "PLN", "source": "ixbrl", "verification": "machine", "uncertain": False,
            }
        ]
        reference, _mapped = iv.import_slots(v1_gt, self.issuer_map, self.metric_key_to_entry)
        self.assertEqual(
            reference[0]["slot_id"],
            f"iss_01/v1/doc_zzz_fy2025/-/{reference[0]['concept_local']}/total/consolidated/flow/reported/2025/v1_unclassified/PLN",
        )
        self.assertEqual(reference[0]["verification"], "machine_v1")

    def test_collapsed_profitloss_suffix_maps_to_total(self):
        v1_gt = [
            {
                "file": "doc_zzz_fy2025.zip",
                "ticker": "ZZZ",
                "tier": "esef",
                "mapped_key": "net_profit__profitloss",
                "period_end": "2025-12-31",
                "period_start": "2025-01-01",
                "statement_basis": "consolidated",
                "value": "500000",
                "currency": "PLN",
                "source": "ixbrl",
                "verification": "machine",
                "uncertain": False,
            }
        ]
        reference, _mapped = iv.import_slots(v1_gt, self.issuer_map, self.metric_key_to_entry)
        self.assertEqual(reference[0]["concept_local"], "ProfitLoss")
        self.assertEqual(reference[0]["attribution"], "total")

    def test_equity_owners_suffix_maps_to_wdf_equity_parent_concept(self):
        v1_gt = [
            {
                "file": "doc_zzz_fy2025.zip", "ticker": "ZZZ", "tier": "esef",
                "mapped_key": "total_equity__owners", "period_end": "2025-12-31", "period_start": None,
                "statement_basis": "consolidated", "value": "1000", "currency": "PLN",
                "source": "ixbrl", "verification": "machine", "uncertain": False,
            }
        ]
        reference, _mapped = iv.import_slots(v1_gt, self.issuer_map, self.metric_key_to_entry)
        self.assertEqual(reference[0]["concept_local"], "EquityAttributableToOwnersOfParent")
        self.assertEqual(reference[0]["attribution"], "owners_of_parent")
        self.assertEqual(reference[0]["window"], "point_in_time")

    def test_positional_tier_records_are_skipped(self):
        v1_gt = [{"tier": "positional", "ticker": "ZZZ", "mapped_key": "revenue"}]
        reference, mapped = iv.import_slots(v1_gt, self.issuer_map, self.metric_key_to_entry)
        self.assertEqual(reference, [])
        self.assertEqual(mapped, [])

    def test_ticker_outside_v2_frame_is_skipped(self):
        v1_gt = [
            {
                "file": "f", "ticker": "OUTSIDE", "tier": "esef", "mapped_key": "revenue",
                "period_end": "2025-12-31", "period_start": None, "statement_basis": "consolidated",
                "value": "1", "currency": "PLN", "source": "ixbrl", "verification": "machine", "uncertain": False,
            }
        ]
        reference, mapped = iv.import_slots(v1_gt, self.issuer_map, self.metric_key_to_entry)
        self.assertEqual(reference, [])
        self.assertEqual(mapped, [])


class MapToEventsTests(unittest.TestCase):
    """Amendment Y / astra r2 finding 21: import_v1.py writes its OWN
    reference artifact and never merges into ground_truth_v2.json unless
    `--map-to-events` explicitly binds a v1 file to a frozen manifest event."""

    def setUp(self):
        self.metric_key_to_entry = _key_map()
        self.issuer_map = {"ZZZ": "iss_01"}
        self.v1_gt = [
            {
                "file": "doc_zzz_fy2025.zip", "ticker": "ZZZ", "tier": "esef", "mapped_key": "revenue",
                "period_end": "2025-12-31", "period_start": "2025-01-01", "statement_basis": "consolidated",
                "value": "1000", "currency": "PLN", "source": "ixbrl", "verification": "machine", "uncertain": False,
            }
        ]

    def test_without_a_mapping_nothing_is_eligible_for_ground_truth(self):
        reference, mapped = iv.import_slots(self.v1_gt, self.issuer_map, self.metric_key_to_entry)
        self.assertEqual(len(reference), 1)
        self.assertEqual(mapped, [])

    def test_mapped_slot_uses_the_real_frozen_event_id_not_the_synthetic_one(self):
        event_map = {"doc_zzz_fy2025.zip": "iss_01/FY2025/pl/consolidated/v1"}
        reference, mapped = iv.import_slots(self.v1_gt, self.issuer_map, self.metric_key_to_entry, event_map)
        self.assertEqual(len(reference), 1)
        self.assertEqual(len(mapped), 1)
        self.assertEqual(mapped[0]["event_id"], "iss_01/FY2025/pl/consolidated/v1")
        self.assertNotEqual(mapped[0]["slot_id"], reference[0]["slot_id"])
        self.assertIn("iss_01/FY2025/pl/consolidated/v1/", mapped[0]["slot_id"])

    def test_main_never_touches_ground_truth_without_map_to_events(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            v1_dir = out / "v1"
            v1_dir.mkdir()
            (v1_dir / "ground_truth.json").write_text(json.dumps(self.v1_gt), encoding="utf-8")
            manifest = {
                "issuers": [{"issuer_id": "iss_01", "ticker": "ZZZ", "exchange": "GPW", "display_name": "ZZZ S.A."}],
                "events": [{"event_id": "iss_01/FY2025/pl/consolidated/v1"}],
            }
            (out / "MANIFEST_v2.json").write_text(json.dumps(manifest), encoding="utf-8")

            iv.main(["--v1-dir", str(v1_dir), "--esef-v2-dir", str(out)])

            self.assertTrue((out / "machine_v1_reference.json").exists())
            self.assertFalse((out / "ground_truth_v2.json").exists())
            ref = json.loads((out / "machine_v1_reference.json").read_text())
            self.assertEqual(len(ref["slots"]), 1)

    def test_main_merges_only_the_explicitly_mapped_rows(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            v1_dir = out / "v1"
            v1_dir.mkdir()
            (v1_dir / "ground_truth.json").write_text(json.dumps(self.v1_gt), encoding="utf-8")
            manifest = {
                "issuers": [{"issuer_id": "iss_01", "ticker": "ZZZ", "exchange": "GPW", "display_name": "ZZZ S.A."}],
                "events": [{"event_id": "iss_01/FY2025/pl/consolidated/v1"}],
            }
            (out / "MANIFEST_v2.json").write_text(json.dumps(manifest), encoding="utf-8")
            mapping_path = out / "mapping.json"
            mapping_path.write_text(json.dumps({"doc_zzz_fy2025.zip": "iss_01/FY2025/pl/consolidated/v1"}), encoding="utf-8")

            iv.main(["--v1-dir", str(v1_dir), "--esef-v2-dir", str(out), "--map-to-events", str(mapping_path)])

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            self.assertEqual(len(gt["slots"]), 1)
            self.assertEqual(gt["slots"][0]["event_id"], "iss_01/FY2025/pl/consolidated/v1")

    def test_main_rejects_a_mapping_to_an_event_id_absent_from_the_manifest(self):
        # Astra r2 finding 21: binding to a non-existent event id is exactly
        # the orphan-event defect this amendment fixes -- fail fast here,
        # never let it reach the harness's own orphan guard.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            v1_dir = out / "v1"
            v1_dir.mkdir()
            (v1_dir / "ground_truth.json").write_text(json.dumps(self.v1_gt), encoding="utf-8")
            manifest = {
                "issuers": [{"issuer_id": "iss_01", "ticker": "ZZZ", "exchange": "GPW", "display_name": "ZZZ S.A."}],
                "events": [{"event_id": "iss_01/FY2025/pl/consolidated/v1"}],
            }
            (out / "MANIFEST_v2.json").write_text(json.dumps(manifest), encoding="utf-8")
            mapping_path = out / "mapping.json"
            mapping_path.write_text(json.dumps({"doc_zzz_fy2025.zip": "iss_01/NOT-A-REAL-EVENT"}), encoding="utf-8")

            with self.assertRaises(ValueError):
                iv.main(["--v1-dir", str(v1_dir), "--esef-v2-dir", str(out), "--map-to-events", str(mapping_path)])
            self.assertFalse((out / "ground_truth_v2.json").exists())


class MergeTests(unittest.TestCase):
    def test_merge_never_overwrites_an_existing_higher_verification_slot(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            existing = {
                "gt_version": "1", "normalization_version": 1, "key_map_version": 1,
                "slots": [{"slot_id": "iss_01/v1/f/ProfitLoss/total/consolidated/flow/reported/2025/v1_unclassified",
                           "verification": "machine", "value": "999"}],
            }
            (out / "ground_truth_v2.json").write_text(json.dumps(existing), encoding="utf-8")
            imported = [dict(existing["slots"][0], value="111", verification="machine_v1")]
            merged = iv.merge_ground_truth(out, imported)
            slot = next(s for s in merged["slots"] if s["slot_id"] == existing["slots"][0]["slot_id"])
            self.assertEqual(slot["verification"], "machine")
            self.assertEqual(slot["value"], "999")

    def test_merge_creates_ground_truth_file_when_absent(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            imported = [
                {"slot_id": "iss_01/v1/f/Revenue/total/consolidated/flow/reported/2025/v1_unclassified",
                 "verification": "machine_v1", "value": "1"}
            ]
            merged = iv.merge_ground_truth(out, imported)
            self.assertEqual(len(merged["slots"]), 1)
            self.assertTrue((out / "ground_truth_v2.json").exists())


if __name__ == "__main__":
    unittest.main()
