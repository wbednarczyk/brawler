#!/usr/bin/env python3
"""Imports the v1 (#182) merged ESEF ground truth into the v2 slot shape
(#331 PR-A, ADR 0112). Stdlib only.

Reads the v1 spike's `ground_truth.json` (esef tier only) + `MANIFEST.json`
(read-only reference: `private/realdata/spikes/esef-positional-gt/`, never
edited here) and the v2 frame's `MANIFEST_v2.json` (for the ticker -> issuer_id
map), and MERGES imported slots into `ground_truth_v2.json`, tagged
`verification: "machine_v1"` -- a declared enum value the measurement harness
excludes from floors until each row is independently re-verified. The
`__owners`/`__equity`/`__profitloss` mapped-key suffixes v1 used to keep or
collapse the owners-of-parent/whole-group variant pair map through
`gt_key_map.json` to (concept_local, attribution).

Usage:
    python3 import_v1.py --v1-dir <esef-positional-gt dir> --esef-v2-dir <dir>
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent

# v1's collapse/split mapped_key spellings -> the v2 key map's metric_key.
# See private/realdata/spikes/esef-positional-gt/merge_labels.py
# ESEF_COLLAPSE_PAIRS: when the total and owners-of-parent readings AGREED
# (no non-controlling interest), v1 collapsed them to the plain metric name;
# when they DISAGREED, both variants survive under their suffixed spelling.
V1_MAPPED_KEY_ALIASES = {
    "net_profit__profitloss": "net_profit",
    "net_profit__owners": "wdf_net_profit_parent",
    "total_equity__equity": "total_equity",
    "total_equity__owners": "wdf_equity_parent",
}


def load_key_map(key_map_path: Path) -> dict[str, dict]:
    data = json.loads(key_map_path.read_text(encoding="utf-8"))
    return {e["metric_key"]: e for e in data["entries"]}


def build_issuer_map(manifest_v2: dict) -> dict[str, str]:
    return {issuer["ticker"]: issuer["issuer_id"] for issuer in manifest_v2["issuers"]}


def import_slots(v1_gt: list[dict], issuer_map: dict[str, str], metric_key_to_entry: dict[str, dict]) -> list[dict]:
    slots = []
    for rec in v1_gt:
        if rec.get("tier") != "esef":
            continue
        issuer_id = issuer_map.get(rec["ticker"])
        if issuer_id is None:
            continue  # v1 covered a ticker outside this v2 acceptance frame

        metric_key = V1_MAPPED_KEY_ALIASES.get(rec["mapped_key"], rec["mapped_key"])
        entry = metric_key_to_entry.get(metric_key)
        if entry is None:
            continue  # a v1 metric with no v2 key-map counterpart

        window = "point_in_time" if entry["period_nature"] == "instant" else "flow"
        period_end = rec["period_end"]
        fiscal_year = int(period_end[:4]) if period_end else None
        # v1 recorded only period_end/period_start, not a period_type label;
        # duration-based period_type classification needs both dates, which
        # v1 point-in-time (instant) records never carry -- so this import
        # stamps a single free-form label rather than guessing FY/H1/Qn from
        # one date. Re-verification (adjudicate.py) assigns the real one.
        period_type = "v1_unclassified"

        event_id = f"{issuer_id}/v1/{Path(rec['file']).stem[:60]}"
        slot_id = (
            f"{event_id}/{entry['concept']}/{entry['attribution']}/{rec['statement_basis']}/"
            f"{window}/reported/{fiscal_year}/{period_type}"
        )
        slots.append(
            {
                "slot_id": slot_id,
                "event_id": event_id,
                "concept_local": entry["concept"],
                "attribution": entry["attribution"],
                "basis": rec["statement_basis"],
                "window": window,
                "variant": "reported",
                "fiscal_year": fiscal_year,
                "period_type": period_type,
                "period_end": period_end,
                "period_start": rec.get("period_start"),
                "currency": rec.get("currency"),
                "value": rec["value"],
                "duration_months": None,
                "verification": "machine_v1",
                "contributing_occurrence_ids": [],
                "resolution_ref": None,
            }
        )
    return slots


def merge_ground_truth(esef_v2_dir: Path, imported_slots: list[dict]) -> dict:
    gt_path = esef_v2_dir / "ground_truth_v2.json"
    if gt_path.exists():
        gt = json.loads(gt_path.read_text(encoding="utf-8"))
    else:
        gt = {"gt_version": "1", "normalization_version": 1, "key_map_version": 1, "slots": []}

    by_id = {s["slot_id"]: s for s in gt["slots"]}
    for slot in imported_slots:
        by_id.setdefault(slot["slot_id"], slot)  # never overwrite an existing (higher-verification) slot
    gt["slots"] = sorted(by_id.values(), key=lambda s: s["slot_id"])
    gt_path.write_text(json.dumps(gt, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return gt


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--v1-dir", required=True)
    parser.add_argument("--esef-v2-dir", required=True)
    args = parser.parse_args(argv)

    v1_dir = Path(args.v1_dir)
    esef_v2_dir = Path(args.esef_v2_dir)

    v1_gt = json.loads((v1_dir / "ground_truth.json").read_text(encoding="utf-8"))
    manifest_v2 = json.loads((esef_v2_dir / "MANIFEST_v2.json").read_text(encoding="utf-8"))
    issuer_map = build_issuer_map(manifest_v2)
    metric_key_to_entry = load_key_map(HERE / "gt_key_map.json")

    imported = import_slots(v1_gt, issuer_map, metric_key_to_entry)
    gt = merge_ground_truth(esef_v2_dir, imported)
    v1_count = sum(1 for s in gt["slots"] if s["verification"] == "machine_v1")
    print(f"import_v1: {len(imported)} v1 slots considered, {v1_count} machine_v1 slots now in ground_truth_v2.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
