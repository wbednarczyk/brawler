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
        slots = iv.import_slots(v1_gt, self.issuer_map, self.metric_key_to_entry)
        self.assertEqual(len(slots), 1)
        self.assertEqual(slots[0]["concept_local"], "ProfitLossAttributableToOwnersOfParent")
        self.assertEqual(slots[0]["attribution"], "owners_of_parent")
        self.assertEqual(slots[0]["verification"], "machine_v1")

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
        slots = iv.import_slots(v1_gt, self.issuer_map, self.metric_key_to_entry)
        self.assertEqual(slots[0]["concept_local"], "ProfitLoss")
        self.assertEqual(slots[0]["attribution"], "total")

    def test_equity_owners_suffix_maps_to_wdf_equity_parent_concept(self):
        v1_gt = [
            {
                "file": "doc_zzz_fy2025.zip", "ticker": "ZZZ", "tier": "esef",
                "mapped_key": "total_equity__owners", "period_end": "2025-12-31", "period_start": None,
                "statement_basis": "consolidated", "value": "1000", "currency": "PLN",
                "source": "ixbrl", "verification": "machine", "uncertain": False,
            }
        ]
        slots = iv.import_slots(v1_gt, self.issuer_map, self.metric_key_to_entry)
        self.assertEqual(slots[0]["concept_local"], "EquityAttributableToOwnersOfParent")
        self.assertEqual(slots[0]["attribution"], "owners_of_parent")
        self.assertEqual(slots[0]["window"], "point_in_time")

    def test_positional_tier_records_are_skipped(self):
        v1_gt = [{"tier": "positional", "ticker": "ZZZ", "mapped_key": "revenue"}]
        slots = iv.import_slots(v1_gt, self.issuer_map, self.metric_key_to_entry)
        self.assertEqual(slots, [])

    def test_ticker_outside_v2_frame_is_skipped(self):
        v1_gt = [
            {
                "file": "f", "ticker": "OUTSIDE", "tier": "esef", "mapped_key": "revenue",
                "period_end": "2025-12-31", "period_start": None, "statement_basis": "consolidated",
                "value": "1", "currency": "PLN", "source": "ixbrl", "verification": "machine", "uncertain": False,
            }
        ]
        slots = iv.import_slots(v1_gt, self.issuer_map, self.metric_key_to_entry)
        self.assertEqual(slots, [])


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
