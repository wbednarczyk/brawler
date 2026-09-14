import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import adjudicate as adj


def _slot(concept, verification, value, event_id="iss_01/FY2025/pl/consolidated/v1", resolution_ref=None,
          package_member=None, attribution="total", basis="consolidated", currency="PLN", unit="iso4217:PLN",
          fiscal_year=2025, period_type="FY", mapped=True):
    return {
        "slot_id": adj.build_slot_id(event_id, package_member, concept, attribution, basis, "flow", "reported", fiscal_year, period_type, currency),
        "event_id": event_id,
        "package_member": package_member,
        "concept_local": concept,
        "mapped": mapped,
        "attribution": attribution,
        "basis": basis,
        "window": "flow",
        "variant": "reported",
        "fiscal_year": fiscal_year,
        "period_type": period_type,
        "period_end": "2025-12-31",
        "period_start": "2025-01-01",
        "currency": currency,
        "unit": unit,
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


def _seal_all(out: Path, reader: str, index: dict, answer_for, round_num: int = 1) -> None:
    """Seals a complete answer set for every task in `index` -- `compare`
    now refuses to run over a partial set (amendment N)."""
    answers = {task_id: answer_for(task_id, meta) for task_id, meta in index.items()}
    answers_path = out / f"{reader}-answers.json"
    answers_path.write_text(json.dumps({"answers": answers}), encoding="utf-8")
    adj.seal(out, reader, answers_path, round_num=round_num)


def _write_occurrences(out: Path, occurrences: list[dict]) -> None:
    (out / "occurrences_v2.json").write_text(json.dumps(occurrences), encoding="utf-8")


def _write_decisions(out: Path, decisions: dict, name: str = "decisions.json") -> Path:
    path = out / name
    path.write_text(json.dumps({"decisions": decisions}), encoding="utf-8")
    return path


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
                self.assertNotIn("resolution_ref", task)
                # attribution IS slot identity, never a value (#331 PR-B) --
                # it must still be present.
                self.assertIn("attribution", task)
                self.assertIn("attribution_dimension", task)

    def test_task_carries_attribution_and_dimension_for_a_dimension_derived_slot(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Equity", "machine", "42", attribution="owners_of_parent")
            _write_gt(out, [slot])
            _write_occurrences(
                out,
                [
                    {
                        "occurrence_id": slot["contributing_occurrence_ids"][0],
                        "dimensions": [{"axis": "ifrs-full:ComponentsOfEquityAxis", "member": "ifrs-full:ParentMember"}],
                    }
                ],
            )
            adj.prepare(out, seed=1)
            task_id = json.loads((out / "adjudication" / "tasks.lock").read_text())["tasks"][0]["task_id"]
            task = json.loads((out / "adjudication" / "tasks" / f"{task_id}.json").read_text())
            self.assertEqual(task["attribution"], "owners_of_parent")
            self.assertEqual(
                task["attribution_dimension"],
                {"axis": "ifrs-full:ComponentsOfEquityAxis", "member": "ifrs-full:ParentMember"},
            )

    def test_task_carries_null_attribution_dimension_for_a_dimensionless_slot(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            _write_gt(out, [slot])
            _write_occurrences(out, [{"occurrence_id": slot["contributing_occurrence_ids"][0], "dimensions": []}])
            adj.prepare(out, seed=1)
            task_id = json.loads((out / "adjudication" / "tasks.lock").read_text())["tasks"][0]["task_id"]
            task = json.loads((out / "adjudication" / "tasks" / f"{task_id}.json").read_text())
            self.assertEqual(task["attribution"], "total")
            self.assertIsNone(task["attribution_dimension"])

    def test_prepare_leaves_attribution_dimension_null_and_counts_it_when_occurrences_disagree_on_member(self):
        # Do not invent one when contributing occurrences disagree.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Equity", "machine", "5", attribution="owners_of_parent")
            slot["contributing_occurrence_ids"] = [f"{slot['event_id']}#1", f"{slot['event_id']}#2"]
            _write_gt(out, [slot])
            _write_occurrences(
                out,
                [
                    {"occurrence_id": f"{slot['event_id']}#1", "dimensions": [{"axis": "ax", "member": "m1"}]},
                    {"occurrence_id": f"{slot['event_id']}#2", "dimensions": [{"axis": "ax", "member": "m2"}]},
                ],
            )
            result = adj.prepare(out, seed=1)
            self.assertEqual(result["ambiguous_attribution_dimension"], 1)
            task_id = json.loads((out / "adjudication" / "tasks.lock").read_text())["tasks"][0]["task_id"]
            task = json.loads((out / "adjudication" / "tasks" / f"{task_id}.json").read_text())
            self.assertIsNone(task["attribution_dimension"])

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
        # Amendment W / astra r2 finding 22: the DISPUTED VALUE is never
        # applied without evidence, but the contradicted machine label must
        # never simply STAY `machine` either -- it becomes `unverified`.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            _write_gt(out, [slot])
            adj.prepare(out, seed=5)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            _seal_all(out, "reader_b", index, lambda tid, meta: _full_answer(slot, value="999", evidence_anchor=None))
            adj.compare(out)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            self.assertEqual(gt["slots"][0]["verification"], "unverified")  # never stays `machine`
            self.assertEqual(gt["slots"][0]["value"], "42")  # the disputed value is never applied
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


class BindingTests(unittest.TestCase):
    """Amendment W / astra r2 finding 13: tasks.lock binds task files
    TOGETHER WITH tasks_index.json and the GT/occurrences snapshot they were
    prepared against."""

    def test_lock_records_index_and_gt_hashes(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            _write_gt(out, [_slot("Revenue", "machine", "42")])
            adj.prepare(out, seed=1)
            lock = json.loads((out / "adjudication" / "tasks.lock").read_text())
            self.assertIn("index_sha256", lock)
            self.assertIn("gt_sha256", lock)
            self.assertIsNotNone(lock["index_sha256"])
            self.assertIsNotNone(lock["gt_sha256"])

    def test_seal_refuses_when_the_index_was_tampered_with(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            _write_gt(out, [_slot("Revenue", "machine", "42")])
            adj.prepare(out, seed=1)
            index_path = out / "adjudication" / "tasks_index.json"
            index_path.write_text(index_path.read_text() + "  ")  # tamper after prepare

            index = json.loads(index_path.read_text())
            task_id = next(iter(index))
            answers_path = out / "answers.json"
            answers_path.write_text(json.dumps({"answers": {task_id: {"value": "42"}}}))
            with self.assertRaises(adj.BindingMismatchError):
                adj.seal(out, "reader_a", answers_path)

    def test_seal_refuses_when_ground_truth_changed_since_prepare(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            _write_gt(out, [slot])
            adj.prepare(out, seed=1)
            # Simulate a re-run of the labeler after tasks were prepared.
            _write_gt(out, [_slot("Revenue", "machine", "43")])

            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            task_id = next(iter(index))
            answers_path = out / "answers.json"
            answers_path.write_text(json.dumps({"answers": {task_id: {"value": "42"}}}))
            with self.assertRaises(adj.BindingMismatchError):
                adj.seal(out, "reader_a", answers_path)

    def test_compare_refuses_when_a_sealed_file_is_tampered_with_after_sealing(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            _write_gt(out, [slot])
            adj.prepare(out, seed=1)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            _seal_all(out, "reader_a", index, lambda tid, meta: _full_answer(slot))

            event_id = next(iter(index.values()))["event_id"]
            sealed_path = out / "adjudication" / event_id / "reader_a.json"
            payload = json.loads(sealed_path.read_text())
            payload["answers"][next(iter(payload["answers"]))]["value"] = "999999"  # tamper post-seal
            sealed_path.write_text(json.dumps(payload))

            with self.assertRaises(adj.SealHashMismatchError):
                adj.compare(out)

    def test_compare_refuses_when_seals_registry_disagrees_with_a_sealed_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            _write_gt(out, [slot])
            adj.prepare(out, seed=1)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            _seal_all(out, "reader_a", index, lambda tid, meta: _full_answer(slot))

            registry_path = out / "adjudication" / "seals.lock"
            registry = json.loads(registry_path.read_text())
            for key in registry:
                registry[key] = "0" * 64  # corrupt the independent registry
            registry_path.write_text(json.dumps(registry))

            with self.assertRaises(adj.SealHashMismatchError):
                adj.compare(out)


class AnswerValidationTests(unittest.TestCase):
    """Amendment W / astra r2 finding 22: an invalid answer fails the WHOLE
    compare transaction, no partial writes -- and never turns into the
    literal string "None" written into ground_truth_v2.json."""

    def test_unparseable_decimal_text_refuses_the_whole_compare_no_partial_writes(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            good_slot = _slot("Revenue", "machine", "42", event_id="iss_01/FY2025/pl/consolidated/v1")
            bad_slot = _slot("GrossProfit", "machine", "10", event_id="iss_01/FY2025/pl/consolidated/v1")
            _write_gt(out, [good_slot, bad_slot])
            adj.prepare(out, seed=1)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())

            def answer_for(tid, meta):
                slot = good_slot if meta["slot_id"] == good_slot["slot_id"] else bad_slot
                if slot is bad_slot:
                    return _full_answer(slot, value="not-a-number")  # garbage decimal text
                return _full_answer(slot)

            _seal_all(out, "reader_a", index, answer_for)
            with self.assertRaises(adj.InvalidAnswerError):
                adj.compare(out)

            # No partial writes: neither slot changed, resolutions.json never written.
            gt = json.loads((out / "ground_truth_v2.json").read_text())
            self.assertEqual({s["value"] for s in gt["slots"]}, {"42", "10"})
            self.assertFalse((out / "adjudication" / "resolutions.json").exists())

    def test_non_three_letter_currency_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            _write_gt(out, [slot])
            adj.prepare(out, seed=1)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            _seal_all(out, "reader_a", index, lambda tid, meta: _full_answer(slot, currency="PL"))
            with self.assertRaises(adj.InvalidAnswerError):
                adj.compare(out)

    def test_out_of_domain_basis_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            _write_gt(out, [slot])
            adj.prepare(out, seed=1)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            _seal_all(out, "reader_a", index, lambda tid, meta: _full_answer(slot, basis="combined"))
            with self.assertRaises(adj.InvalidAnswerError):
                adj.compare(out)


class VerifyEverythingLockedTests(unittest.TestCase):
    """Amendment AB / astra r3 finding 13: `compare` re-verifies EVERY task
    file `tasks.lock` locked and every seal `seals.lock` registered --
    `seals.lock`/`tasks.lock`, never a directory walk, are the source of
    truth for what to read."""

    def _prepared_and_sealed(self, out: Path, slot: dict):
        _write_gt(out, [slot])
        adj.prepare(out, seed=1)
        index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
        _seal_all(out, "reader_a", index, lambda tid, meta: _full_answer(slot))
        return index

    def test_deleting_a_registered_seal_aborts(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            index = self._prepared_and_sealed(out, slot)
            event_id = next(iter(index.values()))["event_id"]
            sealed_path = out / "adjudication" / event_id / "reader_a.json"
            self.assertTrue(sealed_path.exists())
            sealed_path.unlink()  # seals.lock still names it -- directory no longer has it

            with self.assertRaises(adj.SealHashMismatchError):
                adj.compare(out)
            self.assertFalse((out / "adjudication" / "resolutions.json").exists())

    def test_corrupting_a_locked_task_file_aborts(self):
        # Even a task file NO sealed answer references must still match its
        # locked hash -- corruption here was previously undetectable by
        # `compare` entirely (only answered tasks were ever re-checked).
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            index = self._prepared_and_sealed(out, slot)
            task_id = next(iter(index))
            task_path = out / "adjudication" / "tasks" / f"{task_id}.json"
            task_path.write_text(task_path.read_text() + "  ")  # tamper after prepare/seal

            with self.assertRaises(adj.TaskHashMismatchError):
                adj.compare(out)
            self.assertFalse((out / "adjudication" / "resolutions.json").exists())

    def test_an_unregistered_extra_seal_file_aborts(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            index = self._prepared_and_sealed(out, slot)
            event_id = next(iter(index.values()))["event_id"]
            # Hand-plant a seal-shaped file `seal()` never wrote and
            # `seals.lock` never registered.
            rogue = {"reader": "rogue", "event_id": event_id, "answers": {}, "sha256": "0" * 64}
            (out / "adjudication" / event_id / "rogue.json").write_text(json.dumps(rogue), encoding="utf-8")

            with self.assertRaises(adj.SealHashMismatchError):
                adj.compare(out)
            self.assertFalse((out / "adjudication" / "resolutions.json").exists())


class UnitDependentCurrencyValidationTests(unittest.TestCase):
    """Amendment AG / astra r3 finding 25: whether `currency` may be a code
    at all depends on the TASK's own `unit`."""

    def test_null_currency_accepted_on_a_non_monetary_shares_task(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("BasicEarningsLossPerShare", "machine", "1000", currency=None, unit="xbrli:shares")
            _write_gt(out, [slot])
            adj.prepare(out, seed=1)
            task_id = json.loads((out / "adjudication" / "tasks.lock").read_text())["tasks"][0]["task_id"]
            task = json.loads((out / "adjudication" / "tasks" / f"{task_id}.json").read_text())
            self.assertEqual(task["unit"], "xbrli:shares")

            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            _seal_all(out, "reader_a", index, lambda tid, meta: _full_answer(slot))  # currency defaults to None
            adj.compare(out)  # must not raise
            gt = json.loads((out / "ground_truth_v2.json").read_text())
            self.assertEqual(gt["slots"][0]["verification"], "second_read")

    def test_currency_code_rejected_on_a_non_monetary_shares_task(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("BasicEarningsLossPerShare", "machine", "1000", currency=None, unit="xbrli:shares")
            _write_gt(out, [slot])
            adj.prepare(out, seed=1)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            _seal_all(out, "reader_a", index, lambda tid, meta: _full_answer(slot, currency="PLN"))
            with self.assertRaises(adj.InvalidAnswerError):
                adj.compare(out)

    def test_null_currency_still_rejected_on_a_monetary_task(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "1000", currency="PLN", unit="iso4217:PLN")
            _write_gt(out, [slot])
            adj.prepare(out, seed=1)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            _seal_all(out, "reader_a", index, lambda tid, meta: _full_answer(slot, currency=None))
            with self.assertRaises(adj.InvalidAnswerError):
                adj.compare(out)


class RoundTests(unittest.TestCase):
    """#331 PR-B: `prepare --round <n>` (default 1) writes into its own
    directory -- round 1 keeps the pre-existing flat `adjudication/` layout
    (smaller change, every existing test above stays valid unmodified);
    round N>1 writes under `adjudication/round-N/`."""

    def test_round_1_keeps_the_flat_adjudication_layout(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            _write_gt(out, [_slot("Revenue", "machine", "42")])
            adj.prepare(out, seed=1, round_num=1)
            self.assertTrue((out / "adjudication" / "tasks.lock").exists())
            self.assertFalse((out / "adjudication" / "round-1").exists())

    def test_round_2_prepare_writes_under_its_own_subdirectory_round_1_lock_untouched(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            _write_gt(out, [_slot("Revenue", "machine", "42")])
            adj.prepare(out, seed=1)  # round 1
            round1_lock = (out / "adjudication" / "tasks.lock").read_text()

            result = adj.prepare(out, seed=2, round_num=2)
            self.assertGreaterEqual(result["tasks"], 1)
            self.assertTrue((out / "adjudication" / "round-2" / "tasks.lock").exists())
            self.assertEqual((out / "adjudication" / "tasks.lock").read_text(), round1_lock)

    def test_round_2_seal_and_compare_operate_only_on_round_2_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot1 = _slot("Revenue", "machine", "42")
            _write_gt(out, [slot1])
            adj.prepare(out, seed=1)  # round 1
            index1 = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            _seal_all(out, "reader_a", index1, lambda tid, meta: _full_answer(slot1))
            adj.compare(out)  # round 1 closed: adjudication/resolutions.json now exists

            slot2 = _slot("GrossProfit", "machine", "10")
            gt = json.loads((out / "ground_truth_v2.json").read_text())
            gt["slots"].append(slot2)
            (out / "ground_truth_v2.json").write_text(json.dumps(gt), encoding="utf-8")

            adj.prepare(out, seed=2, round_num=2)  # not refused despite round 1's lock existing
            index2 = json.loads((out / "adjudication" / "round-2" / "tasks_index.json").read_text())
            _seal_all(out, "reader_a", index2, lambda tid, meta: _full_answer(slot2), round_num=2)
            result = adj.compare(out, round_num=2)  # not blocked by round 1's closed sealing

            self.assertEqual(result["resolutions"], 1)
            self.assertTrue((out / "adjudication" / "round-2" / "resolutions.json").exists())
            round1_resolutions = json.loads((out / "adjudication" / "resolutions.json").read_text())
            self.assertEqual(len(round1_resolutions), 1)  # round 1's own resolutions, untouched


class RereadClassSelectionTests(unittest.TestCase):
    """Direct tests of the class-selection predicate -- LABELING.md step 6."""

    def test_attribution_dimension_class_selects_slots_with_a_dimensioned_contributing_occurrence(self):
        dim_slot = _slot("Equity", "machine", "5", attribution="owners_of_parent")
        plain_slot = _slot("Revenue", "machine", "1", event_id="iss_02/FY2025/pl/consolidated/v1")  # distinct
        # event_id -- same default event_id would collide on
        # contributing_occurrence_ids[0] ("<event_id>#1") and one slot's
        # occurrence entry would silently overwrite the other's.
        occurrences_by_id = {
            dim_slot["contributing_occurrence_ids"][0]: {"dimensions": [{"axis": "ax", "member": "m"}]},
            plain_slot["contributing_occurrence_ids"][0]: {"dimensions": []},
        }
        selected = adj._slots_in_reread_class("attribution_dimension", [dim_slot, plain_slot], occurrences_by_id)
        self.assertEqual([s["slot_id"] for s in selected], [dim_slot["slot_id"]])

    def test_attribution_name_class_selects_unmapped_dimensionless_name_suffix_slots_only(self):
        # Distinct event_id per slot: `_slot()`'s default
        # contributing_occurrence_ids is "<event_id>#1" -- sharing an
        # event_id would collide two slots onto the same occurrence key.
        name_slot = _slot(
            "ComprehensiveIncomeAttributableToOwnersOfParent", "machine", "9",
            attribution="owners_of_parent", mapped=False, event_id="iss_01/FY2025/pl/consolidated/v1",
        )
        mapped_name_slot = _slot(  # mapped -- stays out, key map already covers it
            "ProfitLossAttributableToOwnersOfParent", "machine", "9",
            attribution="owners_of_parent", mapped=True, event_id="iss_02/FY2025/pl/consolidated/v1",
        )
        dimensioned_unmapped_slot = _slot(  # dimensioned -- excluded (attribution_dimension's territory, not this class's)
            "SomeUnmappedAttributableToNonControllingInterests", "machine", "2",
            attribution="nci", mapped=False, event_id="iss_03/FY2025/pl/consolidated/v1",
        )
        plain_unmapped_slot = _slot(  # no attribution-bearing suffix
            "PlainConcept", "machine", "1", attribution="total", mapped=False, event_id="iss_04/FY2025/pl/consolidated/v1",
        )
        occurrences_by_id = {
            name_slot["contributing_occurrence_ids"][0]: {"dimensions": []},
            mapped_name_slot["contributing_occurrence_ids"][0]: {"dimensions": []},
            dimensioned_unmapped_slot["contributing_occurrence_ids"][0]: {"dimensions": [{"axis": "ax", "member": "m"}]},
            plain_unmapped_slot["contributing_occurrence_ids"][0]: {"dimensions": []},
        }
        slots = [name_slot, mapped_name_slot, dimensioned_unmapped_slot, plain_unmapped_slot]
        selected = adj._slots_in_reread_class("attribution_name", slots, occurrences_by_id)
        self.assertEqual([s["slot_id"] for s in selected], [name_slot["slot_id"]])

    def test_unknown_reread_class_raises(self):
        with self.assertRaises(ValueError):
            adj._slots_in_reread_class("bogus", [], {})


class RereadClassPrepareTests(unittest.TestCase):
    def test_reread_class_adds_matching_slots_on_top_of_the_base_sample_deduped(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slots = [_slot(f"Concept{i}", "machine", "1") for i in range(20)]
            name_slot = _slot("ComprehensiveIncomeAttributableToOwnersOfParent", "machine", "9", attribution="owners_of_parent")
            name_slot["mapped"] = False
            slots.append(name_slot)
            _write_gt(out, slots)
            _write_occurrences(out, [{"occurrence_id": s["contributing_occurrence_ids"][0], "dimensions": []} for s in slots])

            result = adj.prepare(out, seed=1, round_num=2, reread_class="attribution_name")
            index = json.loads((out / "adjudication" / "round-2" / "tasks_index.json").read_text())
            selected_slot_ids = {m["slot_id"] for m in index.values()}
            self.assertIn(name_slot["slot_id"], selected_slot_ids)
            self.assertEqual(len(selected_slot_ids), result["tasks"])  # dedup: no slot appears twice

    def test_several_reread_classes_are_added_together_deduped(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slots = [_slot(f"Concept{i}", "machine", "1") for i in range(20)]
            name_slot = _slot("ComprehensiveIncomeAttributableToOwnersOfParent", "machine", "9", attribution="owners_of_parent")
            name_slot["mapped"] = False
            dim_slot = _slot("Equity", "machine", "7", attribution="owners_of_parent", event_id="iss_09/FY2025/pl/consolidated/v1")
            slots += [name_slot, dim_slot]
            _write_gt(out, slots)
            occurrences = [{"occurrence_id": s["contributing_occurrence_ids"][0], "dimensions": []} for s in slots if s is not dim_slot]
            occurrences.append({"occurrence_id": dim_slot["contributing_occurrence_ids"][0], "dimensions": [{"axis": "ax", "member": "m"}]})
            _write_occurrences(out, occurrences)

            result = adj.prepare(out, seed=1, round_num=2, reread_class=["attribution_dimension", "attribution_name"])
            index = json.loads((out / "adjudication" / "round-2" / "tasks_index.json").read_text())
            selected_slot_ids = {m["slot_id"] for m in index.values()}
            self.assertTrue({name_slot["slot_id"], dim_slot["slot_id"]} <= selected_slot_ids)
            self.assertEqual(len(selected_slot_ids), result["tasks"])

    def test_reread_class_none_by_default_leaves_task_count_unaffected(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slots = [_slot(f"Concept{i}", "machine", "1") for i in range(20)]
            _write_gt(out, slots)
            result = adj.prepare(out, seed=1)
            self.assertEqual(result["class_reread"], 0)


class DecisionsTests(unittest.TestCase):
    """#331 PR-B: an owner decisions file overrides one genuine disagreement
    the other way, with a reason -- for a task the reader answered
    differently from the machine slot only; agreeing/unsettled/unknown
    tasks refuse the whole compare, no writes."""

    def _prepared_and_sealed_disagreement(self, out: Path, **answer_overrides):
        slot = _slot("Revenue", "machine", "42")
        _write_gt(out, [slot])
        adj.prepare(out, seed=5)
        index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
        task_id = next(iter(index))
        _seal_all(out, "reader_b", index, lambda tid, meta: _full_answer(slot, value="999", **answer_overrides))
        return slot, task_id

    def test_keep_machine_leaves_value_and_id_untouched_and_records_the_reason(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot, task_id = self._prepared_and_sealed_disagreement(out)
            old_slot_id = slot["slot_id"]
            decisions_path = _write_decisions(
                out, {task_id: {"keep": "machine", "reason": "readers answered a neighbouring fact", "decided_by": "owner"}}
            )

            adj.compare(out, decisions_path=decisions_path)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            new_slot = gt["slots"][0]
            self.assertEqual(new_slot["verification"], "adjudicated")
            self.assertEqual(new_slot["value"], "42")  # untouched -- the machine value stands
            self.assertEqual(new_slot["slot_id"], old_slot_id)  # untouched -- no re-key
            self.assertEqual(
                new_slot["resolution_ref"],
                {"decision": "keep_machine", "reason": "readers answered a neighbouring fact", "decided_by": "owner"},
            )
            resolutions = json.loads((out / "adjudication" / "resolutions.json").read_text())
            self.assertEqual(resolutions[0]["outcome"], "adjudicated_keep_machine")
            self.assertEqual(resolutions[0]["decisions_sha256"], hashlib.sha256(decisions_path.read_bytes()).hexdigest())

    def test_keep_reader_applies_as_before(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot, task_id = self._prepared_and_sealed_disagreement(out, evidence_anchor=None)
            decisions_path = _write_decisions(
                out, {task_id: {"keep": "reader", "reason": "confirmed on a second filing page", "decided_by": "owner"}}
            )

            adj.compare(out, decisions_path=decisions_path)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            new_slot = gt["slots"][0]
            self.assertEqual(new_slot["verification"], "adjudicated")
            self.assertEqual(new_slot["value"], "999")
            resolutions = json.loads((out / "adjudication" / "resolutions.json").read_text())
            self.assertEqual(resolutions[0]["outcome"], "adjudicated")
            self.assertEqual(resolutions[0]["decisions_sha256"], hashlib.sha256(decisions_path.read_bytes()).hexdigest())

    def test_decision_on_an_agreeing_task_aborts_with_no_writes(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            _write_gt(out, [slot])
            adj.prepare(out, seed=5)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            task_id = next(iter(index))
            _seal_all(out, "reader_b", index, lambda tid, meta: _full_answer(slot))  # agrees
            decisions_path = _write_decisions(out, {task_id: {"keep": "machine", "reason": "n/a", "decided_by": "owner"}})

            with self.assertRaises(adj.InvalidDecisionError):
                adj.compare(out, decisions_path=decisions_path)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            self.assertEqual(gt["slots"][0]["value"], "42")
            self.assertFalse((out / "adjudication" / "resolutions.json").exists())

    def test_decision_on_an_unsettled_task_aborts_with_no_writes(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot = _slot("Revenue", "machine", "42")
            _write_gt(out, [slot])
            adj.prepare(out, seed=5)
            index = json.loads((out / "adjudication" / "tasks_index.json").read_text())
            task_id = next(iter(index))
            _seal_all(out, "reader_b", index, lambda tid, meta: {"unverified": True, "reason": "not legible"})
            decisions_path = _write_decisions(out, {task_id: {"keep": "machine", "reason": "n/a", "decided_by": "owner"}})

            with self.assertRaises(adj.InvalidDecisionError):
                adj.compare(out, decisions_path=decisions_path)

            self.assertFalse((out / "adjudication" / "resolutions.json").exists())

    def test_decision_naming_an_unknown_task_id_aborts_with_no_writes(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot, _task_id = self._prepared_and_sealed_disagreement(out)
            decisions_path = _write_decisions(out, {"task_9999": {"keep": "machine", "reason": "n/a", "decided_by": "owner"}})

            with self.assertRaises(adj.InvalidDecisionError):
                adj.compare(out, decisions_path=decisions_path)

            gt = json.loads((out / "ground_truth_v2.json").read_text())
            self.assertEqual(gt["slots"][0]["value"], "42")
            self.assertFalse((out / "adjudication" / "resolutions.json").exists())

    def test_decision_with_out_of_domain_keep_aborts(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot, task_id = self._prepared_and_sealed_disagreement(out)
            decisions_path = _write_decisions(out, {task_id: {"keep": "bogus", "reason": "n/a", "decided_by": "owner"}})

            with self.assertRaises(adj.InvalidDecisionError):
                adj.compare(out, decisions_path=decisions_path)
            self.assertFalse((out / "adjudication" / "resolutions.json").exists())

    def test_decision_with_empty_reason_aborts(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot, task_id = self._prepared_and_sealed_disagreement(out)
            decisions_path = _write_decisions(out, {task_id: {"keep": "machine", "reason": "   ", "decided_by": "owner"}})

            with self.assertRaises(adj.InvalidDecisionError):
                adj.compare(out, decisions_path=decisions_path)
            self.assertFalse((out / "adjudication" / "resolutions.json").exists())

    def test_no_decisions_file_is_todays_behavior(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            slot, task_id = self._prepared_and_sealed_disagreement(out)  # differs, with evidence

            result = adj.compare(out)  # no decisions_path at all

            self.assertEqual(result["resolutions"], 1)
            gt = json.loads((out / "ground_truth_v2.json").read_text())
            self.assertEqual(gt["slots"][0]["verification"], "adjudicated")
            self.assertEqual(gt["slots"][0]["value"], "999")
            resolutions = json.loads((out / "adjudication" / "resolutions.json").read_text())
            self.assertEqual(resolutions[0]["outcome"], "adjudicated")
            self.assertNotIn("decisions_sha256", resolutions[0])


if __name__ == "__main__":
    unittest.main()
