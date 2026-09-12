#!/usr/bin/env python3
"""Frame builder for ESEF measurement v2 (#331 PR-A, ADR 0112, amendment 1).
Stdlib only.

Inventories a pinned snapshot's fetched documents for a set of issuers BY
BYTES (never by stored extraction outcome -- `financial_facts` and
`report_tagged_facts` are never read, see `fetch_documents` below).

Event boundary = the whole package or loose instance (amendment A): ONE
candidate per manifest file, not per iXBRL member. Its headline
classification (period/basis/language, used only for panel selection) comes
from the file's PRIMARY member -- the one carrying the primary-statement
duration evidence -- pooled across every member so a comparative-year
context in one member can help classify a shorter current-period duration in
another. The labeler (`label_esef_v2.py`) still emits occurrences/slots for
EVERY member with its own per-member basis/language.

Selects the frozen panel (newest annual + newest interim per issuer/basis in
the pinned language = `floor`; the other language of the same fiscal period
in a SEPARATE file = `twin_diagnostic`; earlier fiscal periods AND
corrections sharing a period = `warmup`, up to 4), copies the chosen files
into `<out>/corpus/`, and writes `MANIFEST_v2.json` + `review-queue.json`
(every unresolved candidate is queued BEFORE selection, never dropped --
amendment L / astra r1 finding 9).

Usage:
    python3 build_frame.py --snapshot <sqlite path> --data-dir <report_documents dir> \\
        --issuers <tickers.csv|acceptance-list-file> --out <esef-v2 dir> \\
        [--select newest-annual-interim] [--pin-language pl]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sqlite3
from collections import Counter
from datetime import date
from pathlib import Path

import esef_ixbrl as ix

HERE = Path(__file__).resolve().parent
CONSOLIDATED_TOKENS = ("skonsolidowan", "consolidated")
STANDALONE_TOKENS = ("jednostkow", "standalone", "separate")
_LANG_TOKEN_RE = re.compile(r"[_.\-]?(pl|en)[_.\-]")
_TAG_RE = re.compile(rb"<[^>]+>")
COVER_PAGE_BYTE_WINDOW = 8000  # generous raw-byte window read before stripping tags
COVER_PAGE_TEXT_LIMIT = 2000  # amendment L: "first 2 KB of the instance body after tags stripped"


# ---------------------------------------------------------------------------
# Database access -- report_documents/companies ONLY (ADR 0112 dec. 10: the
# frame classifies from filing bytes, never from a stored extraction
# outcome). A unit test wraps the connection with `set_trace_callback` and
# asserts every statement this module issues stays within that boundary.
# ---------------------------------------------------------------------------
def open_database(snapshot_path: str) -> sqlite3.Connection:
    return sqlite3.connect(f"file:{snapshot_path}?mode=ro", uri=True)


def fetch_documents(conn: sqlite3.Connection, tickers: list[str]) -> list[dict]:
    placeholders = ",".join("?" for _ in tickers)
    cur = conn.execute(
        f"""SELECT rd.id, rd.local_path, rd.title, rd.url, rd.content_type, rd.content_hash,
                   rd.fetched_at, c.ticker, c.exchange, c.display_name
            FROM report_documents rd
            JOIN companies c ON rd.company_id = c.id
            WHERE rd.fetch_status = 'fetched' AND c.ticker IN ({placeholders})
            ORDER BY c.ticker, rd.id""",
        tickers,
    )
    cols = [d[0] for d in cur.description]
    return [dict(zip(cols, row)) for row in cur.fetchall()]


# ---------------------------------------------------------------------------
# Classification from filing evidence only
# ---------------------------------------------------------------------------
def load_key_map_concepts(key_map_path: Path) -> tuple[set[str], set[str]]:
    data = json.loads(key_map_path.read_text(encoding="utf-8"))
    duration = {e["concept"] for e in data["entries"] if e["period_nature"] == "duration"}
    instant = {e["concept"] for e in data["entries"] if e["period_nature"] == "instant"}
    return duration, instant


def _basis_from_text(text: str) -> str:
    low = (text or "").lower()
    if any(tok in low for tok in CONSOLIDATED_TOKENS):
        return "consolidated"
    if any(tok in low for tok in STANDALONE_TOKENS):
        return "standalone"
    return "unknown"


def cover_page_text(member_bytes: bytes) -> str:
    """First ~2KB of the instance body with markup tags stripped (amendment
    L basis precedence's last resort before `unknown`)."""
    stripped = _TAG_RE.sub(b" ", member_bytes[:COVER_PAGE_BYTE_WINDOW])
    return stripped.decode("utf-8", errors="ignore")[:COVER_PAGE_TEXT_LIMIT]


def classify_basis(member_hint: str, outer_hints: list[str], cover_text: str) -> tuple[str, bool]:
    """(basis_scope, contradiction). Amendment L precedence: member path ->
    package/outer path+title -> cover-page text -> `unknown`. A contradiction
    (member and outer evidence both resolve, and disagree) is never silently
    resolved by precedence -- it becomes `unknown` and is flagged for the
    review queue (astra r1 finding 8: a consolidated package title must not
    override a clearly standalone member path)."""
    member_basis = _basis_from_text(member_hint)
    outer_basis = _basis_from_text(" ".join(outer_hints))
    if member_basis != "unknown" and outer_basis != "unknown" and member_basis != outer_basis:
        return "unknown", True
    if member_basis != "unknown":
        return member_basis, False
    if outer_basis != "unknown":
        return outer_basis, False
    return _basis_from_text(cover_text), False


def classify_language(xml_lang: str | None, text_sources: list[str]) -> str:
    if xml_lang:
        code = xml_lang.split("-")[0].lower()
        if code in ("pl", "en"):
            return code
    for text in text_sources:
        low = f".{(text or '').lower()}."
        match = _LANG_TOKEN_RE.search(low)
        if match:
            return match.group(1)
    return "unknown"


def months_between(start: str, end: str) -> int:
    # Day-count based (not calendar-component subtraction): a period's
    # *length* is what buckets it into FY/H1/Q3(9M)/quarter, so
    # round(days/30.4368) -- the average month length -- classifies a true
    # ~12/9/6/3-month span correctly regardless of which day-of-month it
    # starts/ends on, and genuinely irregular spans fall outside every ±1
    # tolerance band below and land on "unknown" rather than being coerced
    # into the nearest bucket.
    days = (date.fromisoformat(end) - date.fromisoformat(start)).days
    return round(days / 30.4368)


def classify_period_type(months: float, end_month: int, fiscal_start_month: int = 1) -> str:
    """`fiscal_start_month` (1-12) is the fiscal year's own first month,
    established from evidence elsewhere in the same filing (amendment L: "a
    12-month duration ending in March is FY with a March year-end; quarters
    counted from the fiscal-year start") -- defaults to January (calendar
    year) when no other evidence is available, which reproduces the old
    calendar-quarter behavior."""
    if 11 <= months <= 13:
        return "FY"
    if 8 <= months <= 10:
        return "Q3"
    if 5 <= months <= 7:
        return "H1"
    if 2 <= months <= 4:
        offset = (end_month - fiscal_start_month) % 12
        return "Q1" if offset <= 3 else "Q2" if offset <= 6 else "Q3" if offset <= 9 else "Q4"
    return "unknown"


def classify_file(instances: list[dict], duration_concepts: set[str], doc_title: str, outer_path: str) -> dict:
    """Classifies one FILE (amendment A: the event boundary is the whole
    package) from filing evidence pooled across every member: primary period
    (longest current duration ending at the latest end date, chosen from
    every member's duration-concept occurrences), fiscal-year start (the
    most common start month among ALL duration candidates in the file --
    cumulative FY/H1/9M periods all start there), basis (member -> outer ->
    cover-page precedence, astra r1 finding 8), language (xml:lang -> filename)."""
    pooled: list[tuple[dict, dict]] = [
        (instance, occ)
        for instance in instances
        for occ in instance["occurrences"]
        if occ["concept_local"] in duration_concepts
        and not occ["dimensions"]
        and occ["period"]
        and "start" in occ["period"]
        and "end" in occ["period"]
    ]

    if not pooled:
        primary_instance = instances[0]
        text_sources = [doc_title, outer_path, primary_instance.get("package_member") or ""]
        cover_text = cover_page_text(primary_instance.get("raw_bytes") or b"")
        basis_scope, contradiction = classify_basis(
            primary_instance.get("package_member") or "", [doc_title, outer_path], cover_text
        )
        language = classify_language(primary_instance.get("lang"), text_sources)
        reasons = ["no primary-statement duration concept found in any package member"]
        if basis_scope == "unknown":
            reasons.append(
                "basis contradicts between member and outer evidence" if contradiction else "basis not recoverable from path/title/cover page"
            )
        if language == "unknown":
            reasons.append("language not recoverable from xml:lang/filename")
        return {
            "period_type": "unknown",
            "fiscal_year": None,
            "period_start": None,
            "period_end": None,
            "duration_months": None,
            "basis_scope": basis_scope,
            "language": language,
            "primary_member": primary_instance.get("package_member"),
            "unknown_reasons": reasons,
        }

    latest_end = max(occ["period"]["end"] for _inst, occ in pooled)
    at_latest = [(inst, occ) for inst, occ in pooled if occ["period"]["end"] == latest_end]
    primary_instance, chosen = max(at_latest, key=lambda pair: months_between(pair[1]["period"]["start"], pair[1]["period"]["end"]))
    start, end = chosen["period"]["start"], chosen["period"]["end"]
    months = months_between(start, end)
    fiscal_start_month = Counter(int(occ["period"]["start"].split("-")[1]) for _inst, occ in pooled).most_common(1)[0][0]
    period_type = classify_period_type(months, int(end.split("-")[1]), fiscal_start_month)
    fiscal_year = int(end.split("-")[0])

    text_sources = [doc_title, outer_path, primary_instance.get("package_member") or ""]
    cover_text = cover_page_text(primary_instance.get("raw_bytes") or b"")
    basis_scope, contradiction = classify_basis(primary_instance.get("package_member") or "", [doc_title, outer_path], cover_text)
    language = classify_language(primary_instance.get("lang"), text_sources)

    unknown_reasons = []
    if period_type == "unknown":
        unknown_reasons.append(f"irregular duration ({months} months)")
    if basis_scope == "unknown":
        unknown_reasons.append(
            "basis contradicts between member and outer evidence" if contradiction else "basis not recoverable from path/title/cover page"
        )
    if language == "unknown":
        unknown_reasons.append("language not recoverable from xml:lang/filename")

    return {
        "period_type": period_type,
        "fiscal_year": fiscal_year,
        "period_start": start,
        "period_end": end,
        "duration_months": months,
        "basis_scope": basis_scope,
        "language": language,
        "primary_member": primary_instance.get("package_member"),
        "unknown_reasons": unknown_reasons,
    }


# ---------------------------------------------------------------------------
# Selection (deterministic: newest annual + newest interim per issuer/basis
# in the pinned language = floor; the other language of the same fiscal
# period in a separate file = twin_diagnostic; earlier fiscal periods AND
# corrections sharing a period = warmup, up to 4 -- astra r1 finding 10:
# a correction must become a later vintage, never be discarded)
# ---------------------------------------------------------------------------
def _sort_key(c: dict) -> tuple:
    # Amendment L: domain date = the filing's own date (context period_end);
    # `fetched_at` is only a tie-break, `is_package` after that, sha256 last.
    return (c["period_end"] or "", c.get("fetched_at") or "", c["is_package"], c["sha256"])


def select_events(candidates: list[dict], pin_language: str) -> list[dict]:
    by_issuer: dict[str, list[dict]] = {}
    for c in candidates:
        by_issuer.setdefault(c["issuer_id"], []).append(c)

    selected: list[dict] = []
    for _issuer_id, items in by_issuer.items():
        by_basis: dict[str, list[dict]] = {}
        for c in items:
            by_basis.setdefault(c["basis_scope"], []).append(c)

        for _basis, basis_items in by_basis.items():
            pinned = [c for c in basis_items if c["language"] == pin_language and c["period_type"] != "unknown"]
            annuals = sorted((c for c in pinned if c["period_type"] == "FY"), key=_sort_key, reverse=True)
            interims = sorted((c for c in pinned if c["period_type"] not in ("FY", "unknown")), key=_sort_key, reverse=True)

            group_selected: list[dict] = []
            floor_events = ([annuals[0]] if annuals else []) + ([interims[0]] if interims else [])
            for f in floor_events:
                f["role"] = "floor"
                group_selected.append(f)
                twins = [
                    c
                    for c in basis_items
                    if c is not f
                    and c["language"] != pin_language
                    and c["fiscal_year"] == f["fiscal_year"]
                    and c["period_type"] == f["period_type"]
                ]
                if twins:
                    twin = sorted(twins, key=_sort_key, reverse=True)[0]
                    twin["role"] = "twin_diagnostic"
                    group_selected.append(twin)

            # Earlier fiscal periods AND corrections sharing the floor's own
            # period (excluded only by object identity, never by period key
            # -- a same-period correction must stay eligible as a warmup
            # vintage) fill up to 4 warmup slots, newest-first.
            already_selected_ids = {id(c) for c in group_selected}
            remaining = [c for c in pinned if id(c) not in already_selected_ids]
            for order, c in enumerate(sorted(remaining, key=_sort_key, reverse=True)[:4]):
                c["role"] = "warmup"
                c["warmup_order"] = order
                group_selected.append(c)

            selected.extend(group_selected)
    return selected


# ---------------------------------------------------------------------------
# Corpus copy + manifest assembly
# ---------------------------------------------------------------------------
def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def copy_into_corpus(corpus_dir: Path, original_name: str, data: bytes, sha256: str) -> str:
    corpus_dir.mkdir(parents=True, exist_ok=True)
    dest = corpus_dir / original_name
    if dest.exists() and sha256_hex(dest.read_bytes()) != sha256:
        stem, suffix = Path(original_name).stem, Path(original_name).suffix
        dest = corpus_dir / f"{stem}-{sha256[:8]}{suffix}"
    if not dest.exists():
        dest.write_bytes(data)
    return dest.name


def registry_hash(event_shas: list[str]) -> str:
    return hashlib.sha256("\n".join(sorted(event_shas)).encode("utf-8")).hexdigest()


def build_frame(
    snapshot_path: str,
    data_dir: str,
    tickers: list[str],
    out_dir: str,
    pin_language: str = "pl",
) -> dict:
    key_map_path = HERE / "gt_key_map.json"
    duration_concepts, _instant_concepts = load_key_map_concepts(key_map_path)

    conn = open_database(snapshot_path)
    try:
        rows = fetch_documents(conn, tickers)
    finally:
        conn.close()

    out = Path(out_dir)
    corpus_dir = out / "corpus"
    data_root = Path(data_dir)

    issuer_ids: dict[str, str] = {}
    issuers_meta: dict[str, dict] = {}
    for ticker in sorted({r["ticker"] for r in rows}):
        issuer_ids[ticker] = f"iss_{len(issuer_ids) + 1:02d}"

    candidates: list[dict] = []
    honest_limitations: list[str] = []
    zero_eligible_per_issuer = {t: True for t in issuer_ids}
    # Astra r1 finding 9: every unresolved candidate is queued BEFORE
    # selection -- keyed by sha256 so it survives regardless of whether
    # selection later picks it as an event.
    review_queue_index: dict[str, dict] = {}

    for row in rows:
        ticker = row["ticker"]
        issuer_id = issuer_ids[ticker]
        issuers_meta[issuer_id] = {
            "issuer_id": issuer_id,
            "ticker": ticker,
            "exchange": row["exchange"],
            "display_name": row["display_name"],
        }
        if not row["local_path"]:
            honest_limitations.append(f"{row['id']}: fetched but no local_path recorded")
            continue
        file_path = data_root / row["local_path"]
        if not file_path.exists():
            honest_limitations.append(f"{row['id']}: local_path {row['local_path']} missing on disk")
            continue

        data = file_path.read_bytes()
        sha = sha256_hex(data)
        parsed = ix.parse_document(data)

        if not parsed["instances"]:
            looks_eligible = (row["content_type"] or "").lower().find("xml") >= 0 or file_path.suffix.lower() in (
                ".xhtml",
                ".html",
                ".zip",
            )
            if looks_eligible:
                candidates.append(
                    {
                        "issuer_id": issuer_id,
                        "role": "floor",
                        "language": "unknown",
                        "basis_scope": "unknown",
                        "period_type": "unknown",
                        "fiscal_year": None,
                        "period_start": None,
                        "period_end": None,
                        "duration_months": None,
                        "sha256": sha,
                        "bytes": len(data),
                        "package_member": None,
                        "is_package": ix.is_zip(data),
                        "fetched_at": row["fetched_at"],
                        "document": row,
                        "file_bytes": data,
                        "original_name": Path(row["local_path"]).name,
                        "labeling_note": "no iXBRL instance found in an eligible-looking file (parse failure)",
                        "unknown_reasons": ["parse failure"],
                    }
                )
                review_queue_index[sha] = {
                    "issuer_id": issuer_id,
                    "sha256": sha,
                    "package_member": None,
                    "reasons": ["parse failure: no iXBRL instance found in an eligible-looking file"],
                }
            else:
                honest_limitations.append(f"{row['id']}: no iXBRL instance found (non-iXBRL markup)")
            continue

        zero_eligible_per_issuer[ticker] = False
        # Amendment A: event = the WHOLE FILE, not one candidate per member.
        # `raw_bytes` lets classify_file read the cover page of whichever
        # member turns out to be primary; it never leaves this function.
        member_bytes_by_name = dict(ix.zip_members(data)) if ix.is_zip(data) else {None: data}
        instances_with_bytes = list(parsed["instances"])
        for inst in instances_with_bytes:
            inst["raw_bytes"] = member_bytes_by_name.get(inst.get("package_member"))

        classification = classify_file(instances_with_bytes, duration_concepts, row["title"] or "", row["local_path"])
        for reason in classification["unknown_reasons"]:
            honest_limitations.append(f"{row['id']} ({classification['primary_member'] or '<raw>'}): {reason}")
        candidate = {
            "issuer_id": issuer_id,
            "language": classification["language"],
            "basis_scope": classification["basis_scope"],
            "period_type": classification["period_type"],
            "fiscal_year": classification["fiscal_year"],
            "period_start": classification["period_start"],
            "period_end": classification["period_end"],
            "duration_months": classification["duration_months"],
            "sha256": sha,
            "bytes": len(data),
            "package_member": classification["primary_member"],
            "is_package": ix.is_zip(data),
            "fetched_at": row["fetched_at"],
            "document": row,
            "file_bytes": data,
            "original_name": Path(row["local_path"]).name,
            "unknown_reasons": classification["unknown_reasons"],
        }
        candidates.append(candidate)
        if classification["unknown_reasons"]:
            review_queue_index[sha] = {
                "issuer_id": issuer_id,
                "sha256": sha,
                "package_member": classification["primary_member"],
                "reasons": classification["unknown_reasons"],
            }

    for ticker, was_zero in zero_eligible_per_issuer.items():
        if was_zero:
            honest_limitations.append(f"{ticker}: zero eligible iXBRL instances in the fetched corpus")

    parse_failures = [c for c in candidates if c.get("labeling_note")]
    selectable = [c for c in candidates if not c.get("labeling_note")]
    selected = select_events(selectable, pin_language) + parse_failures

    events = []
    event_shas: list[str] = []
    # Vintage numbering follows chronological (domain-date) order, not
    # processing order -- astra r1 finding 10: "vintage 1 + count of EARLIER
    # eligible instances" requires actual time order, and a same-period
    # correction (kept as a warmup above, never discarded) must land at a
    # HIGHER vintage than the event it corrects.
    vintage_counter: dict[tuple, int] = {}
    for c in sorted(
        selected,
        key=lambda c: (c["issuer_id"], c["fiscal_year"] or 0, c["period_type"], c["language"], c["basis_scope"], _sort_key(c)),
    ):
        key = (c["issuer_id"], c["fiscal_year"], c["period_type"], c["language"], c["basis_scope"])
        vintage = vintage_counter.get(key, 0) + 1
        vintage_counter[key] = vintage

        period_label = f"FY{c['fiscal_year']}" if c["fiscal_year"] is not None else "unknown"
        event_id = f"{c['issuer_id']}/{period_label}/{c['period_type']}/{c['language']}/{c['basis_scope']}/v{vintage}"
        dest_name = copy_into_corpus(corpus_dir, c["original_name"], c["file_bytes"], c["sha256"])
        event_shas.append(c["sha256"])

        event = {
            "event_id": event_id,
            "issuer_id": c["issuer_id"],
            "role": c["role"],
            "language": c["language"],
            "basis_scope": c["basis_scope"],
            "vintage": vintage,
            "warmup_order": c.get("warmup_order", 0),  # astra r1 finding 6: present on every event
            "labeled_period": {
                "fiscal_year": c["fiscal_year"],
                "period_type": c["period_type"],
                "period_end": c["period_end"],
                "period_start": c["period_start"],
                "duration_months": c["duration_months"],
            },
            "file": {
                "name": dest_name,
                "sha256": c["sha256"],
                "bytes": c["bytes"],
                "package_member": c["package_member"],
            },
            "document": {
                "id": c["document"]["id"],
                "title": c["document"]["title"],
                "url": c["document"]["url"],
                "content_type": c["document"]["content_type"],
                "content_hash": c["document"]["content_hash"],
            },
        }
        if c.get("labeling_note"):
            event["labeling_note"] = c["labeling_note"]
        events.append(event)

        entry = review_queue_index.get(c["sha256"])
        if entry is not None:
            entry["event_id"] = event_id

    manifest = {
        "manifest_version": 1,
        "snapshot": {"source": Path(snapshot_path).name, "taken_at": date.today().isoformat()},
        "registry_hash": registry_hash(event_shas),
        "issuers": [issuers_meta[iid] for iid in sorted(issuers_meta)],
        "events": sorted(events, key=lambda e: e["event_id"]),
        "honest_limitations": sorted(set(honest_limitations)),
    }

    out.mkdir(parents=True, exist_ok=True)
    (out / "MANIFEST_v2.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    (out / "review-queue.json").write_text(
        json.dumps(sorted(review_queue_index.values(), key=lambda r: (r["issuer_id"], r["sha256"])), indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    return manifest


def parse_issuers_arg(value: str) -> list[str]:
    path = Path(value)
    if path.exists():
        lines = [ln.strip() for ln in path.read_text(encoding="utf-8").splitlines()]
        return [ln for ln in lines if ln and not ln.startswith("#")]
    return [t.strip() for t in value.split(",") if t.strip()]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--snapshot", required=True)
    parser.add_argument("--data-dir", required=True)
    parser.add_argument("--issuers", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--select", default="newest-annual-interim", choices=["newest-annual-interim"])
    parser.add_argument("--pin-language", default="pl")
    args = parser.parse_args(argv)

    tickers = parse_issuers_arg(args.issuers)
    manifest = build_frame(args.snapshot, args.data_dir, tickers, args.out, args.pin_language)
    print(f"build_frame: {len(manifest['events'])} events across {len(manifest['issuers'])} issuers -> {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
