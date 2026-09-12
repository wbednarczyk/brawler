#!/usr/bin/env python3
"""Frame builder for ESEF measurement v2 (#331 PR-A, ADR 0112). Stdlib only.

Inventories a pinned snapshot's fetched documents for a set of issuers BY
BYTES (never by stored extraction outcome -- `financial_facts` and
`report_tagged_facts` are never read, see `fetch_documents` below), classifies
each eligible iXBRL instance from filing evidence only, selects the frozen
panel (newest annual + newest interim per issuer/basis in the pinned
language = `floor`; the other language of the same event = `twin_diagnostic`;
earlier fiscal periods = `warmup`), copies the chosen files into
`<out>/corpus/`, and writes `MANIFEST_v2.json` + `review-queue.json`.

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
from datetime import date
from pathlib import Path

import esef_ixbrl as ix

HERE = Path(__file__).resolve().parent
CONSOLIDATED_TOKENS = ("skonsolidowan", "consolidated")
STANDALONE_TOKENS = ("jednostkow", "standalone", "separate")
_LANG_TOKEN_RE = re.compile(r"[_.\-]?(pl|en)[_.\-]")


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


def classify_basis(text_sources: list[str]) -> str:
    for text in text_sources:
        low = (text or "").lower()
        if any(tok in low for tok in CONSOLIDATED_TOKENS):
            return "consolidated"
        if any(tok in low for tok in STANDALONE_TOKENS):
            return "standalone"
    return "unknown"


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


def classify_period_type(months: float, end_month: int) -> str:
    # GPW interim ESEF/iXBRL filings are cumulative year-to-date (owner
    # convention: the current-period column IS the YTD column), so a
    # 9-month duration is always the Q3 cumulative filing -- never bucketed
    # by end_month like the 3-month (standalone-quarter-length) band below.
    if 11 <= months <= 13:
        return "FY"
    if 8 <= months <= 10:
        return "Q3"
    if 5 <= months <= 7:
        return "H1"
    if 2 <= months <= 4:
        return "Q1" if end_month <= 3 else "Q2" if end_month <= 6 else "Q3" if end_month <= 9 else "Q4"
    return "unknown"


def classify_instance(instance: dict, duration_concepts: set[str], doc_title: str, path_hint: str) -> dict:
    """Classifies one parsed instance from filing evidence: primary-statement
    period (longest current duration ending at the latest end date), basis
    (package path/title tokens), language (xml:lang, then filename tokens)."""
    occs = instance["occurrences"]
    candidates = [
        o
        for o in occs
        if o["concept_local"] in duration_concepts
        and not o["dimensions"]
        and o["period"]
        and "start" in o["period"]
        and "end" in o["period"]
    ]
    text_sources = [doc_title, path_hint, instance.get("package_member") or ""]
    basis_scope = classify_basis(text_sources)
    language = classify_language(instance.get("lang"), text_sources)

    if not candidates:
        return {
            "period_type": "unknown",
            "fiscal_year": None,
            "period_start": None,
            "period_end": None,
            "duration_months": None,
            "basis_scope": basis_scope,
            "language": language,
            "unknown_reasons": ["no primary-statement duration concept found"],
        }

    latest_end = max(o["period"]["end"] for o in candidates)
    at_latest = [o for o in candidates if o["period"]["end"] == latest_end]
    chosen = max(at_latest, key=lambda o: months_between(o["period"]["start"], o["period"]["end"]))
    start, end = chosen["period"]["start"], chosen["period"]["end"]
    months = months_between(start, end)
    period_type = classify_period_type(months, int(end.split("-")[1]))
    fiscal_year = int(end.split("-")[0])

    unknown_reasons = []
    if period_type == "unknown":
        unknown_reasons.append(f"irregular duration ({months} months)")
    if basis_scope == "unknown":
        unknown_reasons.append("basis not recoverable from path/title")
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
        "unknown_reasons": unknown_reasons,
    }


# ---------------------------------------------------------------------------
# Selection (deterministic: newest annual + newest interim per issuer/basis
# in the pinned language = floor; the other language of the same fiscal
# period/basis = twin_diagnostic; earlier fiscal periods = warmup, up to 4)
# ---------------------------------------------------------------------------
def _sort_key(c: dict) -> tuple:
    # period_end desc, package-over-loose, then domain date, then sha256 --
    # the pinned tie-break order (decision 10).
    return (c["period_end"] or "", c["is_package"], c.get("fetched_at") or "", c["sha256"])


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

            floor_events = ([annuals[0]] if annuals else []) + ([interims[0]] if interims else [])
            for f in floor_events:
                f["role"] = "floor"
                selected.append(f)
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
                    selected.append(twin)

            floor_keys = {(f["fiscal_year"], f["period_type"]) for f in floor_events}
            remaining = [c for c in pinned if (c["fiscal_year"], c["period_type"]) not in floor_keys]
            for order, c in enumerate(sorted(remaining, key=_sort_key, reverse=True)[:4]):
                c["role"] = "warmup"
                c["warmup_order"] = order
                selected.append(c)
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
            else:
                honest_limitations.append(f"{row['id']}: no iXBRL instance found (non-iXBRL markup)")
            continue

        zero_eligible_per_issuer[ticker] = False
        for instance in parsed["instances"]:
            classification = classify_instance(instance, duration_concepts, row["title"] or "", row["local_path"])
            for reason in classification["unknown_reasons"]:
                honest_limitations.append(f"{row['id']} ({instance.get('package_member') or '<raw>'}): {reason}")
            candidates.append(
                {
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
                    "package_member": instance.get("package_member"),
                    "is_package": ix.is_zip(data),
                    "fetched_at": row["fetched_at"],
                    "document": row,
                    "file_bytes": data,
                    "original_name": Path(row["local_path"]).name,
                    "unknown_reasons": classification["unknown_reasons"],
                }
            )

    for ticker, was_zero in zero_eligible_per_issuer.items():
        if was_zero:
            honest_limitations.append(f"{ticker}: zero eligible iXBRL instances in the fetched corpus")

    parse_failures = [c for c in candidates if c.get("labeling_note")]
    selectable = [c for c in candidates if not c.get("labeling_note")]
    selected = select_events(selectable, pin_language) + parse_failures

    events = []
    review_queue = []
    event_shas: list[str] = []
    vintage_counter: dict[tuple, int] = {}

    for c in sorted(selected, key=lambda c: (c["issuer_id"], c["sha256"])):
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
        if c["role"] == "warmup":
            event["warmup_order"] = c["warmup_order"]
        if c.get("labeling_note"):
            event["labeling_note"] = c["labeling_note"]
        events.append(event)

        if c["unknown_reasons"]:
            review_queue.append({"event_id": event_id, "reasons": c["unknown_reasons"]})

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
        json.dumps(sorted(review_queue, key=lambda r: r["event_id"]), indent=2, ensure_ascii=False) + "\n",
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
