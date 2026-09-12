#!/usr/bin/env python3
"""Imports the v1 (#182) merged ESEF ground truth as a REFERENCE artifact
(#331 PR-A, ADR 0112, amendment 2 Y). Stdlib only.

Reads the v1 spike's `ground_truth.json` (esef tier only) + `MANIFEST.json`
(read-only reference: `private/realdata/spikes/esef-positional-gt/`, never
edited here) and the v2 frame's `MANIFEST_v2.json` (for the ticker ->
issuer_id map), and writes `machine_v1_reference.json` -- its OWN artifact,
tagged `verification: "machine_v1"`. It NEVER merges into
`ground_truth_v2.json` unless `--map-to-events <json>` explicitly binds a v1
file to a FROZEN manifest event_id (astra r2 finding 21: the old code
invented synthetic `/v1/...` event ids with no manifest counterpart, which
the harness's orphan-event guard now rejects outright -- breaking
measurement rather than merely leaving the v1 rows unverified). Unmapped
rows stay reference-only and are reported as counts.

The `__owners`/`__equity`/`__profitloss` mapped-key suffixes v1 used to keep
or collapse the owners-of-parent/whole-group variant pair map through
`gt_key_map.json` to (concept_local, attribution).

Usage:
    python3 import_v1.py --v1-dir <esef-positional-gt dir> --esef-v2-dir <dir> \\
        [--map-to-events <mapping.json>]

`mapping.json` shape: `{"<v1 ground_truth.json 'file' value>": "<frozen manifest event_id>"}`.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from label_esef_v2 import SLOT_KEYS, build_slot_id, normalized_by_for_slot

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


def load_contract_normalized(key_map_path: Path) -> list[dict]:
    return json.loads(key_map_path.read_text(encoding="utf-8")).get("contract_normalized", [])


def build_issuer_map(manifest_v2: dict) -> dict[str, str]:
    return {issuer["ticker"]: issuer["issuer_id"] for issuer in manifest_v2["issuers"]}


def _build_slot(rec: dict, issuer_id: str, entry: dict, event_id: str, contract_normalized: list[dict]) -> dict:
    window = "point_in_time" if entry["period_nature"] == "instant" else "flow"
    period_end = rec["period_end"]
    fiscal_year = int(period_end[:4]) if period_end else None
    # v1 recorded only period_end/period_start, not a period_type label;
    # duration-based period_type classification needs both dates, which v1
    # point-in-time (instant) records never carry -- so this import stamps a
    # single free-form label rather than guessing FY/H1/Qn from one date.
    # Re-verification (adjudicate.py) assigns the real one.
    period_type = "v1_unclassified"
    currency = rec.get("currency")
    # v1 carried no raw iXBRL unit measure string, only the derived
    # currency code -- ISO 4217 is the only unit shape v1 ever represented
    # (GPW/ESEF monetary facts); a currency-less v1 row has no unit either.
    unit = f"iso4217:{currency}" if currency else None
    # v1 carried no package_member (it read the first ZIP instance only,
    # astra r1 finding 7); "-" is the amendment F template's explicit token
    # for "no member", same as a loose-file slot.
    slot_id = build_slot_id(
        event_id, None, entry["concept"], entry["attribution"], rec["statement_basis"], window, "reported", fiscal_year, period_type, currency
    )
    slot = {
        "slot_id": slot_id,
        "event_id": event_id,
        "package_member": None,
        "concept_local": entry["concept"],
        "mapped": True,  # amendment AF: v1 only ever imports a row with a real key-map entry
        "attribution": entry["attribution"],
        "basis": rec["statement_basis"],
        "window": window,
        "variant": "reported",
        "fiscal_year": fiscal_year,
        "period_type": period_type,
        "period_end": period_end,
        "period_start": rec.get("period_start"),
        "currency": currency,
        "unit": unit,
        "value": rec["value"],
        "duration_months": None,
        "language": "unknown",  # amendment AF: v1 had no member language
        "population": "floor",  # v1 read the primary (pinned-language-equivalent) consolidated filing
        "normalized_by": normalized_by_for_slot(entry["concept"], window, period_type, contract_normalized),
        "verification": "machine_v1",
        "contributing_occurrence_ids": [],
        "resolution_ref": None,
    }
    assert set(slot.keys()) == SLOT_KEYS, f"import_v1 slot key set drifted from label_esef_v2.SLOT_KEYS: {set(slot.keys()) ^ SLOT_KEYS}"
    return slot


def import_slots(
    v1_gt: list[dict],
    issuer_map: dict[str, str],
    metric_key_to_entry: dict[str, dict],
    event_map: dict[str, str] | None = None,
    contract_normalized: list[dict] | None = None,
) -> tuple[list[dict], list[dict]]:
    """Returns (reference_slots, mapped_slots). `reference_slots` always use
    a synthetic `<issuer_id>/v1/<file stem>` event id and are written to
    `machine_v1_reference.json` -- amendment Y: they are never eligible for
    `ground_truth_v2.json` on their own. `mapped_slots` use the REAL frozen
    manifest event_id from `event_map` (keyed by the v1 record's `file`) and
    are the only slots `main()` merges into `ground_truth_v2.json`."""
    reference_slots: list[dict] = []
    mapped_slots: list[dict] = []
    contract_normalized = contract_normalized or []
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

        synthetic_event_id = f"{issuer_id}/v1/{Path(rec['file']).stem[:60]}"
        reference_slots.append(_build_slot(rec, issuer_id, entry, synthetic_event_id, contract_normalized))

        mapped_event_id = (event_map or {}).get(rec["file"])
        if mapped_event_id:
            mapped_slots.append(_build_slot(rec, issuer_id, entry, mapped_event_id, contract_normalized))

    return reference_slots, mapped_slots


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
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--v1-dir", required=True)
    parser.add_argument("--esef-v2-dir", required=True)
    parser.add_argument(
        "--map-to-events",
        type=Path,
        default=None,
        help='JSON {"<v1 file>": "<frozen manifest event_id>"} -- binds explicit v1 rows into ground_truth_v2.json',
    )
    args = parser.parse_args(argv)

    v1_dir = Path(args.v1_dir)
    esef_v2_dir = Path(args.esef_v2_dir)

    v1_gt = json.loads((v1_dir / "ground_truth.json").read_text(encoding="utf-8"))
    manifest_v2 = json.loads((esef_v2_dir / "MANIFEST_v2.json").read_text(encoding="utf-8"))
    issuer_map = build_issuer_map(manifest_v2)
    metric_key_to_entry = load_key_map(HERE / "gt_key_map.json")
    contract_normalized = load_contract_normalized(HERE / "gt_key_map.json")

    event_map = json.loads(args.map_to_events.read_text(encoding="utf-8")) if args.map_to_events else None
    if event_map is not None:
        # Fail fast: binding to an event id absent from the frozen manifest
        # is exactly the orphan-event defect amendment Y fixes -- never let
        # it silently through to the harness's own guard.
        valid_event_ids = {e["event_id"] for e in manifest_v2["events"]}
        bad = {v1_file: event_id for v1_file, event_id in event_map.items() if event_id not in valid_event_ids}
        if bad:
            raise ValueError(f"--map-to-events names event id(s) absent from MANIFEST_v2.json: {bad}")

    reference_slots, mapped_slots = import_slots(v1_gt, issuer_map, metric_key_to_entry, event_map, contract_normalized)

    ref_path = esef_v2_dir / "machine_v1_reference.json"
    ref_path.write_text(
        json.dumps({"slots": sorted(reference_slots, key=lambda s: s["slot_id"])}, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )

    if mapped_slots:
        merge_ground_truth(esef_v2_dir, mapped_slots)

    print(
        f"import_v1: {len(reference_slots)} v1 row(s) written to machine_v1_reference.json "
        f"({len(mapped_slots)} explicitly mapped into ground_truth_v2.json via --map-to-events, "
        f"{len(reference_slots) - len(mapped_slots)} unmapped, reference-only)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
