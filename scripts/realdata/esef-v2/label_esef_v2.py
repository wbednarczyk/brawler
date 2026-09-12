#!/usr/bin/env python3
"""ESEF ground-truth labeler v2 (#331 PR-A, ADR 0112). Stdlib only.

For every event in `MANIFEST_v2.json`, parses its corpus file (or ZIP
member) with `esef_ixbrl`, emits every fact as a raw occurrence row
(`occurrences_v2.json`, no panel filter -- the panel is a scoring-time
concern) and derives normalized `ground_truth_v2.json` slots keyed off
`gt_key_map.json`. Two occurrences agreeing on concept, dimensions, period,
unit and currency merge into one `machine` slot; disagreeing on value makes
the slot `unverified` with both values kept in `resolution_ref`.

Usage:
    python3 label_esef_v2.py --esef-v2-dir <dir>
"""
from __future__ import annotations

import argparse
import json
import zipfile
from io import BytesIO
from pathlib import Path

import esef_ixbrl as ix
from build_frame import classify_period_type, months_between

HERE = Path(__file__).resolve().parent


def load_key_map(key_map_path: Path) -> dict:
    return json.loads(key_map_path.read_text(encoding="utf-8"))


def read_event_bytes(corpus_dir: Path, file_info: dict) -> bytes:
    path = corpus_dir / file_info["name"]
    data = path.read_bytes()
    member = file_info.get("package_member")
    if member is None:
        return data
    with zipfile.ZipFile(BytesIO(data)) as z:
        return z.read(member)


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
    months = months_between(start, end)
    period_type = classify_period_type(months, int(end.split("-")[1]))
    return int(end.split("-")[0]), period_type, start, end, months


def currency_from_unit(unit_str: str | None) -> str | None:
    if not unit_str:
        return None
    code = unit_str.split("/")[0].split(":")[-1]
    return code if len(code) == 3 and code.isalpha() and code.isupper() else None


def resolve_attribution(base_attribution: str, dimensions: list[dict]) -> str:
    """Concept-level attribution (from the key map) unless a dimension
    member carries explicit NCI/parent semantics -- e.g. the base `ProfitLoss`
    concept (attribution `total`) tagged with a `ComponentsOfEquityAxis`
    member naming non-controlling interests really reports the NCI slice."""
    for dim in dimensions:
        member_local = (dim.get("member") or "").split(":")[-1]
        if "NoncontrollingInterest" in member_local:
            return "nci"
        if "Parent" in member_local:
            return "owners_of_parent"
    return base_attribution


def emit_occurrences(event: dict, parsed_occurrences: list[dict]) -> list[dict]:
    rows = []
    for n, occ in enumerate(parsed_occurrences, start=1):
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


def derive_slots(event: dict, occurrences: list[dict], concept_info: dict[str, dict]) -> list[dict]:
    groups: dict[tuple, list[dict]] = {}
    for occ in occurrences:
        if occ["parse_status"] != "ok" or occ["value"] is None or not occ["period"]:
            continue
        info = concept_info.get(occ["concept_local"])
        if info is None:
            continue
        dim_sig = tuple(sorted((d.get("axis"), d.get("member")) for d in occ["dimensions"]))
        currency = currency_from_unit(occ["unit"])
        period_key = tuple(sorted(occ["period"].items()))
        groups.setdefault((occ["concept_local"], dim_sig, period_key, currency), []).append(occ)

    slots = []
    for (concept_local, _dim_sig, _period_key, currency), occs in sorted(
        groups.items(), key=lambda kv: (kv[0][0], kv[0][1])
    ):
        values = sorted({o["value"] for o in occs})
        occ_ids = sorted(o["occurrence_id"] for o in occs)
        info = concept_info[concept_local]
        attribution = resolve_attribution(info["attribution"], occs[0]["dimensions"])
        window = "point_in_time" if "instant" in occs[0]["period"] else "flow"
        fiscal_year, period_type, period_start, period_end, duration_months = classify_period(occs[0]["period"])

        if len(values) > 1:
            verification, resolution_ref, value = (
                "unverified",
                {"conflicting_values": values, "occurrence_ids": occ_ids},
                values[0],
            )
        else:
            verification, resolution_ref, value = "machine", None, values[0]

        slots.append(
            {
                "slot_id": f"{event['event_id']}/{concept_local}/{attribution}/{event['basis_scope']}/{window}/reported/{fiscal_year}/{period_type}",
                "event_id": event["event_id"],
                "concept_local": concept_local,
                "attribution": attribution,
                "basis": event["basis_scope"],
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
    return slots


def label_esef_v2(esef_v2_dir: str, key_map_path: Path | None = None) -> dict:
    out = Path(esef_v2_dir)
    manifest = json.loads((out / "MANIFEST_v2.json").read_text(encoding="utf-8"))
    key_map = load_key_map(key_map_path or (HERE / "gt_key_map.json"))
    concept_info = {e["concept"]: e for e in key_map["entries"]}

    all_occurrences: list[dict] = []
    all_slots: list[dict] = []
    for event in manifest["events"]:
        data = read_event_bytes(out / "corpus", event["file"])
        parsed = ix.parse_instance(data, event["file"].get("package_member"))
        occurrences = emit_occurrences(event, parsed["occurrences"])
        all_occurrences.extend(occurrences)
        all_slots.extend(derive_slots(event, occurrences, concept_info))

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
    return ground_truth_doc


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--esef-v2-dir", required=True)
    args = parser.parse_args(argv)
    doc = label_esef_v2(args.esef_v2_dir)
    print(f"label_esef_v2: {len(doc['slots'])} ground-truth slots -> {args.esef_v2_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
