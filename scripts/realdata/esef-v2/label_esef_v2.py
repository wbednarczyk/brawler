#!/usr/bin/env python3
"""ESEF ground-truth labeler v2 (#331 PR-A, ADR 0112, amendment 2). Stdlib
only.

Event boundary = the whole package (amendment A): for every event in
`MANIFEST_v2.json`, parses EVERY `.xhtml`/`.html` member of its corpus file
(not just the manifest's primary member) with `esef_ixbrl`, emitting every
fact as a raw occurrence row (`occurrences_v2.json`, no panel filter -- the
panel is a scoring-time concern) with its own `package_member`, `basis` and
`language` derived per-member, and derives normalized `ground_truth_v2.json`
slots for EVERY IFRS concept (mapped through `gt_key_map.json` or not --
`mapped: true|false`, amendment Q).

Two occurrences agreeing on package_member, concept, attribution, unit,
currency and period merge into one `machine` slot (Decimal-normalized value
agreement, never a string compare); disagreeing makes the slot `unverified`
with every distinct value kept in `resolution_ref`. A slot is derived only
when the occurrence's dimension set is EMPTY or consists solely of the
attribution axis member (parent/NCI) -- any other axis (segment, geography,
class), alone or alongside a parent/NCI member, keeps the occurrence as
evidence only (amendment Z; `dimensional_occurrences` counts these). A
concept whose expanded namespace is not the recognized IFRS `ifrs-full`
family is treated as an unmapped extension concept for slot purposes,
however its local name reads (amendment M).

Member language is carried on every occurrence and slot (amendment S):
members in the pinned language are the `floor` population; other-language
members' slots are `twin_diagnostic` population INSIDE the same event
(excluded from floor denominators). A member whose OWN basis evidence
contradicts the event's headline basis keeps `basis: unknown` and is queued
for review -- the headline basis is never substituted over a contradiction
(amendment T). A member with a parse error, or a numeric fact with no
resolvable unit, is queued and counted rather than silently vanishing
(amendment U). Every occurrence's fiscal year/period type (instants
included) is classified against the frame's persisted `fiscal_start_month`,
never a re-derived or defaulted calendar (amendment V).

Usage:
    python3 label_esef_v2.py --esef-v2-dir <dir> [--pin-language pl]
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

# The 2024+ ESEF taxonomies publish under https://xbrl.ifrs.org (older under http://) — real-data run 2026-09-12.
IFRS_NAMESPACE_RE = re.compile(r"^\{https?://xbrl\.ifrs\.org/taxonomy/[^/]+/ifrs-full\}")

CASH_FLOW_CONCEPTS = {
    "CashFlowsFromUsedInOperatingActivities",
    "CashFlowsFromUsedInInvestingActivities",
    "CashFlowsFromUsedInFinancingActivities",
}


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


def classify_period(period: dict | None, fiscal_start_month: int | None) -> tuple:
    """(fiscal_year, period_type, period_start, period_end, duration_months)
    for ONE occurrence's own period -- comparative-year facts in the same
    filing get their own (fiscal_year, period_type), distinct from the
    event's labeled current period; that is why `slot_id` carries them.

    `fiscal_start_month` is the frame's PERSISTED calendar (amendment V) --
    used for instants too: a March year-end balance sheet is FY, not Q1,
    when the fiscal year starts in April. Defaults to January (calendar
    year) only when the frame recorded none (an `unknown`-period event)."""
    fsm = fiscal_start_month or 1
    if not period:
        return None, "unknown", None, None, None
    if "instant" in period:
        end = period["instant"]
        month = int(end.split("-")[1])
        months_into_year = (month - fsm) % 12 + 1
        period_type = {12: "FY", 6: "H1", 3: "Q1", 9: "Q3"}.get(months_into_year, "unknown")
        return int(end.split("-")[0]), period_type, None, end, None
    start, end = period.get("start"), period.get("end")
    if not start or not end:
        return None, "unknown", start, end, None
    months = bf.months_between(start, end)
    period_type = bf.classify_period_type(months, int(end.split("-")[1]), fsm)
    return int(end.split("-")[0]), period_type, start, end, months


def currency_from_unit(unit_str: str | None) -> str | None:
    if not unit_str:
        return None
    code = unit_str.split("/")[0].split(":")[-1]
    return code if len(code) == 3 and code.isalpha() and code.isupper() else None


def resolve_attribution(base_attribution: str, dimensions: list[dict]) -> str | None:
    """Concept-level attribution (from the key map, or `total` for an
    unmapped concept) for a PRIMARY (no dimensions) occurrence, OR for an
    occurrence whose dimension set is EXACTLY one member carrying explicit
    NCI/parent semantics. Amendment Z (astra r2 finding 5): any OTHER
    dimension set -- more than one axis, or a single axis that is not
    parent/NCI, INCLUDING a parent/NCI member accompanied by a further
    segment/geography/class axis -- returns `None`: not a scoring slot, only
    occurrence evidence. The old rule returned on the FIRST parent/NCI match
    and silently ignored any additional axis."""
    if not dimensions:
        return base_attribution
    if len(dimensions) == 1:
        member_local = (dimensions[0].get("member") or "").split(":")[-1]
        if "NoncontrollingInterest" in member_local:
            return "nci"
        if "Parent" in member_local:
            return "owners_of_parent"
    return None


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


def emit_occurrences(event: dict, parsed_occurrences: list[dict], language: str, start_index: int) -> list[dict]:
    rows = []
    for offset, occ in enumerate(parsed_occurrences):
        n = start_index + offset
        rows.append(
            {
                "occurrence_id": f"{event['event_id']}#{n}",
                "event_id": event["event_id"],
                **occ,
                "language": language,  # amendment S: never discarded
                "verification": "machine",
                "resolution_ref": None,
            }
        )
    return rows


def classify_member_evidence(event: dict, member_name: str | None, member_bytes: bytes, lang: str | None) -> dict:
    """Per-member (basis, language, contradiction), amendment S/T: language
    is always kept; a member basis that CONTRADICTS the event's headline
    basis stays `unknown` and is flagged -- the headline is substituted only
    when the member gave NO signal at all (not when it disagreed)."""
    outer_hints = [event["document"].get("title") or "", event["file"]["name"]]
    cover_text = bf.cover_page_text(member_bytes)
    member_basis, contradiction_vs_outer = bf.classify_basis(member_name or "", outer_hints, cover_text)

    contradicts_headline = (
        member_basis != "unknown" and event["basis_scope"] != "unknown" and member_basis != event["basis_scope"]
    )
    if contradicts_headline or contradiction_vs_outer:
        basis, contradiction = "unknown", True
    elif member_basis != "unknown":
        basis, contradiction = member_basis, False
    else:
        basis, contradiction = event["basis_scope"], False  # no member-level signal at all -- fall back

    language = bf.classify_language(lang, [member_name or "", event["file"]["name"]])
    if language == "unknown":
        language = event["language"]
    return {"basis": basis, "language": language, "contradiction": contradiction}


def normalized_by_for_slot(concept_local: str, window: str, period_type: str, contract_normalized_ids: set[str]) -> list[str]:
    """Amendment Q: `normalized_by` records which `contract_normalized`
    rule(s) from `gt_key_map.json` apply to this slot, so the harness can
    remove the convention-resolved population from both sides for the
    sensitivity score. `cumulative_context_to_flow` applies to every interim
    (non-FY) duration -- GPW interim filings are cumulative YTD by
    convention; `cash_flow_outflow_sign` applies to the three cash-flow
    concepts, whose sign convention is normalized regardless of the actual
    sign of a given instance."""
    ids = []
    if "cumulative_context_to_flow" in contract_normalized_ids and window == "flow" and period_type not in ("FY", "unknown"):
        ids.append("cumulative_context_to_flow")
    if "cash_flow_outflow_sign" in contract_normalized_ids and concept_local in CASH_FLOW_CONCEPTS:
        ids.append("cash_flow_outflow_sign")
    return ids


def derive_slots(
    event: dict,
    occurrences: list[dict],
    concept_info: dict[str, dict],
    member_basis: dict[str | None, str],
    pin_language: str,
    contract_normalized_ids: set[str],
) -> tuple[list[dict], int]:
    """Returns (slots, dimensional_occurrences) -- the latter counts
    occurrences excluded from slot derivation by amendment Z's dimension
    rule (amendment Z / astra r2 finding 5)."""
    groups: dict[tuple, list[dict]] = {}
    dimensional_occurrences = 0
    fiscal_start_month = event["labeled_period"].get("fiscal_start_month")

    for occ in occurrences:
        if occ["parse_status"] != "ok" or occ["value"] is None or not occ["period"]:
            continue
        if not is_ifrs_concept(occ["concept_qname"]):
            continue
        info = concept_info.get(occ["concept_local"])
        base_attribution = info["attribution"] if info else "total"  # amendment Q: unmapped concepts default to total
        attribution = resolve_attribution(base_attribution, occ["dimensions"])
        if attribution is None:
            if occ["dimensions"]:
                dimensional_occurrences += 1
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
        fiscal_year, period_type, period_start, period_end, duration_months = classify_period(occs[0]["period"], fiscal_start_month)
        basis = member_basis.get(package_member, event["basis_scope"])
        info = concept_info.get(concept_local)
        language = occs[0].get("language", "unknown")
        population = "floor" if language == pin_language else "twin_diagnostic"

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
                "mapped": info is not None,  # amendment Q: every IFRS concept gets a slot, mapped or not
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
                "language": language,  # amendment S
                "population": population,  # amendment S: floor vs within-package twin_diagnostic
                "normalized_by": normalized_by_for_slot(concept_local, window, period_type, contract_normalized_ids),
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
    return slots, dimensional_occurrences


def _append_review_queue(esef_v2_dir: Path, new_entries: list[dict]) -> None:
    """Merges new entries into the shared review-queue.json (build_frame.py
    owns its initial contents; label_esef_v2.py adds member-level findings
    -- amendment T/U). Keyed by (event_id, package_member) to stay idempotent
    across re-runs."""
    if not new_entries:
        return
    path = esef_v2_dir / "review-queue.json"
    existing = json.loads(path.read_text(encoding="utf-8")) if path.exists() else []
    by_key = {(e.get("event_id"), e.get("package_member")): e for e in existing}
    for entry in new_entries:
        key = (entry.get("event_id"), entry.get("package_member"))
        if key in by_key:
            by_key[key]["reasons"] = sorted(set(by_key[key].get("reasons", [])) | set(entry.get("reasons", [])))
        else:
            by_key[key] = entry
    path.write_text(
        json.dumps(sorted(by_key.values(), key=lambda e: (e.get("issuer_id") or "", str(e.get("event_id")))), indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )


def label_esef_v2(esef_v2_dir: str, key_map_path: Path | None = None, pin_language: str = "pl") -> dict:
    out = Path(esef_v2_dir)
    manifest = json.loads((out / "MANIFEST_v2.json").read_text(encoding="utf-8"))
    key_map = load_key_map(key_map_path or (HERE / "gt_key_map.json"))
    concept_info = {e["concept"]: e for e in key_map["entries"]}
    contract_normalized_ids = {e["id"] for e in key_map.get("contract_normalized", [])}

    all_occurrences: list[dict] = []
    all_slots: list[dict] = []
    unresolved_occurrences = 0
    unresolved_members = 0
    dimensional_occurrences = 0
    new_review_entries: list[dict] = []

    for event in manifest["events"]:
        members = read_event_members(out / "corpus", event["file"])
        event_occurrences: list[dict] = []
        member_basis: dict[str | None, str] = {}

        for member_name, member_bytes in members:
            parsed = ix.parse_instance(member_bytes, member_name)
            if parsed.get("parse_error"):
                # Amendment U / astra r2 finding 9: a member-level parse
                # failure alongside a valid primary member used to vanish
                # silently (zero unresolved occurrences to show for it).
                unresolved_members += 1
                new_review_entries.append(
                    {
                        "issuer_id": event["issuer_id"],
                        "event_id": event["event_id"],
                        "package_member": member_name,
                        "reasons": [f"member parse error: {parsed['parse_error']}"],
                    }
                )
                continue

            evidence = classify_member_evidence(event, member_name, member_bytes, parsed.get("lang"))
            member_basis[member_name] = evidence["basis"]
            if evidence["contradiction"]:
                new_review_entries.append(
                    {
                        "issuer_id": event["issuer_id"],
                        "event_id": event["event_id"],
                        "package_member": member_name,
                        "reasons": ["member basis contradicts the event's headline basis"],
                    }
                )
            rows = emit_occurrences(event, parsed["occurrences"], evidence["language"], start_index=len(event_occurrences) + 1)
            event_occurrences.extend(rows)

        unresolved_occurrences += sum(1 for o in event_occurrences if o["parse_status"] != "ok")
        all_occurrences.extend(event_occurrences)
        slots, dim_excluded = derive_slots(event, event_occurrences, concept_info, member_basis, pin_language, contract_normalized_ids)
        all_slots.extend(slots)
        dimensional_occurrences += dim_excluded

    _append_review_queue(out, new_review_entries)

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
    return {
        "ground_truth": ground_truth_doc,
        "unresolved_occurrences": unresolved_occurrences,
        "unresolved_members": unresolved_members,
        "dimensional_occurrences": dimensional_occurrences,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--esef-v2-dir", required=True)
    parser.add_argument("--pin-language", default="pl")
    args = parser.parse_args(argv)
    result = label_esef_v2(args.esef_v2_dir, pin_language=args.pin_language)
    print(
        f"label_esef_v2: {len(result['ground_truth']['slots'])} ground-truth slots, "
        f"{result['unresolved_occurrences']} unresolved occurrences, "
        f"{result['unresolved_members']} unresolved members, "
        f"{result['dimensional_occurrences']} dimensional-only occurrences -> {args.esef_v2_dir}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
