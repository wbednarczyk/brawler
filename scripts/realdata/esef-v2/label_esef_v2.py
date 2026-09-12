#!/usr/bin/env python3
"""ESEF ground-truth labeler v2 (#331 PR-A, ADR 0112, amendment 1). Stdlib
only.

Event boundary = the whole package (amendment A): for every event in
`MANIFEST_v2.json`, parses EVERY `.xhtml`/`.html` member of its corpus file
(not just the manifest's primary member) with `esef_ixbrl`, emitting every
fact as a raw occurrence row (`occurrences_v2.json`, no panel filter -- the
panel is a scoring-time concern) with its own `package_member`, `basis` and
`language` derived per-member (member path -> package/outer evidence ->
cover-page text -> the event's own headline classification as a last
resort), and derives normalized `ground_truth_v2.json` slots keyed off
`gt_key_map.json`.

Two occurrences agreeing on package_member, concept, attribution, unit,
currency and period merge into one `machine` slot (Decimal-normalized value
agreement, never a string compare); disagreeing makes the slot `unverified`
with every distinct value kept in `resolution_ref`. A dimensioned occurrence
whose dimension does not resolve to a known attribution (NCI/owners-of-parent)
never becomes a slot -- it stays evidence in `occurrences_v2.json` only
(amendment F). A concept whose expanded namespace is not the recognized IFRS
`ifrs-full` family is treated as an unmapped extension concept for slot
purposes, however its local name reads (amendment M).

Usage:
    python3 label_esef_v2.py --esef-v2-dir <dir>
"""
from __future__ import annotations

import argparse
import json
import re
import zipfile
from decimal import Decimal
from io import BytesIO
from pathlib import Path

import build_frame as bf
import esef_ixbrl as ix

HERE = Path(__file__).resolve().parent

IFRS_NAMESPACE_RE = re.compile(r"^\{http://xbrl\.ifrs\.org/taxonomy/[^/]+/ifrs-full\}")


def load_key_map(key_map_path: Path) -> dict:
    return json.loads(key_map_path.read_text(encoding="utf-8"))


def read_event_members(corpus_dir: Path, file_info: dict) -> list[tuple[str | None, bytes]]:
    """Every `.xhtml`/`.html` member of the event's file (amendment A: the
    event boundary is the whole package, so every member gets labeled, not
    just the manifest's recorded primary `package_member`). A loose file
    yields a single `(None, data)` member."""
    data = (corpus_dir / file_info["name"]).read_bytes()
    if not zipfile.is_zipfile(BytesIO(data)):
        return [(None, data)]
    return ix.zip_members(data)


def is_ifrs_concept(concept_qname: str | None) -> bool:
    """Amendment M: matching uses the EXPANDED name against the key map's
    IFRS namespace family -- an extension or unresolved-prefix concept never
    maps, however its local name happens to read."""
    return bool(IFRS_NAMESPACE_RE.match(concept_qname or ""))


def classify_period(period: dict | None) -> tuple:
    """(fiscal_year, period_type, period_start, period_end, duration_months)
    for ONE occurrence's own period -- comparative-year facts in the same
    filing get their own (fiscal_year, period_type), distinct from the
    event's labeled current period; that is why `slot_id` carries them."""
    if not period:
        return None, "unknown", None, None, None
    if "instant" in period:
        end = period["instant"]
        month = int(end.split("-")[1])
        period_type = {12: "FY", 6: "H1", 3: "Q1", 9: "Q3"}.get(month, "unknown")
        return int(end.split("-")[0]), period_type, None, end, None
    start, end = period.get("start"), period.get("end")
    if not start or not end:
        return None, "unknown", start, end, None
    months = bf.months_between(start, end)
    period_type = bf.classify_period_type(months, int(end.split("-")[1]))
    return int(end.split("-")[0]), period_type, start, end, months


def currency_from_unit(unit_str: str | None) -> str | None:
    if not unit_str:
        return None
    code = unit_str.split("/")[0].split(":")[-1]
    return code if len(code) == 3 and code.isalpha() and code.isupper() else None


def resolve_attribution(base_attribution: str, dimensions: list[dict]) -> str | None:
    """Concept-level attribution (from the key map) for a PRIMARY (no
    dimensions) occurrence; a dimension member carrying explicit NCI/parent
    semantics overrides it (e.g. the base `ProfitLoss` concept, attribution
    `total`, tagged with a `ComponentsOfEquityAxis` NCI member really reports
    the NCI slice). A dimensioned occurrence whose dimension does NOT map to
    a known attribution returns `None` -- amendment F: it is not a scoring
    slot, only occurrence evidence (a segment/geography breakdown must never
    silently collide with the primary total under the same identity)."""
    for dim in dimensions:
        member_local = (dim.get("member") or "").split(":")[-1]
        if "NoncontrollingInterest" in member_local:
            return "nci"
        if "Parent" in member_local:
            return "owners_of_parent"
    return base_attribution if not dimensions else None


def build_slot_id(
    event_id: str,
    package_member: str | None,
    concept_local: str,
    attribution: str,
    basis: str,
    window: str,
    variant: str,
    fiscal_year: int | None,
    period_type: str,
    currency: str | None,
) -> str:
    """Amendment F slot id template, the single source of truth
    (`adjudicate.py` re-keys through this same function on a correction)."""
    return "/".join(
        str(x) for x in (event_id, package_member or "-", concept_local, attribution, basis, window, variant, fiscal_year, period_type, currency)
    )


def emit_occurrences(event: dict, parsed_occurrences: list[dict], start_index: int) -> list[dict]:
    rows = []
    for offset, occ in enumerate(parsed_occurrences):
        n = start_index + offset
        rows.append(
            {
                "occurrence_id": f"{event['event_id']}#{n}",
                "event_id": event["event_id"],
                **occ,
                "verification": "machine",
                "resolution_ref": None,
            }
        )
    return rows


def classify_member_evidence(event: dict, member_name: str | None, member_bytes: bytes, lang: str | None) -> tuple[str, str]:
    """Per-member (basis, language), amendment A: "GT slots carry
    basis/language per member". Falls back to the event's own headline
    classification (established by `build_frame.py` from the file's primary
    member) when a specific member's own evidence does not resolve --
    e.g. a bare numeric-only note member with no path/cover-page hint."""
    outer_hints = [event["document"].get("title") or "", event["file"]["name"]]
    cover_text = bf.cover_page_text(member_bytes)
    basis, _contradiction = bf.classify_basis(member_name or "", outer_hints, cover_text)
    if basis == "unknown":
        basis = event["basis_scope"]
    language = bf.classify_language(lang, [member_name or "", event["file"]["name"]])
    if language == "unknown":
        language = event["language"]
    return basis, language


def derive_slots(event: dict, occurrences: list[dict], concept_info: dict[str, dict], member_basis: dict[str | None, str]) -> list[dict]:
    groups: dict[tuple, list[dict]] = {}
    for occ in occurrences:
        if occ["parse_status"] != "ok" or occ["value"] is None or not occ["period"]:
            continue
        if not is_ifrs_concept(occ["concept_qname"]):
            continue
        info = concept_info.get(occ["concept_local"])
        if info is None:
            continue
        attribution = resolve_attribution(info["attribution"], occ["dimensions"])
        if attribution is None:
            continue
        currency = currency_from_unit(occ["unit"])
        period_key = tuple(sorted(occ["period"].items()))
        # Grouping key includes package_member, raw unit AND currency
        # (astra r1 finding 5: unit identity was previously omitted, so a
        # plain-amount unit and a differently-shaped unit resolving to the
        # same currency code could wrongly merge).
        key = (occ["package_member"], occ["concept_local"], attribution, occ["unit"], currency, period_key)
        groups.setdefault(key, []).append(occ)

    slots = []
    for (package_member, concept_local, attribution, _unit, currency, _period_key), occs in sorted(
        groups.items(), key=lambda kv: (kv[0][0] or "", kv[0][1], kv[0][2])
    ):
        # Normalized-Decimal value agreement, never a string compare
        # (astra r1 finding 5: "1.0" and "1.00" are the same value).
        value_by_decimal: dict[Decimal, str] = {}
        for o in occs:
            value_by_decimal.setdefault(Decimal(o["value"]), o["value"])
        distinct = sorted(value_by_decimal.items())
        occ_ids = sorted(o["occurrence_id"] for o in occs)
        window = "point_in_time" if "instant" in occs[0]["period"] else "flow"
        fiscal_year, period_type, period_start, period_end, duration_months = classify_period(occs[0]["period"])
        basis = member_basis.get(package_member, event["basis_scope"])

        if len(distinct) > 1:
            verification, resolution_ref, value = (
                "unverified",
                {"conflicting_values": [s for _d, s in distinct], "occurrence_ids": occ_ids},
                distinct[0][1],
            )
        else:
            verification, resolution_ref, value = "machine", None, distinct[0][1]

        slots.append(
            {
                "slot_id": build_slot_id(
                    event["event_id"], package_member, concept_local, attribution, basis, window, "reported", fiscal_year, period_type, currency
                ),
                "event_id": event["event_id"],
                "package_member": package_member,
                "concept_local": concept_local,
                "attribution": attribution,
                "basis": basis,
                "window": window,
                "variant": "reported",
                "fiscal_year": fiscal_year,
                "period_type": period_type,
                "period_end": period_end,
                "period_start": period_start,
                "currency": currency,
                "value": value,
                "duration_months": duration_months,
                "verification": verification,
                "contributing_occurrence_ids": occ_ids,
                "resolution_ref": resolution_ref,
            }
        )

    seen_ids = set()
    for slot in slots:
        if slot["slot_id"] in seen_ids:
            raise ValueError(f"duplicate slot_id {slot['slot_id']!r} -- grouping key is not unique enough (astra r1 finding 5)")
        seen_ids.add(slot["slot_id"])
    return slots


def label_esef_v2(esef_v2_dir: str, key_map_path: Path | None = None) -> dict:
    out = Path(esef_v2_dir)
    manifest = json.loads((out / "MANIFEST_v2.json").read_text(encoding="utf-8"))
    key_map = load_key_map(key_map_path or (HERE / "gt_key_map.json"))
    concept_info = {e["concept"]: e for e in key_map["entries"]}

    all_occurrences: list[dict] = []
    all_slots: list[dict] = []
    unresolved_occurrences = 0

    for event in manifest["events"]:
        members = read_event_members(out / "corpus", event["file"])
        event_occurrences: list[dict] = []
        member_basis: dict[str | None, str] = {}

        for member_name, member_bytes in members:
            parsed = ix.parse_instance(member_bytes, member_name)
            basis, _language = classify_member_evidence(event, member_name, member_bytes, parsed.get("lang"))
            member_basis[member_name] = basis
            rows = emit_occurrences(event, parsed["occurrences"], start_index=len(event_occurrences) + 1)
            event_occurrences.extend(rows)

        unresolved_occurrences += sum(1 for o in event_occurrences if o["parse_status"] != "ok")
        all_occurrences.extend(event_occurrences)
        all_slots.extend(derive_slots(event, event_occurrences, concept_info, member_basis))

    occurrences_doc = sorted(all_occurrences, key=lambda o: o["occurrence_id"])
    ground_truth_doc = {
        "gt_version": "1",
        "normalization_version": key_map.get("normalization_version", 1),
        "key_map_version": key_map["key_map_version"],
        "slots": sorted(all_slots, key=lambda s: s["slot_id"]),
    }

    (out / "occurrences_v2.json").write_text(
        json.dumps(occurrences_doc, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    (out / "ground_truth_v2.json").write_text(
        json.dumps(ground_truth_doc, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    return {"ground_truth": ground_truth_doc, "unresolved_occurrences": unresolved_occurrences}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--esef-v2-dir", required=True)
    args = parser.parse_args(argv)
    result = label_esef_v2(args.esef_v2_dir)
    print(
        f"label_esef_v2: {len(result['ground_truth']['slots'])} ground-truth slots, "
        f"{result['unresolved_occurrences']} unresolved occurrences (unit/context/namespace) -> {args.esef_v2_dir}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
