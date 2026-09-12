#!/usr/bin/env python3
"""Blinded adjudication for ESEF measurement v2 (#331 PR-A, ADR 0112,
`LABELING.md`). Stdlib only.

Three subcommands:
  prepare  builds indistinguishable review tasks: every machine-flagged
           disagreement (an `unverified` ground-truth slot) plus a seeded,
           frozen 10% sample of agreements (`machine` slots) -- minimum 10
           tasks total, or all of them if fewer exist. A task carries only
           the filing reference (event_id), concept and period -- never a
           candidate value or which selection rule produced it, so a
           disagreement task and an agreement-sample task are identical in
           shape and mixed in random order.
  seal     stores one reader's answers for one event
           (`adjudication/<event_id>/<reader>.json`, hash + timestamp).
           Refuses once `compare` has run -- sealed reads are then closed.
  compare  mechanically compares sealed answers against the machine slots,
           writes `adjudication/resolutions.json` and updates
           `ground_truth_v2.json` verification statuses: the reader's value
           matches the machine's -> `second_read`; it differs -> `adjudicated`
           (the reader's filing-anchored read settles it, on either a machine
           `unverified` disagreement or a plain `machine` slot -- a
           contradicted agreement-sample slot is exactly the systematic-error
           signal LABELING.md step 6 asks the owner to act on); the reader
           could not settle it from the filing, or multiple readers
           disagreed with each other -> stays/becomes `unverified`. The
           production app's extraction is never consulted here and the only
           values ever written are the reader's own.

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
from pathlib import Path

AGREEMENT_SAMPLE_FRACTION = 0.10
MIN_TASKS = 10
PROTOCOL_VERSION = 1


class SealingClosedError(RuntimeError):
    """Raised when `seal` is attempted after `compare` has already run."""


def _adjudication_dir(esef_v2_dir: Path) -> Path:
    return esef_v2_dir / "adjudication"


def prepare(esef_v2_dir: Path, seed: int) -> dict:
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

    adjudication_dir = _adjudication_dir(esef_v2_dir)
    tasks_dir = adjudication_dir / "tasks"
    tasks_dir.mkdir(parents=True, exist_ok=True)

    lock: list[dict] = []
    index: dict[str, dict] = {}
    for i, (slot, kind) in enumerate(chosen, start=1):
        task_id = f"task_{i:04d}"
        task = {
            "task_id": task_id,
            "event_id": slot["event_id"],
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


def seal(esef_v2_dir: Path, reader: str, answers_path: Path) -> list[Path]:
    adjudication_dir = _adjudication_dir(esef_v2_dir)
    if (adjudication_dir / "resolutions.json").exists():
        raise SealingClosedError("compare has already run for this corpus; sealing is closed")

    answers = json.loads(answers_path.read_text(encoding="utf-8"))
    index = json.loads((adjudication_dir / "tasks_index.json").read_text(encoding="utf-8"))

    by_event: dict[str, dict] = {}
    for task_id, answer in answers.get("answers", {}).items():
        meta = index.get(task_id)
        if meta is None:
            raise ValueError(f"unknown task_id {task_id!r} -- not in tasks_index.json")
        by_event.setdefault(meta["event_id"], {})[task_id] = answer

    timestamp = datetime.now(timezone.utc).isoformat()
    sealed_paths = []
    for event_id, event_answers in by_event.items():
        event_dir = adjudication_dir / event_id
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
        dest = event_dir / f"{reader}.json"
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


def compare(esef_v2_dir: Path) -> dict:
    adjudication_dir = _adjudication_dir(esef_v2_dir)
    index = json.loads((adjudication_dir / "tasks_index.json").read_text(encoding="utf-8"))
    gt = json.loads((esef_v2_dir / "ground_truth_v2.json").read_text(encoding="utf-8"))
    slots_by_id = {s["slot_id"]: s for s in gt["slots"]}

    answers_by_task: dict[str, list[tuple[str, dict]]] = {}
    for payload in _iter_sealed_files(adjudication_dir):
        for task_id, answer in payload["answers"].items():
            answers_by_task.setdefault(task_id, []).append((payload["reader"], answer))

    resolutions = []
    for task_id, meta in sorted(index.items()):
        readers = answers_by_task.get(task_id)
        if not readers:
            continue
        slot = slots_by_id.get(meta["slot_id"])
        if slot is None:
            continue

        original_value = slot["value"]
        evidence = next((a.get("evidence_anchor") for _r, a in readers if a.get("evidence_anchor")), None)
        any_unsettled = any(a.get("unverified") for _r, a in readers)
        reader_values = {a["value"] for _r, a in readers if not a.get("unverified")}

        # Three outcomes only, uniform regardless of why the task was picked
        # (a disagreement or an agreement-sample check are indistinguishable
        # by design -- LABELING.md step 5): the reader could not settle it
        # from the filing, or multiple readers disagreed with each other ->
        # unsettled; a single definite read matching the machine's value ->
        # agree; a single definite read that DIFFERS -> adjudicated (the
        # reader's filing-anchored value settles it, whether it resolves a
        # machine `unverified` conflict or contradicts a plain `machine`
        # slot -- the latter is exactly the systematic-error signal step 6
        # asks the owner to act on). The production app's value never enters.
        if any_unsettled or len(reader_values) != 1:
            outcome, new_verification = "unsettled", "unverified"
        else:
            reader_value = next(iter(reader_values))
            if reader_value == original_value:
                outcome, new_verification = "agree", "second_read"
            else:
                outcome, new_verification = "adjudicated", "adjudicated"
                slot["value"] = reader_value
                slot["resolution_ref"] = None

        slot["verification"] = new_verification
        resolutions.append(
            {
                "task_id": task_id,
                "slot_id": meta["slot_id"],
                "event_id": meta["event_id"],
                "task_kind": meta["task_kind"],
                "outcome": outcome,
                "reader_values": sorted(reader_values),
                "machine_value": original_value,
                "evidence_anchor": evidence,
                "readers": sorted(r for r, _ in readers),
            }
        )

    (adjudication_dir / "resolutions.json").write_text(
        json.dumps(resolutions, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    (esef_v2_dir / "ground_truth_v2.json").write_text(
        json.dumps(gt, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    return {"resolutions": len(resolutions)}


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
        result = prepare(esef_v2_dir, args.seed)
        print(f"adjudicate prepare: {result['tasks']} tasks ({result['disagreements']} disagreements, "
              f"{result['agreement_sample']} agreement-sample)")
    elif args.command == "seal":
        try:
            sealed = seal(esef_v2_dir, args.reader, args.answers)
        except SealingClosedError as exc:
            print(f"adjudicate seal: refused -- {exc}")
            return 1
        print(f"adjudicate seal: sealed {len(sealed)} event file(s) for reader {args.reader!r}")
    elif args.command == "compare":
        result = compare(esef_v2_dir)
        print(f"adjudicate compare: {result['resolutions']} resolutions")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
