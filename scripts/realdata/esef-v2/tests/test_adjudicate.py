import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import adjudicate as adj


def _slot(concept, verification, value, event_id="iss_01/FY2025/pl/consolidated/v1", resolution_ref=None):
    return {
        "slot_id": f"{event_id}/{concept}/total/consolidated/flow/reported/2025/FY",
        "event_id": event_id,
        "concept_local": concept,
        "attribution": "total",
        "basis": "consolidated",
        "window": "flow",
        "variant": "reported",
        "fiscal_year": 2025,
        "period_type": "FY",
        "period_end": "2025-12-31",
        "period_start": "2025-01-01",
        "currency": "PLN",
        "value": value,
        "duration_months": 12,
        "verification": verification,
        "contributing_occurrence_ids": [f"{event_id}#1"],
        "resolution_ref": resolution_ref,
    }


def _write_gt(out: Path, slots: list[dict]) -> None:
    (out / "ground_truth_v2.json").write_text(
        json.dumps({"gt_version": "1", "normalization_version": 1, "key_map_version": 1, "slots": slots}),
        encoding="utf-8",
    )


class PrepareTests(unittest.TestCase):
    def test_all_disagreements_included_plus_ten_percent_sample_minimum_ten_tasks(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slots = [_slot("Revenue", "unverified", "1", resolution_ref={"conflicting_values": ["1", "2"], "occurrence_ids": []})]
            slots += [_slot(f"Concept{i}", "machine", "1") for i in range(20)]
            _write_gt(out, slots)

            result = adj.prepare(out, seed=42)
            self.assertEqual(result["disagreements"], 1)
            self.assertEqual(result["agreement_sample"], 2)  # 10% of 20
            self.assertGreaterEqual(result["tasks"], adj.MIN_TASKS)

            tasks = sorted((out / "adjudication" / "tasks").glob("*.json"))
            self.assertEqual(len(tasks), result["tasks"])

    def test_task_files_never_carry_a_candidate_value_or_task_kind(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slots = [_slot("Revenue", "unverified", "1", resolution_ref={"conflicting_values": ["1", "2"], "occurrence_ids": []})]
            slots += [_slot(f"Concept{i}", "machine", "1") for i in range(20)]
            _write_gt(out, slots)
            adj.prepare(out, seed=1)

            for task_file in (out / "adjudication" / "tasks").glob("*.json"):
                task = json.loads(task_file.read_text())
                self.assertNotIn("value", task)
                self.assertNotIn("task_kind", task)
                self.assertNotIn("verification", task)

    def test_deterministic_given_the_same_seed(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slots = [_slot(f"Concept{i}", "machine", "1") for i in range(30)]
            _write_gt(out, slots)
            r1 = adj.prepare(out, seed=7)
            index1 = json.loads((out / "adjudication" / "tasks_index.json").read_text())

            out2 = out / "again"
            out2.mkdir()
            _write_gt(out2, slots)
            adj.prepare(out2, seed=7)
            index2 = json.loads((out2 / "adjudication" / "tasks_index.json").read_text())

            self.assertEqual(
                [v["slot_id"] for v in index1.values()],
                [v["slot_id"] for v in index2.values()],
            )
            self.assertEqual(r1["tasks"], adj.MIN_TASKS)  # 30 * 10% = 3 < MIN_TASKS, topped up

    def test_minimum_ten_or_all_if_fewer(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slots = [_slot("Revenue", "machine", "1")]
            _write_gt(out, slots)
            result = adj.prepare(out, seed=1)
            self.assertEqual(result["tasks"], 1)  # only one slot exists total


class SealTests(unittest.TestCase):
    def _prepared(self, out: Path):
        slots = [_slot("Revenue", "unverified", "1", resolution_ref={"conflicting_values": ["1", "2"], "occurrence_ids": []})]
        slots += [_slot(f"Concept{i}", "machine", "1") for i in range(9)]
        _write_gt(out, slots)
        adj.prepare(out, seed=1)
        return json.loads((out / "adjudication" / "tasks_index.json").read_text())

    def test_seal_writes_under_the_event_id_path(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            index = self._prepared(out)
            task_id = next(iter(index))
            event_id = index[task_id]["event_id"]
            answers_path = out / "answers.json"
            answers_path.write_text(json.dumps({"answers": {task_id: {"value": "1", "evidence_anchor": "p.3"}}}))

            sealed = adj.seal(out, "reader_a", answers_path)
            self.assertEqual(len(sealed), 1)
            self.assertTrue((out / "adjudication" / event_id / "reader_a.json").exists())
            payload = json.loads((out / "adjudication" / event_id / "reader_a.json").read_text())
            self.assertIn("sealed_at", payload)
            self.assertIn("sha256", payload)

    def test_seal_refuses_after_compare_has_run(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            (out / "adjudication").mkdir()
            (out / "adjudication" / "resolutions.json").write_text("[]")
            answers_path = out / "answers.json"
            answers_path.write_text(json.dumps({"answers": {}}))
            with self.assertRaises(adj.SealingClosedError):
                adj.seal(out, "reader_a", answers_path)


class CompareTests(unittest.TestCase):
    def test_compare_never_writes_a_value_the_app_supplied(self):
        # The resolved slot value after `compare` must come from a reader
        # answer, never from anywhere else -- assert it equals the sealed
        # reader value exactly for both outcomes.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slots = [
                _slot("Revenue", "unverified", "1", resolution_ref={"conflicting_values": ["1000", "2000"], "occurrence_ids": []}),
            ]
            slots[0]["value"] = "1000"
            slots += [_slot(f"Concept{i}", "machine", "1") for i in range(9)]
            _write_gt(out, slots)
            index = adj_index = None
            result = adj.prepare(out, seed=3)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())

            disagreement_task = next(tid for tid, meta in index.items() if meta["task_kind"] == "disagreement")
            event_id = index[disagreement_task]["event_id"]
            answers_path = out / "answers.json"
            answers_path.write_text(
                json.dumps({"answers": {disagreement_task: {"value": "2000", "evidence_anchor": "note p.5"}}})
            )
            adj.seal(out, "reader_a", answers_path)
            adj.compare(out)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            slot = next(s for s in gt["slots"] if s["slot_id"] == index[disagreement_task]["slot_id"])
            self.assertEqual(slot["verification"], "adjudicated")
            self.assertEqual(slot["value"], "2000")
            self.assertIsNone(slot["resolution_ref"])

    def test_agreement_sample_confirmed_becomes_second_read(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slots = [_slot(f"Concept{i}", "machine", "42") for i in range(9)]
            _write_gt(out, slots)
            adj.prepare(out, seed=5)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            sample_task = next(tid for tid, meta in index.items() if meta["task_kind"] == "agreement_sample")
            answers_path = out / "answers.json"
            answers_path.write_text(json.dumps({"answers": {sample_task: {"value": "42", "evidence_anchor": "p.1"}}}))
            adj.seal(out, "reader_b", answers_path)
            adj.compare(out)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            slot = next(s for s in gt["slots"] if s["slot_id"] == index[sample_task]["slot_id"])
            self.assertEqual(slot["verification"], "second_read")

    def test_agreement_sample_contradicted_becomes_adjudicated_systematic_error_signal(self):
        # LABELING.md step 6: a contradicted agreement-sample slot is
        # exactly the systematic-error signal -- the reader's independent,
        # filing-anchored read settles this slot as `adjudicated`; the owner
        # decides separately whether the whole class needs a full re-read.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slots = [_slot(f"Concept{i}", "machine", "42") for i in range(9)]
            _write_gt(out, slots)
            adj.prepare(out, seed=5)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            sample_task = next(tid for tid, meta in index.items() if meta["task_kind"] == "agreement_sample")
            answers_path = out / "answers.json"
            answers_path.write_text(json.dumps({"answers": {sample_task: {"value": "999", "evidence_anchor": "p.1"}}}))
            adj.seal(out, "reader_b", answers_path)
            adj.compare(out)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            slot = next(s for s in gt["slots"] if s["slot_id"] == index[sample_task]["slot_id"])
            self.assertEqual(slot["verification"], "adjudicated")
            self.assertEqual(slot["value"], "999")

    def test_reader_marked_unverified_answer_stays_unverified(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slots = [_slot(f"Concept{i}", "machine", "42") for i in range(9)]
            _write_gt(out, slots)
            adj.prepare(out, seed=5)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            sample_task = next(tid for tid, meta in index.items() if meta["task_kind"] == "agreement_sample")
            answers_path = out / "answers.json"
            answers_path.write_text(
                json.dumps({"answers": {sample_task: {"unverified": True, "reason": "figure not legible in the filing"}}})
            )
            adj.seal(out, "reader_b", answers_path)
            adj.compare(out)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            slot = next(s for s in gt["slots"] if s["slot_id"] == index[sample_task]["slot_id"])
            self.assertEqual(slot["verification"], "unverified")
            self.assertEqual(slot["value"], "42")  # unsettled never overwrites the machine value


if __name__ == "__main__":
    unittest.main()
