import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import adjudicate as adj


def _slot(concept, verification, value, event_id="iss_01/FY2025/pl/consolidated/v1", resolution_ref=None,
          package_member=None, attribution="total", basis="consolidated", currency="PLN",
          fiscal_year=2025, period_type="FY"):
    return {
        "slot_id": adj.build_slot_id(event_id, package_member, concept, attribution, basis, "flow", "reported", fiscal_year, period_type, currency),
        "event_id": event_id,
        "package_member": package_member,
        "concept_local": concept,
        "attribution": attribution,
        "basis": basis,
        "window": "flow",
        "variant": "reported",
        "fiscal_year": fiscal_year,
        "period_type": period_type,
        "period_end": "2025-12-31",
        "period_start": "2025-01-01",
        "currency": currency,
        "value": value,
        "duration_months": 12,
        "verification": verification,
        "contributing_occurrence_ids": [f"{event_id}#1"],
        "resolution_ref": resolution_ref,
    }


def _full_answer(slot, **overrides):
    """A complete, filing-anchored reader answer confirming (by default)
    every normalized field of `slot` -- override individual fields to
    express a genuine disagreement."""
    answer = {
        "value": slot["value"],
        "currency": slot["currency"],
        "basis": slot["basis"],
        "attribution": slot["attribution"],
        "fiscal_year": slot["fiscal_year"],
        "period_type": slot["period_type"],
        "evidence_anchor": "p.1",
    }
    answer.update(overrides)
    return answer


def _write_gt(out: Path, slots: list[dict]) -> None:
    (out / "ground_truth_v2.json").write_text(
        json.dumps({"gt_version": "1", "normalization_version": 1, "key_map_version": 1, "slots": slots}),
        encoding="utf-8",
    )


def _seal_all(out: Path, reader: str, index: dict, answer_for) -> None:
    """Seals a complete answer set for every task in `index` -- `compare`
    now refuses to run over a partial set (amendment N)."""
    answers = {task_id: answer_for(task_id, meta) for task_id, meta in index.items()}
    answers_path = out / f"{reader}-answers.json"
    answers_path.write_text(json.dumps({"answers": answers}), encoding="utf-8")
    adj.seal(out, reader, answers_path)


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

    def test_prepare_refuses_to_overwrite_an_existing_task_set(self):
        # Amendment N: task/index/GT bindings are frozen by hash once
        # prepared -- re-preparing would associate old sealed answers with
        # newly (re-)assigned task ids.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            _write_gt(out, [_slot("Revenue", "machine", "1")])
            adj.prepare(out, seed=1)
            with self.assertRaises(adj.PrepareExistsError):
                adj.prepare(out, seed=2)


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
            answers_path.write_text(
                json.dumps({"answers": {task_id: {"value": "1", "currency": "PLN", "basis": "consolidated",
                                                   "attribution": "total", "fiscal_year": 2025, "period_type": "FY",
                                                   "evidence_anchor": "p.3"}}})
            )

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

    def test_seal_refuses_to_reseal_an_existing_reader(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            index = self._prepared(out)
            task_id = next(iter(index))
            answers_path = out / "answers.json"
            answers_path.write_text(json.dumps({"answers": {task_id: {"value": "1"}}}))
            adj.seal(out, "reader_a", answers_path)
            with self.assertRaises(adj.ResealError):
                adj.seal(out, "reader_a", answers_path)

    def test_seal_refuses_when_task_file_hash_no_longer_matches_the_lock(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            index = self._prepared(out)
            task_id = next(iter(index))
            # Tamper with the task file after prepare -- its hash no longer
            # matches tasks.lock.
            task_path = out / "adjudication" / "tasks" / f"{task_id}.json"
            task_path.write_text(task_path.read_text() + "  ")
            answers_path = out / "answers.json"
            answers_path.write_text(json.dumps({"answers": {task_id: {"value": "1"}}}))
            with self.assertRaises(adj.TaskHashMismatchError):
                adj.seal(out, "reader_a", answers_path)


class CompareTests(unittest.TestCase):
    def test_compare_refuses_when_a_prepared_task_has_no_sealed_answer(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            _write_gt(out, [_slot("Revenue", "machine", "1"), _slot("GrossProfit", "machine", "2")])
            adj.prepare(out, seed=1)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            one_task = next(iter(index))
            answers_path = out / "answers.json"
            answers_path.write_text(json.dumps({"answers": {one_task: {"value": "1"}}}))
            adj.seal(out, "reader_a", answers_path)
            with self.assertRaises(adj.IncompleteAdjudicationError):
                adj.compare(out)

    def test_compare_never_writes_a_value_the_app_supplied(self):
        # The resolved slot value after `compare` must come from a reader
        # answer, never from anywhere else.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "unverified", "1000", resolution_ref={"conflicting_values": ["1000", "2000"], "occurrence_ids": []})
            _write_gt(out, [slot])
            adj.prepare(out, seed=3)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            task_id = next(iter(index))
            _seal_all(out, "reader_a", index, lambda tid, meta: _full_answer(slot, value="2000"))
            adj.compare(out)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            new_slot = gt["slots"][0]
            self.assertEqual(new_slot["verification"], "adjudicated")
            self.assertEqual(new_slot["value"], "2000")
            self.assertIsNone(new_slot["resolution_ref"])

    def test_agreement_confirmed_becomes_second_read(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            _write_gt(out, [slot])
            adj.prepare(out, seed=5)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            _seal_all(out, "reader_b", index, lambda tid, meta: _full_answer(slot))
            adj.compare(out)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            self.assertEqual(gt["slots"][0]["verification"], "second_read")

    def test_agreement_contradicted_with_evidence_becomes_adjudicated(self):
        # LABELING.md step 6: a contradicted agreement-sample slot is
        # exactly the systematic-error signal -- the reader's independent,
        # filing-anchored read settles this slot as `adjudicated`.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            _write_gt(out, [slot])
            adj.prepare(out, seed=5)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            _seal_all(out, "reader_b", index, lambda tid, meta: _full_answer(slot, value="999"))
            adj.compare(out)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            self.assertEqual(gt["slots"][0]["verification"], "adjudicated")
            self.assertEqual(gt["slots"][0]["value"], "999")

    def test_disagreement_without_evidence_anchor_is_refused_not_applied(self):
        # Amendment N / astra r1 finding 12: an adjudicated change requires
        # an evidence anchor -- never silently applied.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            _write_gt(out, [slot])
            adj.prepare(out, seed=5)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            _seal_all(out, "reader_b", index, lambda tid, meta: _full_answer(slot, value="999", evidence_anchor=None))
            adj.compare(out)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            self.assertEqual(gt["slots"][0]["verification"], "machine")  # unchanged, refused
            self.assertEqual(gt["slots"][0]["value"], "42")  # unchanged, refused
            resolutions = json.loads((out / "adjudication" / "resolutions.json").read_text())
            self.assertEqual(resolutions[0]["outcome"], "rejected_missing_evidence")

    def test_currency_only_disagreement_is_not_silently_accepted_as_agree(self):
        # Astra r1 finding 12: comparing value alone let a same-number,
        # different-currency answer pass as `second_read`.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "1000", currency="PLN")
            _write_gt(out, [slot])
            adj.prepare(out, seed=5)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            _seal_all(out, "reader_b", index, lambda tid, meta: _full_answer(slot, currency="EUR"))
            adj.compare(out)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            self.assertEqual(gt["slots"][0]["verification"], "adjudicated")
            self.assertEqual(gt["slots"][0]["currency"], "EUR")

    def test_attribution_correction_rekeys_slot_id_atomically(self):
        # Amendment N: "update/re-key corrected dimensions atomically".
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("ProfitLoss", "machine", "42", attribution="total")
            old_slot_id = slot["slot_id"]
            _write_gt(out, [slot])
            adj.prepare(out, seed=5)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            _seal_all(out, "reader_b", index, lambda tid, meta: _full_answer(slot, attribution="nci"))
            adj.compare(out)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            self.assertEqual(len(gt["slots"]), 1)
            new_slot = gt["slots"][0]
            self.assertEqual(new_slot["attribution"], "nci")
            self.assertNotEqual(new_slot["slot_id"], old_slot_id)
            self.assertIn("/nci/", new_slot["slot_id"])

    def test_reader_marked_unverified_answer_stays_unverified(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            _write_gt(out, [slot])
            adj.prepare(out, seed=5)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            _seal_all(out, "reader_b", index, lambda tid, meta: {"unverified": True, "reason": "figure not legible in the filing"})
            adj.compare(out)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            self.assertEqual(gt["slots"][0]["verification"], "unverified")
            self.assertEqual(gt["slots"][0]["value"], "42")  # unsettled never overwrites the machine value

    def test_multiple_readers_disagreeing_stays_unverified(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            _write_gt(out, [slot])
            adj.prepare(out, seed=5)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            task_id = next(iter(index))
            answers_path_a = out / "a.json"
            answers_path_a.write_text(json.dumps({"answers": {task_id: _full_answer(slot, value="999")}}))
            adj.seal(out, "reader_a", answers_path_a)
            answers_path_b = out / "b.json"
            answers_path_b.write_text(json.dumps({"answers": {task_id: _full_answer(slot, value="888")}}))
            adj.seal(out, "reader_b", answers_path_b)
            adj.compare(out)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            self.assertEqual(gt["slots"][0]["verification"], "unverified")
            self.assertEqual(gt["slots"][0]["value"], "42")  # neither reader's value applied


if __name__ == "__main__":
    unittest.main()
