#!/usr/bin/env python3
"""Blinded adjudication for ESEF measurement v2 (#331 PR-A, ADR 0112,
`LABELING.md`, amendment 2 W). Stdlib only.

Three subcommands:
  prepare  builds indistinguishable review tasks: every machine-flagged
           disagreement (an `unverified` ground-truth slot) plus a seeded,
           frozen 10% sample of agreements (`machine` slots) -- minimum 10
           tasks total, or all of them if fewer exist. A task carries only
           the filing reference (event_id, package_member), concept and
           period -- never a candidate value or which selection rule
           produced it. `tasks.lock` binds the task file hashes TOGETHER
           WITH the current hash of `tasks_index.json`, `ground_truth_v2.json`
           and `occurrences_v2.json` (amendment W) -- refuses to overwrite an
           existing task set.
  seal     stores one reader's answers for one event
           (`adjudication/<event_id>/<reader>.json`, hash + timestamp).
           Re-verifies the FULL binding (index/GT/occurrences hashes, not
           just the answered task files) before sealing, refuses once
           `compare` has run, refuses an existing reader seal, and records
           the seal's own hash in `adjudication/seals.lock` (a registry
           `compare` re-checks independently of each file's self-reported
           hash).
  compare  re-verifies every binding and every seal hash first; then
           validates EVERY sealed answer's domain (finite Decimal value,
           3-letter currency, basis/attribution/period_type in the known
           enum, integer fiscal_year) -- a single invalid answer anywhere
           fails the WHOLE transaction with NO writes at all (amendment W).
           Only once every answer is known-valid does it compare the FULL
           normalized answer against the machine slot. Refuses unless every
           prepared task has a sealed answer. Outcomes: agree ->
           `second_read`; differs, with an evidence anchor -> `adjudicated`
           (atomically re-keyed if the correction changes the slot's
           identity); differs with NO evidence anchor -> `unverified`
           (amendment W: a contradicted machine label never stays
           `machine`), the disputed value never applied; unsettled, or
           multiple readers disagreeing -> stays/becomes `unverified`. The
           production app's extraction is never consulted; the only values
           ever written are a reader's own, and only after validation.

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

from label_esef_v2 import build_slot_id, currency_from_unit

AGREEMENT_SAMPLE_FRACTION = 0.10
MIN_TASKS = 10
PROTOCOL_VERSION = 1
NORMALIZED_ANSWER_FIELDS = ("value", "currency", "basis", "attribution", "fiscal_year", "period_type")
VALID_BASES = {"consolidated", "standalone", "unknown"}
VALID_ATTRIBUTIONS = {"total", "owners_of_parent", "nci"}
VALID_PERIOD_TYPES = {"FY", "H1", "Q1", "Q2", "Q3", "Q4", "unknown"}


class SealingClosedError(RuntimeError):
    """Raised when `seal` is attempted after `compare` has already run."""


class PrepareExistsError(RuntimeError):
    """Raised when `prepare` would overwrite an existing, frozen task set."""


class ResealError(RuntimeError):
    """Raised when `seal` would overwrite an existing reader seal."""


class TaskHashMismatchError(RuntimeError):
    """Raised when a task file's current hash no longer matches tasks.lock."""


class BindingMismatchError(RuntimeError):
    """Raised when tasks_index.json / ground_truth_v2.json / occurrences_v2.json
    no longer matches the hash `tasks.lock` bound them to at prepare time
    (amendment W: the full binding, not just answered task files)."""


class SealHashMismatchError(RuntimeError):
    """Raised when a sealed answer file's content no longer matches its own
    recorded hash, or that hash disagrees with `seals.lock`."""


class IncompleteAdjudicationError(RuntimeError):
    """Raised when `compare` runs while a prepared task has no sealed answer."""


class InvalidAnswerError(RuntimeError):
    """Raised when a sealed answer fails domain validation -- amendment W:
    the whole compare transaction fails, no partial writes."""


def _adjudication_dir(esef_v2_dir: Path) -> Path:
    return esef_v2_dir / "adjudication"


def _file_sha256_or_none(path: Path) -> str | None:
    return hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else None


def prepare(esef_v2_dir: Path, seed: int) -> dict:
    adjudication_dir = _adjudication_dir(esef_v2_dir)
    if (adjudication_dir / "tasks.lock").exists():
        raise PrepareExistsError(
            f"{adjudication_dir / 'tasks.lock'} already exists -- task/index/GT bindings are frozen by hash "
            "once prepared (amendment W); re-preparing would associate old sealed answers with new task ids."
        )

    gt_path = esef_v2_dir / "ground_truth_v2.json"
    gt = json.loads(gt_path.read_text(encoding="utf-8"))
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

    lock_tasks: list[dict] = []
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
            "unit": slot.get("unit"),  # amendment AG: evidence location, never a value
        }
        task_bytes = json.dumps(task, indent=2, ensure_ascii=False, sort_keys=True).encode("utf-8") + b"\n"
        (tasks_dir / f"{task_id}.json").write_bytes(task_bytes)
        lock_tasks.append({"task_id": task_id, "sha256": hashlib.sha256(task_bytes).hexdigest()})
        index[task_id] = {"slot_id": slot["slot_id"], "event_id": slot["event_id"], "task_kind": kind}

    index_path = adjudication_dir / "tasks_index.json"
    index_path.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")

    # Amendment W: the lock binds task files TOGETHER WITH the index and the
    # GT/occurrences snapshot they were prepared against -- astra r2 finding
    # 13: the old lock covered only task files, so a changed index or a
    # re-run labeler between prepare and compare went undetected.
    lock = {
        "tasks": lock_tasks,
        "index_sha256": _file_sha256_or_none(index_path),
        "gt_sha256": _file_sha256_or_none(gt_path),
        "occurrences_sha256": _file_sha256_or_none(esef_v2_dir / "occurrences_v2.json"),
    }
    (adjudication_dir / "tasks.lock").write_text(json.dumps(lock, indent=2) + "\n", encoding="utf-8")
    return {"tasks": len(chosen), "disagreements": len(disagreements), "agreement_sample": len(sample)}


def _verify_bindings(esef_v2_dir: Path, adjudication_dir: Path, lock: dict) -> None:
    """Re-verifies the FULL amendment-W binding: not just the task files a
    reader happens to be answering, but the index and the GT/occurrences
    snapshot the whole task set was prepared against."""
    checks = (
        (adjudication_dir / "tasks_index.json", lock.get("index_sha256")),
        (esef_v2_dir / "ground_truth_v2.json", lock.get("gt_sha256")),
        (esef_v2_dir / "occurrences_v2.json", lock.get("occurrences_sha256")),
    )
    for path, locked_hash in checks:
        if _file_sha256_or_none(path) != locked_hash:
            raise BindingMismatchError(f"{path} no longer matches the hash tasks.lock bound it to at prepare time")


def seal(esef_v2_dir: Path, reader: str, answers_path: Path) -> list[Path]:
    adjudication_dir = _adjudication_dir(esef_v2_dir)
    if (adjudication_dir / "resolutions.json").exists():
        raise SealingClosedError("compare has already run for this corpus; sealing is closed")

    lock = json.loads((adjudication_dir / "tasks.lock").read_text(encoding="utf-8"))
    _verify_bindings(esef_v2_dir, adjudication_dir, lock)

    answers = json.loads(answers_path.read_text(encoding="utf-8"))
    index = json.loads((adjudication_dir / "tasks_index.json").read_text(encoding="utf-8"))
    lock_by_task = {t["task_id"]: t["sha256"] for t in lock["tasks"]}

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

    seals_path = adjudication_dir / "seals.lock"
    seals_registry = json.loads(seals_path.read_text(encoding="utf-8")) if seals_path.exists() else {}

    timestamp = datetime.now(timezone.utc).isoformat()
    sealed_paths = []
    for event_id, event_answers in by_event.items():
        event_dir = adjudication_dir / event_id
        dest = event_dir / f"{reader}.json"
        if dest.exists():
            raise ResealError(f"{dest} already sealed for reader {reader!r} -- resealing is refused (amendment W)")
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
        seal_hash = hashlib.sha256(body).hexdigest()
        payload["sha256"] = seal_hash
        dest.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        # Amendment W: the seal hash is ALSO recorded in a separate registry,
        # independent of the sealed file's own self-reported hash, so
        # `compare` has an external reference point to catch a file whose
        # content AND embedded hash were both altered consistently.
        seals_registry[f"{event_id}/{reader}"] = seal_hash
        sealed_paths.append(dest)

    seals_path.write_text(json.dumps(seals_registry, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return sealed_paths


def _verify_registered_seal(adjudication_dir: Path, identity: str, expected_hash: str) -> tuple[dict, Path]:
    """Loads and verifies ONE seal registered in `seals.lock` -- the
    registry entry is the source of truth (amendment AB / astra r3 finding
    13): a missing file, unreadable/malformed JSON, or a hash that
    disagrees with either the payload's own embedded `sha256` or the
    registry's recorded hash all abort `compare`."""
    event_id, reader = identity.rsplit("/", 1)
    sealed_path = adjudication_dir / event_id / f"{reader}.json"
    if not sealed_path.exists():
        raise SealHashMismatchError(f"registered seal {identity} is missing its file {sealed_path}")
    try:
        payload = json.loads(sealed_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SealHashMismatchError(f"registered seal {identity} at {sealed_path} is not valid JSON: {exc}") from exc
    if not (isinstance(payload, dict) and "answers" in payload and "reader" in payload):
        raise SealHashMismatchError(f"registered seal {identity} at {sealed_path} is malformed")
    body = {k: v for k, v in payload.items() if k != "sha256"}
    recomputed = hashlib.sha256(json.dumps(body, indent=2, ensure_ascii=False, sort_keys=True).encode("utf-8")).hexdigest()
    if recomputed != payload.get("sha256") or recomputed != expected_hash:
        raise SealHashMismatchError(f"registered seal {identity} hash mismatch")
    return payload, sealed_path


def _find_unregistered_seals(adjudication_dir: Path, registered_paths: set[Path]) -> Path | None:
    """A directory scan used ONLY to detect an EXTRA sealed file that
    `seals.lock` never registered -- the registry (not the directory)
    remains the source of truth for which seals to actually read (amendment
    AB). Returns the first offending path, or `None`."""
    for path in adjudication_dir.rglob("*.json"):
        if path.parent == adjudication_dir or path.parent.name == "tasks":
            continue  # tasks.lock / tasks_index.json / seals.lock / resolutions.json / adjudication/tasks/*
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            continue
        if isinstance(payload, dict) and "answers" in payload and "reader" in payload and path.resolve() not in registered_paths:
            return path
    return None


def _decimal_or_none(value) -> Decimal | None:
    try:
        return Decimal(str(value))
    except (InvalidOperation, TypeError):
        return None


def _validate_answer_fields(task_id: str, reader: str, answer: dict, task_unit: str | None) -> None:
    """Amendment W / astra r2 finding 22: an answer that isn't explicitly
    `unverified` must be a fully-formed, in-domain reading -- a decimal that
    parses but is NaN/Infinity, a non-3-letter currency, or an out-of-enum
    basis/attribution/period_type/fiscal_year used to sail through as an
    "incomplete answer" only when a whole FIELD was missing, letting a
    single garbage value (e.g. unparseable text) turn into the literal
    string `"None"` written into ground_truth_v2.json.

    Amendment AG / astra r3 finding 25: whether `currency` may be a code AT
    ALL depends on the TASK's own `unit` (evidence location): a monetary
    unit (has a currency numerator) requires a 3-letter code; a non-monetary
    unit (e.g. a bare share count, `xbrli:pure`) requires `currency: null`
    -- a fabricated code on a non-monetary slot is itself an invalid
    answer, not merely ignored."""
    if answer.get("unverified"):
        return
    identity = f"{task_id} ({reader})"
    missing = [f for f in NORMALIZED_ANSWER_FIELDS if f not in answer]
    if missing:
        raise InvalidAnswerError(f"{identity}: answer missing field(s) {missing}")
    value = _decimal_or_none(answer["value"])
    if value is None or not value.is_finite():
        raise InvalidAnswerError(f"{identity}: value {answer['value']!r} is not a finite decimal")
    currency = answer["currency"]
    is_monetary_unit = currency_from_unit(task_unit) is not None
    if is_monetary_unit:
        if not (isinstance(currency, str) and len(currency) == 3 and currency.isalpha() and currency.isupper()):
            raise InvalidAnswerError(f"{identity}: currency {currency!r} is not a 3-letter code (task unit {task_unit!r} is monetary)")
    elif currency is not None:
        raise InvalidAnswerError(f"{identity}: currency {currency!r} must be null -- task unit {task_unit!r} has no monetary numerator")
    if answer["basis"] not in VALID_BASES:
        raise InvalidAnswerError(f"{identity}: basis {answer['basis']!r} outside the known domain {sorted(VALID_BASES)}")
    if answer["attribution"] not in VALID_ATTRIBUTIONS:
        raise InvalidAnswerError(f"{identity}: attribution {answer['attribution']!r} outside the known domain {sorted(VALID_ATTRIBUTIONS)}")
    if answer["period_type"] not in VALID_PERIOD_TYPES:
        raise InvalidAnswerError(f"{identity}: period_type {answer['period_type']!r} outside the known domain {sorted(VALID_PERIOD_TYPES)}")
    if not isinstance(answer["fiscal_year"], int) or isinstance(answer["fiscal_year"], bool):
        raise InvalidAnswerError(f"{identity}: fiscal_year {answer['fiscal_year']!r} is not an integer")


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
    """The reader's own full, independently-derived reading. Returns `None`
    for an explicitly `unverified` answer; every other answer reaching this
    point has already passed `_validate_answer_fields`, so construction here
    cannot fail."""
    if answer.get("unverified"):
        return None
    return (
        Decimal(str(answer["value"])),
        answer["currency"],
        answer["basis"],
        answer["attribution"],
        answer["fiscal_year"],
        answer["period_type"],
    )


def compare(esef_v2_dir: Path) -> dict:
    adjudication_dir = _adjudication_dir(esef_v2_dir)
    lock = json.loads((adjudication_dir / "tasks.lock").read_text(encoding="utf-8"))
    _verify_bindings(esef_v2_dir, adjudication_dir, lock)

    # Amendment AB (1) / astra r3 finding 13: re-verify the sha256 of EVERY
    # task file `tasks.lock` locked -- not just the ones a sealed answer
    # happens to reference. Captures each task's `unit` for answer
    # validation (amendment AG) while the file is already open.
    task_units: dict[str, str | None] = {}
    for entry in lock["tasks"]:
        task_id = entry["task_id"]
        task_path = adjudication_dir / "tasks" / f"{task_id}.json"
        if not task_path.exists():
            raise TaskHashMismatchError(f"{task_path} is missing (locked in tasks.lock)")
        task_bytes = task_path.read_bytes()
        if hashlib.sha256(task_bytes).hexdigest() != entry["sha256"]:
            raise TaskHashMismatchError(f"{task_path} no longer matches tasks.lock")
        task_units[task_id] = json.loads(task_bytes).get("unit")

    index = json.loads((adjudication_dir / "tasks_index.json").read_text(encoding="utf-8"))
    gt = json.loads((esef_v2_dir / "ground_truth_v2.json").read_text(encoding="utf-8"))
    slots_by_id = {s["slot_id"]: s for s in gt["slots"]}

    seals_path = adjudication_dir / "seals.lock"
    seals_registry = json.loads(seals_path.read_text(encoding="utf-8")) if seals_path.exists() else {}

    # Amendment AB (2): `seals.lock` is the source of truth for WHICH seals
    # to read -- never a directory walk. Every registered seal must exist
    # and verify; a sealed file on disk that ISN'T registered aborts too.
    answers_by_task: dict[str, list[tuple[str, dict]]] = {}
    registered_paths: set[Path] = set()
    for identity, expected_hash in seals_registry.items():
        payload, sealed_path = _verify_registered_seal(adjudication_dir, identity, expected_hash)
        registered_paths.add(sealed_path.resolve())
        for task_id, answer in payload["answers"].items():
            answers_by_task.setdefault(task_id, []).append((payload["reader"], answer))

    unregistered = _find_unregistered_seals(adjudication_dir, registered_paths)
    if unregistered is not None:
        raise SealHashMismatchError(f"sealed file {unregistered} is not registered in adjudication/seals.lock (unregistered seal)")

    missing = sorted(task_id for task_id in index if task_id not in answers_by_task)
    if missing:
        raise IncompleteAdjudicationError(
            f"{len(missing)} prepared task(s) have no sealed answer yet (amendment W): {', '.join(missing)}"
        )

    # Amendment W: validate EVERY sealed answer before touching anything --
    # a single invalid answer fails the whole transaction, never a partial
    # write (astra r2 finding 22).
    for task_id, readers in answers_by_task.items():
        for reader, answer in readers:
            _validate_answer_fields(task_id, reader, answer, task_units.get(task_id))

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
        # design -- LABELING.md step 5): the reader could not settle it, or
        # multiple readers disagreed with each other -> unsettled; a single
        # complete, matching read -> agree; a single complete, DIFFERING
        # read WITH an evidence anchor -> adjudicated (settles a machine
        # conflict or flags a contradicted agreement-sample slot as a
        # systematic-error signal, LABELING.md step 6); a differing read
        # with NO evidence anchor becomes `unverified` -- amendment W: a
        # contradicted machine label never stays `machine` -- without ever
        # applying the disputed value.
        if any_unsettled or len(answer_tuples) != 1 or None in answer_tuples:
            outcome, new_verification = "unsettled", "unverified"
        else:
            answer_tuple = next(iter(answer_tuples))
            if answer_tuple == slot_tuple:
                outcome, new_verification = "agree", "second_read"
            elif evidence is None:
                outcome, new_verification = "rejected_missing_evidence", "unverified"
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
        except (SealingClosedError, ResealError, TaskHashMismatchError, BindingMismatchError) as exc:
            print(f"adjudicate seal: refused -- {exc}")
            return 1
        print(f"adjudicate seal: sealed {len(sealed)} event file(s) for reader {args.reader!r}")
    elif args.command == "compare":
        try:
            result = compare(esef_v2_dir)
        except (
            IncompleteAdjudicationError,
            BindingMismatchError,
            SealHashMismatchError,
            InvalidAnswerError,
            TaskHashMismatchError,
        ) as exc:
            print(f"adjudicate compare: refused -- {exc}")
            return 1
        print(f"adjudicate compare: {result['resolutions']} resolutions")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
