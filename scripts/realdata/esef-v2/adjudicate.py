#!/usr/bin/env python3
"""Blinded adjudication for ESEF measurement v2 (#331 PR-A, ADR 0112,
`LABELING.md`, amendment 1 N). Stdlib only.

Three subcommands:
  prepare  builds indistinguishable review tasks: every machine-flagged
           disagreement (an `unverified` ground-truth slot) plus a seeded,
           frozen 10% sample of agreements (`machine` slots) -- minimum 10
           tasks total, or all of them if fewer exist. A task carries only
           the filing reference (event_id, package_member), concept and
           period -- never a candidate value or which selection rule
           produced it. Refuses to overwrite an existing task set (amendment
           N: task/index/GT bindings are frozen by hash once prepared).
  seal     stores one reader's answers for one event
           (`adjudication/<event_id>/<reader>.json`, hash + timestamp).
           Refuses an existing reader seal for that event, refuses once
           `compare` has run, and validates every answered task's hash
           against `tasks.lock` (a task file that changed since `prepare`
           is refused, never silently accepted).
  compare  mechanically compares the FULL normalized answer (value as
           Decimal, currency, basis, attribution, fiscal period) against
           the machine slot -- not value alone. Refuses unless every
           prepared task has a sealed answer. Outcomes: agree -> `second_read`;
           differs, with an evidence anchor -> `adjudicated` (atomically
           re-keyed if the correction changes the slot's identity, e.g. a
           corrected attribution); differs with NO evidence anchor ->
           refused (never silently applied); unsettled, or multiple readers
           disagreeing -> stays/becomes `unverified`. The production app's
           extraction is never consulted; the only values ever written are
           the reader's own.

Usage:
    python3 adjudicate.py prepare --esef-v2-dir <dir> --seed <int>
    python3 adjudicate.py seal --esef-v2-dir <dir> <reader> <answers.json>
    python3 adjudicate.py compare --esef-v2-dir <dir>
"""
from __future__ import annotations

import argparse
import hashlib
import json
import random
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
from pathlib import Path

from label_esef_v2 import build_slot_id

AGREEMENT_SAMPLE_FRACTION = 0.10
MIN_TASKS = 10
PROTOCOL_VERSION = 1
NORMALIZED_ANSWER_FIELDS = ("value", "currency", "basis", "attribution", "fiscal_year", "period_type")


class SealingClosedError(RuntimeError):
    """Raised when `seal` is attempted after `compare` has already run."""


class PrepareExistsError(RuntimeError):
    """Raised when `prepare` would overwrite an existing, frozen task set."""


class ResealError(RuntimeError):
    """Raised when `seal` would overwrite an existing reader seal."""


class TaskHashMismatchError(RuntimeError):
    """Raised when a task file's current hash no longer matches tasks.lock."""


class IncompleteAdjudicationError(RuntimeError):
    """Raised when `compare` runs while a prepared task has no sealed answer."""


def _adjudication_dir(esef_v2_dir: Path) -> Path:
    return esef_v2_dir / "adjudication"


def prepare(esef_v2_dir: Path, seed: int) -> dict:
    adjudication_dir = _adjudication_dir(esef_v2_dir)
    if (adjudication_dir / "tasks.lock").exists():
        raise PrepareExistsError(
            f"{adjudication_dir / 'tasks.lock'} already exists -- task/index/GT bindings are frozen by hash "
            "once prepared (amendment N); re-preparing would associate old sealed answers with new task ids."
        )

    gt = json.loads((esef_v2_dir / "ground_truth_v2.json").read_text(encoding="utf-8"))
    disagreements = [s for s in gt["slots"] if s["verification"] == "unverified"]
    agreements = [s for s in gt["slots"] if s["verification"] == "machine"]

    rng = random.Random(seed)
    shuffled_agreements = agreements[:]
    rng.shuffle(shuffled_agreements)
    sample_size = max(1, round(len(agreements) * AGREEMENT_SAMPLE_FRACTION)) if agreements else 0
    sample = shuffled_agreements[:sample_size]
    sample_ids = {id(s) for s in sample}

    chosen = [(s, "disagreement") for s in disagreements] + [(s, "agreement_sample") for s in sample]
    if len(chosen) < MIN_TASKS:
        remaining_pool = [s for s in shuffled_agreements if id(s) not in sample_ids]
        needed = min(MIN_TASKS - len(chosen), len(remaining_pool))
        chosen += [(s, "agreement_sample") for s in remaining_pool[:needed]]

    rng.shuffle(chosen)  # disagreement and agreement-sample tasks are indistinguishable, in random order

    tasks_dir = adjudication_dir / "tasks"
    tasks_dir.mkdir(parents=True, exist_ok=True)

    lock: list[dict] = []
    index: dict[str, dict] = {}
    for i, (slot, kind) in enumerate(chosen, start=1):
        task_id = f"task_{i:04d}"
        task = {
            "task_id": task_id,
            "event_id": slot["event_id"],
            "package_member": slot.get("package_member"),
            "concept_local": slot["concept_local"],
            "basis": slot["basis"],
            "window": slot["window"],
            "fiscal_year": slot["fiscal_year"],
            "period_type": slot["period_type"],
        }
        task_bytes = json.dumps(task, indent=2, ensure_ascii=False, sort_keys=True).encode("utf-8") + b"\n"
        (tasks_dir / f"{task_id}.json").write_bytes(task_bytes)
        lock.append({"task_id": task_id, "sha256": hashlib.sha256(task_bytes).hexdigest()})
        index[task_id] = {"slot_id": slot["slot_id"], "event_id": slot["event_id"], "task_kind": kind}

    (adjudication_dir / "tasks.lock").write_text(json.dumps(lock, indent=2) + "\n", encoding="utf-8")
    (adjudication_dir / "tasks_index.json").write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
    return {"tasks": len(chosen), "disagreements": len(disagreements), "agreement_sample": len(sample)}


def _load_lock(adjudication_dir: Path) -> dict[str, str]:
    lock = json.loads((adjudication_dir / "tasks.lock").read_text(encoding="utf-8"))
    return {entry["task_id"]: entry["sha256"] for entry in lock}


def seal(esef_v2_dir: Path, reader: str, answers_path: Path) -> list[Path]:
    adjudication_dir = _adjudication_dir(esef_v2_dir)
    if (adjudication_dir / "resolutions.json").exists():
        raise SealingClosedError("compare has already run for this corpus; sealing is closed")

    answers = json.loads(answers_path.read_text(encoding="utf-8"))
    index = json.loads((adjudication_dir / "tasks_index.json").read_text(encoding="utf-8"))
    lock_by_task = _load_lock(adjudication_dir)

    by_event: dict[str, dict] = {}
    for task_id, answer in answers.get("answers", {}).items():
        meta = index.get(task_id)
        if meta is None:
            raise ValueError(f"unknown task_id {task_id!r} -- not in tasks_index.json")
        task_path = adjudication_dir / "tasks" / f"{task_id}.json"
        current_hash = hashlib.sha256(task_path.read_bytes()).hexdigest()
        if lock_by_task.get(task_id) != current_hash:
            raise TaskHashMismatchError(
                f"{task_path} no longer matches tasks.lock -- refusing to seal an answer against a changed task"
            )
        by_event.setdefault(meta["event_id"], {})[task_id] = answer

    timestamp = datetime.now(timezone.utc).isoformat()
    sealed_paths = []
    for event_id, event_answers in by_event.items():
        event_dir = adjudication_dir / event_id
        dest = event_dir / f"{reader}.json"
        if dest.exists():
            raise ResealError(f"{dest} already sealed for reader {reader!r} -- resealing is refused (amendment N)")
        event_dir.mkdir(parents=True, exist_ok=True)
        payload = {
            "reader": reader,
            "event_id": event_id,
            "sealed_at": timestamp,
            "protocol_version": PROTOCOL_VERSION,
            "reader_meta": answers.get("reader_meta"),  # model/prompt hash sidecar, if supplied
            "answers": event_answers,
        }
        body = json.dumps(payload, indent=2, ensure_ascii=False, sort_keys=True).encode("utf-8")
        payload["sha256"] = hashlib.sha256(body).hexdigest()
        dest.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        sealed_paths.append(dest)
    return sealed_paths


def _iter_sealed_files(adjudication_dir: Path):
    for path in adjudication_dir.rglob("*.json"):
        if path.parent == adjudication_dir or path.parent.name == "tasks":
            continue  # tasks.lock / tasks_index.json / resolutions.json / adjudication/tasks/*
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            continue
        if isinstance(payload, dict) and "answers" in payload and "reader" in payload:
            yield payload


def _decimal_or_none(value) -> Decimal | None:
    try:
        return Decimal(str(value))
    except (InvalidOperation, TypeError):
        return None


def _slot_normalized_tuple(slot: dict) -> tuple:
    return (
        _decimal_or_none(slot["value"]),
        slot["currency"],
        slot["basis"],
        slot["attribution"],
        slot["fiscal_year"],
        slot["period_type"],
    )


def _answer_normalized_tuple(answer: dict) -> tuple | None:
    """The reader's own full, independently-derived reading -- amendment N /
    astra r1 finding 12: comparing value alone let a same-number,
    different-currency answer silently pass as `second_read`. Returns None
    if any required field is missing (a malformed/incomplete definite
    answer, never silently defaulted from the slot under test)."""
    if any(field not in answer for field in NORMALIZED_ANSWER_FIELDS):
        return None
    return (
        _decimal_or_none(answer["value"]),
        answer["currency"],
        answer["basis"],
        answer["attribution"],
        answer["fiscal_year"],
        answer["period_type"],
    )


def compare(esef_v2_dir: Path) -> dict:
    adjudication_dir = _adjudication_dir(esef_v2_dir)
    index = json.loads((adjudication_dir / "tasks_index.json").read_text(encoding="utf-8"))
    gt = json.loads((esef_v2_dir / "ground_truth_v2.json").read_text(encoding="utf-8"))
    slots_by_id = {s["slot_id"]: s for s in gt["slots"]}

    answers_by_task: dict[str, list[tuple[str, dict]]] = {}
    for payload in _iter_sealed_files(adjudication_dir):
        for task_id, answer in payload["answers"].items():
            answers_by_task.setdefault(task_id, []).append((payload["reader"], answer))

    missing = sorted(task_id for task_id in index if task_id not in answers_by_task)
    if missing:
        raise IncompleteAdjudicationError(
            f"{len(missing)} prepared task(s) have no sealed answer yet (amendment N): {', '.join(missing)}"
        )

    resolutions = []
    for task_id, meta in sorted(index.items()):
        readers = answers_by_task[task_id]
        slot = slots_by_id.get(meta["slot_id"])
        if slot is None:
            continue

        slot_tuple = _slot_normalized_tuple(slot)
        evidence = next((a.get("evidence_anchor") for _r, a in readers if a.get("evidence_anchor")), None)
        any_unsettled = any(a.get("unverified") for _r, a in readers)
        answer_tuples = {_answer_normalized_tuple(a) for _r, a in readers if not a.get("unverified")}

        # Uniform outcomes regardless of why the task was picked (a
        # disagreement or an agreement-sample check are indistinguishable by
        # design -- LABELING.md step 5): the reader could not settle it, an
        # answer was malformed/incomplete, or multiple readers disagreed
        # with each other -> unsettled; a single complete, matching read ->
        # agree; a single complete, DIFFERING read WITH an evidence anchor ->
        # adjudicated (settles a machine conflict or flags a contradicted
        # agreement-sample slot as a systematic-error signal, LABELING.md
        # step 6); a differing read with NO evidence anchor is refused, not
        # silently applied (amendment N).
        if any_unsettled or len(answer_tuples) != 1 or None in answer_tuples:
            outcome, new_verification = "unsettled", "unverified"
        else:
            answer_tuple = next(iter(answer_tuples))
            if answer_tuple == slot_tuple:
                outcome, new_verification = "agree", "second_read"
            elif evidence is None:
                outcome, new_verification = "rejected_missing_evidence", slot["verification"]
            else:
                outcome, new_verification = "adjudicated", "adjudicated"
                _apply_adjudicated_answer(slot, answer_tuple)

        slot["verification"] = new_verification
        resolutions.append(
            {
                "task_id": task_id,
                "slot_id": slot["slot_id"],  # re-read after a possible re-key
                "event_id": meta["event_id"],
                "task_kind": meta["task_kind"],
                "outcome": outcome,
                "evidence_anchor": evidence,
                "readers": sorted(r for r, _ in readers),
            }
        )

    _assert_unique_slot_ids(gt["slots"])
    (adjudication_dir / "resolutions.json").write_text(
        json.dumps(resolutions, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    (esef_v2_dir / "ground_truth_v2.json").write_text(
        json.dumps(gt, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    return {"resolutions": len(resolutions)}


def _apply_adjudicated_answer(slot: dict, answer_tuple: tuple) -> None:
    """Applies the reader's corrected fields to `slot` IN PLACE (same dict
    object, same membership in gt["slots"]) and re-keys `slot_id` atomically
    if the correction changed any field the id is built from -- amendment N:
    "update/re-key corrected dimensions atomically". Never touches
    `contributing_occurrence_ids`: the underlying raw evidence is unchanged,
    only its interpretation is."""
    value, currency, basis, attribution, fiscal_year, period_type = answer_tuple
    slot["value"] = str(value)
    slot["currency"] = currency
    slot["basis"] = basis
    slot["attribution"] = attribution
    slot["fiscal_year"] = fiscal_year
    slot["period_type"] = period_type
    slot["resolution_ref"] = None
    slot["slot_id"] = build_slot_id(
        slot["event_id"],
        slot.get("package_member"),
        slot["concept_local"],
        attribution,
        basis,
        slot["window"],
        slot["variant"],
        fiscal_year,
        period_type,
        currency,
    )


def _assert_unique_slot_ids(slots: list[dict]) -> None:
    seen = set()
    for slot in slots:
        if slot["slot_id"] in seen:
            raise ValueError(f"duplicate slot_id {slot['slot_id']!r} after adjudication re-key (astra r1 finding 5)")
        seen.add(slot["slot_id"])


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--esef-v2-dir", required=True)
    sub = parser.add_subparsers(dest="command", required=True)

    p_prepare = sub.add_parser("prepare")
    p_prepare.add_argument("--seed", type=int, required=True)

    p_seal = sub.add_parser("seal")
    p_seal.add_argument("reader")
    p_seal.add_argument("answers", type=Path)

    sub.add_parser("compare")

    args = parser.parse_args(argv)
    esef_v2_dir = Path(args.esef_v2_dir)

    if args.command == "prepare":
        try:
            result = prepare(esef_v2_dir, args.seed)
        except PrepareExistsError as exc:
            print(f"adjudicate prepare: refused -- {exc}")
            return 1
        print(f"adjudicate prepare: {result['tasks']} tasks ({result['disagreements']} disagreements, "
              f"{result['agreement_sample']} agreement-sample)")
    elif args.command == "seal":
        try:
            sealed = seal(esef_v2_dir, args.reader, args.answers)
        except (SealingClosedError, ResealError, TaskHashMismatchError) as exc:
            print(f"adjudicate seal: refused -- {exc}")
            return 1
        print(f"adjudicate seal: sealed {len(sealed)} event file(s) for reader {args.reader!r}")
    elif args.command == "compare":
        try:
            result = compare(esef_v2_dir)
        except IncompleteAdjudicationError as exc:
            print(f"adjudicate compare: refused -- {exc}")
            return 1
        print(f"adjudicate compare: {result['resolutions']} resolutions")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
