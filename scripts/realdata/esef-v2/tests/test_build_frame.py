import json
import sqlite3
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import build_frame as bf
from ixbrl_fixtures import context, make_instance, non_fraction, unit, zip_package


def _make_db(tmp_path: Path, *, with_financial_facts_row: bool = True) -> Path:
    db_path = tmp_path / "snapshot.sqlite3"
    conn = sqlite3.connect(db_path)
    conn.executescript(
        """
        CREATE TABLE companies (id TEXT PRIMARY KEY, exchange TEXT, ticker TEXT, display_name TEXT);
        CREATE TABLE report_documents (
            id TEXT PRIMARY KEY, company_id TEXT, local_path TEXT, title TEXT, url TEXT,
            content_type TEXT, content_hash TEXT, fetch_status TEXT, fetched_at TEXT
        );
        CREATE TABLE financial_facts (id TEXT PRIMARY KEY, value TEXT);
        """
    )
    conn.execute("INSERT INTO companies VALUES ('c1','GPW','ZZZ','ZZZ S.A.')")
    conn.execute(
        "INSERT INTO report_documents VALUES ('doc1','c1','zzz_fy2025_pl.xhtml','ZZZ consolidated FY2025',"
        "'http://example.org/zzz','application/xhtml+xml',NULL,'fetched','2026-03-01T00:00:00Z')"
    )
    if with_financial_facts_row:
        conn.execute("INSERT INTO financial_facts VALUES ('ff1', 'poison: this must never be read')")
    conn.commit()
    conn.close()
    return db_path


class SqlBoundaryTests(unittest.TestCase):
    def test_fetch_documents_never_touches_financial_facts(self):
        with tempfile.TemporaryDirectory() as tmp:
            db_path = _make_db(Path(tmp))
            conn = sqlite3.connect(db_path)
            statements = []
            conn.set_trace_callback(statements.append)
            rows = bf.fetch_documents(conn, ["ZZZ"])
            conn.close()

            self.assertEqual(len(rows), 1)
            for sql in statements:
                self.assertNotIn("financial_facts", sql)
                self.assertNotIn("report_tagged_facts", sql)


class ClassificationTests(unittest.TestCase):
    def setUp(self):
        self.duration, self.instant = bf.load_key_map_concepts(Path(__file__).resolve().parent.parent / "gt_key_map.json")

    def _instance(self, start, end, lang="pl", package_member=None, extra_facts=""):
        import esef_ixbrl as ix

        doc = make_instance(
            contexts=context("c1", start=start, end=end),
            units=unit("u1"),
            facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000") + extra_facts,
            lang=lang,
        )
        inst = ix.parse_instance(doc, package_member)
        inst["package_member"] = package_member
        inst["raw_bytes"] = doc
        return inst

    def test_annual_period_classified_fy(self):
        inst = self._instance("2025-01-01", "2025-12-31")
        result = bf.classify_file([inst], self.duration, "ZZZ skonsolidowane sprawozdanie roczne", "zzz_pl.xhtml")
        self.assertEqual(result["period_type"], "FY")
        self.assertEqual(result["fiscal_year"], 2025)
        self.assertEqual(result["basis_scope"], "consolidated")
        self.assertEqual(result["language"], "pl")

    def test_half_year_classified_h1(self):
        inst = self._instance("2025-01-01", "2025-06-30", lang="en")
        result = bf.classify_file([inst], self.duration, "ZZZ standalone", "zzz_h1_en.xhtml")
        self.assertEqual(result["period_type"], "H1")
        self.assertEqual(result["basis_scope"], "standalone")
        self.assertEqual(result["language"], "en")

    def test_language_falls_back_to_filename_when_no_xml_lang(self):
        import esef_ixbrl as ix

        doc = make_instance(
            contexts=context("c1", start="2025-01-01", end="2025-12-31"),
            units=unit("u1"),
            facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000"),
        )
        doc = doc.replace(b' xml:lang="pl"', b"")  # strip xml:lang to force the filename fallback
        inst = ix.parse_instance(doc)
        inst["package_member"] = None
        inst["raw_bytes"] = doc
        result = bf.classify_file([inst], self.duration, "ZZZ", "zzz_en.xhtml")
        self.assertEqual(result["language"], "en")

    def test_quarter_classified_by_end_month(self):
        # No other-duration evidence in the file -> fiscal year start
        # defaults to this quarter's own start month (January) -> Q1.
        inst = self._instance("2025-01-01", "2025-03-31")
        result = bf.classify_file([inst], self.duration, "ZZZ", "zzz.xhtml")
        self.assertEqual(result["period_type"], "Q1")

    def test_quarter_classified_from_shifted_fiscal_year_start(self):
        # Amendment L / astra r1 finding 10: quarters are counted from the
        # FISCAL year start, not always calendar January. Two comparative
        # contexts starting in April (the true fiscal-year start) outvote
        # the current quarter's own July start when inferring the fiscal
        # calendar, so Jul-Sep becomes Q2 of an April-start fiscal year
        # (Apr-Jun=Q1, Jul-Sep=Q2), not Q3 as a calendar-Jan reading would.
        import esef_ixbrl as ix

        doc = make_instance(
            contexts=(
                context("c_current", start="2025-07-01", end="2025-09-30")
                + context("c_cmp1", start="2024-04-01", end="2025-03-31")
                + context("c_cmp2", start="2024-04-01", end="2024-09-30")
            ),
            units=unit("u1"),
            facts=(
                non_fraction("ifrs-full:Revenue", "c_current", "u1", "100")
                + non_fraction("ifrs-full:GrossProfit", "c_cmp1", "u1", "400")
                + non_fraction("ifrs-full:GrossProfit", "c_cmp2", "u1", "200")
            ),
        )
        inst = ix.parse_instance(doc)
        inst["package_member"] = None
        inst["raw_bytes"] = doc
        result = bf.classify_file([inst], self.duration, "ZZZ", "zzz.xhtml")
        self.assertEqual(result["period_type"], "Q2")
        self.assertEqual(result["fiscal_year"], 2025)

    def test_irregular_duration_is_unknown(self):
        # 2-4/5-7/8-10/11-13 months are contiguous (Q1-4/H1/Q3-YTD/FY) --
        # only a span outside that whole 2-13 month range is irregular.
        inst = self._instance("2025-01-01", "2026-04-01")
        result = bf.classify_file([inst], self.duration, "ZZZ", "zzz.xhtml")
        self.assertEqual(result["period_type"], "unknown")
        self.assertIn("irregular duration (15 months)", result["unknown_reasons"])

    def test_nine_month_cumulative_ytd_classified_q3(self):
        # GPW convention: the interim current-period column is cumulative
        # YTD, so a 9-month duration is always Q3 -- never bucketed by
        # end_month the way the 3-month (standalone-quarter) band is.
        inst = self._instance("2025-01-01", "2025-09-30")
        result = bf.classify_file([inst], self.duration, "ZZZ skonsolidowane", "zzz_q3_pl.xhtml")
        self.assertEqual(result["period_type"], "Q3")
        self.assertEqual(result["duration_months"], 9)

    def test_basis_and_language_unknown_when_not_recoverable(self):
        inst = self._instance("2025-01-01", "2025-12-31", lang=None or "")
        result = bf.classify_file([inst], self.duration, "no hints here", "generic.xhtml")
        self.assertEqual(result["basis_scope"], "unknown")
        self.assertEqual(result["language"], "unknown")

    def test_basis_falls_back_to_cover_page_text(self):
        # Neither the member path nor the outer path/title carry a basis
        # word -- the last resort is the first ~2KB of the instance body
        # itself (amendment L).
        inst = self._instance("2025-01-01", "2025-12-31", package_member="reports/primary.xhtml")
        inst["raw_bytes"] = b"<html><body>Skonsolidowane sprawozdanie finansowe grupy kapitalowej</body></html>"
        result = bf.classify_file([inst], self.duration, "no hints", "no_hints_path.xhtml")
        self.assertEqual(result["basis_scope"], "consolidated")

    def test_member_and_outer_basis_contradiction_is_unknown_and_queued(self):
        # Astra r1 finding 8: a package whose OUTER title says consolidated
        # must not override a clearly STANDALONE member path.
        inst = self._instance(
            "2025-01-01", "2025-12-31", package_member="reports/sprawozdanie_jednostkowe.xhtml"
        )
        result = bf.classify_file(
            [inst], self.duration, "ZZZ Consolidated Annual Report", "zzz_consolidated.zip"
        )
        self.assertEqual(result["basis_scope"], "unknown")
        self.assertIn("basis contradicts between member and outer evidence", result["unknown_reasons"])

    def test_cover_page_basis_survives_a_large_preamble(self):
        # Real-data finding (owner DB, 2026-09-12): a real ESEF instance's
        # hidden ix:header/context/fact preamble is mostly TAGS (stripped
        # away) with only tiny text nodes between them, but it easily runs
        # past 8000 raw bytes before any visible cover-page text begins --
        # the old 8000-byte raw window cut the basis word off entirely,
        # leaving basis "unknown" for every real ESEF package. This preamble
        # is >8000 raw bytes but strips down to a few hundred characters, so
        # it also proves the fix isn't just "a bigger text budget" -- the
        # RAW window has to clear the tag-heavy preamble first.
        preamble = b"<div>" + (b"<span>x</span>" * 700) + b"</div>"  # ~9.8KB raw, ~700 chars once stripped
        inst = self._instance("2025-01-01", "2025-12-31", package_member="reports/primary.xhtml")
        inst["raw_bytes"] = preamble + b"<body>Skonsolidowane sprawozdanie finansowe grupy kapitalowej</body>"
        self.assertGreater(len(preamble), 8000)
        result = bf.classify_file([inst], self.duration, "no hints", "no_hints_path.xhtml")
        self.assertEqual(result["basis_scope"], "consolidated")

    def test_cover_page_basis_survives_a_huge_inline_style_block(self):
        # Real-data finding (owner DB, 2026-09-12): one real filing's
        # <style> block alone was 8.5MB (of a 9.3MB file) -- plain
        # tag-stripping leaves that raw CSS TEXT as "content" (style/script
        # tags removed, their text kept), swamping any bounded window long
        # before the real basis word. Style/script content must be excised
        # wholesale, not just their tags.
        style_block = b"<style>" + (b"body{color:#fff}\n" * 20000) + b"</style>"  # ~340KB of CSS text
        inst = self._instance("2025-01-01", "2025-12-31", package_member="reports/primary.xhtml")
        inst["raw_bytes"] = b"<head>" + style_block + b"</head><body>Skonsolidowane sprawozdanie</body>"
        result = bf.classify_file([inst], self.duration, "no hints", "no_hints_path.xhtml")
        self.assertEqual(result["basis_scope"], "consolidated")

    def test_cover_page_basis_reachable_through_dense_tag_markup(self):
        # Real-data finding (owner DB, 2026-09-12): real ix:header/context
        # markup is so tag-dense that stripping tags from a modest raw
        # window yields almost no text -- one real filing needed ~3MB of
        # raw bytes (after style removal) to reach its first visible
        # cover-page word. A few thousand short, attribute-heavy tags here
        # simulate that density; the basis word must still be found.
        dense_markup = b'<context id="c1" scheme="x"></context>' * 15000  # ~580KB raw, ~0 chars once stripped
        inst = self._instance("2025-01-01", "2025-12-31", package_member="reports/primary.xhtml")
        inst["raw_bytes"] = dense_markup + b"<body>Skonsolidowane sprawozdanie</body>"
        self.assertGreater(len(dense_markup), 300_000)
        result = bf.classify_file([inst], self.duration, "no hints", "no_hints_path.xhtml")
        self.assertEqual(result["basis_scope"], "consolidated")

    def test_language_from_esef_package_folder_name(self):
        # Real-data finding: the folder-name language token sits right
        # before a "/" (`ATR-2025-12-31-1-pl/reports/...`), which the old
        # `_.-`-only boundary regex never matched.
        lang, report_date = bf.esef_folder_info("ATR-2025-12-31-1-pl/reports/atr-2025-12-31-1-pl.xhtml")
        self.assertEqual(lang, "pl")
        self.assertEqual(report_date, "2025-12-31")

    def test_language_falls_back_to_esef_folder_name_when_xml_lang_absent(self):
        inst = self._instance("2025-01-01", "2025-12-31", package_member="ATR-2025-12-31-1-en/reports/x.xhtml")
        import esef_ixbrl as ix

        inst_bytes = inst["raw_bytes"].replace(b' xml:lang="pl"', b"")
        inst = ix.parse_instance(inst_bytes, inst["package_member"])
        inst["package_member"] = "ATR-2025-12-31-1-en/reports/x.xhtml"
        inst["raw_bytes"] = inst_bytes
        result = bf.classify_file([inst], self.duration, "no hints", "no_hints_path.xhtml")
        self.assertEqual(result["language"], "en")


class SelectionDeterminismTests(unittest.TestCase):
    def _candidate(self, issuer_id, period_type, fiscal_year, end, language="pl", basis="consolidated", sha="a" * 64,
                   is_package=False, fetched_at="2026-01-01"):
        return {
            "issuer_id": issuer_id,
            "period_type": period_type,
            "fiscal_year": fiscal_year,
            "period_start": f"{fiscal_year}-01-01",
            "period_end": end,
            "language": language,
            "basis_scope": basis,
            "sha256": sha,
            "is_package": is_package,
            "fetched_at": fetched_at,
        }

    def test_same_input_yields_same_selection(self):
        candidates = [
            self._candidate("iss_01", "FY", 2025, "2025-12-31", sha="a" * 64),
            self._candidate("iss_01", "FY", 2024, "2024-12-31", sha="b" * 64),
            self._candidate("iss_01", "H1", 2025, "2025-06-30", sha="c" * 64),
            self._candidate("iss_01", "FY", 2025, "2025-12-31", language="en", sha="d" * 64),
        ]
        result1 = bf.select_events([dict(c) for c in candidates], "pl")
        result2 = bf.select_events([dict(c) for c in candidates], "pl")
        roles1 = sorted((c["sha256"], c["role"]) for c in result1)
        roles2 = sorted((c["sha256"], c["role"]) for c in result2)
        self.assertEqual(roles1, roles2)

        by_sha = {c["sha256"]: c["role"] for c in result1}
        self.assertEqual(by_sha["a" * 64], "floor")  # newest annual, pinned language
        self.assertEqual(by_sha["c" * 64], "floor")  # the only interim
        self.assertEqual(by_sha["d" * 64], "twin_diagnostic")  # EN twin of the FY2025 floor
        self.assertEqual(by_sha["b" * 64], "warmup")  # earlier fiscal year

    def test_sha_order_tiebreak_is_stable(self):
        # Two candidates identical on every selection field except sha256 --
        # the higher sha wins deterministically (reverse sort), every run.
        candidates = [
            self._candidate("iss_01", "FY", 2025, "2025-12-31", sha="z" * 64),
            self._candidate("iss_01", "FY", 2025, "2025-12-31", sha="a" * 64),
        ]
        result = bf.select_events([dict(c) for c in candidates], "pl")
        floors = [c for c in result if c["role"] == "floor"]
        self.assertEqual(len(floors), 1)
        self.assertEqual(floors[0]["sha256"], "z" * 64)

    def test_correction_sharing_the_floor_period_becomes_a_warmup_vintage_not_discarded(self):
        # Astra r1 finding 10: the OLD code excluded every candidate sharing
        # the floor's (fiscal_year, period_type) from the warmup pool --
        # silently discarding a same-period correction/restatement. The
        # fix excludes only the exact selected objects (by identity).
        original = self._candidate("iss_01", "FY", 2025, "2025-12-31", sha="a" * 64, fetched_at="2026-01-01")
        correction = self._candidate("iss_01", "FY", 2025, "2025-12-31", sha="b" * 64, fetched_at="2026-03-01")
        result = bf.select_events([dict(original), dict(correction)], "pl")
        self.assertEqual(len(result), 2)
        roles = {c["sha256"]: c["role"] for c in result}
        # the later-fetched correction sorts first -> floor; the original
        # becomes a warmup vintage instead of vanishing.
        self.assertEqual(roles["b" * 64], "floor")
        self.assertEqual(roles["a" * 64], "warmup")


class RegistryHashTests(unittest.TestCase):
    def test_stable_regardless_of_input_order(self):
        shas = ["b" * 64, "a" * 64, "c" * 64]
        self.assertEqual(bf.registry_hash(shas), bf.registry_hash(list(reversed(shas))))

    def test_changes_when_a_file_changes(self):
        h1 = bf.registry_hash(["a" * 64, "b" * 64])
        h2 = bf.registry_hash(["a" * 64, "c" * 64])
        self.assertNotEqual(h1, h2)


class EndToEndDeterminismTests(unittest.TestCase):
    def test_build_frame_is_deterministic_across_runs(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            data_dir = tmp_path / "docs"
            data_dir.mkdir()
            instance = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000"),
                lang="pl",
            )
            pkg = zip_package({"reports/zzz_pl.xhtml": instance})
            (data_dir / "zzz.zip").write_bytes(pkg)

            db_path = _make_db(tmp_path, with_financial_facts_row=False)
            conn = sqlite3.connect(db_path)
            conn.execute("UPDATE report_documents SET local_path='zzz.zip', title='ZZZ skonsolidowane'")
            conn.commit()
            conn.close()

            out1 = tmp_path / "out1"
            out2 = tmp_path / "out2"
            m1 = bf.build_frame(str(db_path), str(data_dir), ["ZZZ"], str(out1))
            m2 = bf.build_frame(str(db_path), str(data_dir), ["ZZZ"], str(out2))

            # registry_hash and event shape are stable across runs; drop the
            # run-local snapshot.taken_at date before comparing.
            m1.pop("snapshot"), m2.pop("snapshot")
            self.assertEqual(m1, m2)
            self.assertEqual(len(m1["events"]), 1)
            self.assertEqual(m1["events"][0]["role"], "floor")
            self.assertEqual(m1["events"][0]["warmup_order"], 0)  # astra r1 finding 6: present on every event
            self.assertEqual(m1["events"][0]["labeled_period"]["duration_months"], 12)
            self.assertTrue((out1 / "corpus" / "zzz.zip").exists())

    def test_unselected_unknown_candidate_is_queued_not_dropped(self):
        # Astra r1 finding 9: an unresolved candidate that never gets picked
        # as an event (its period can't be classified, so it can't compete
        # for floor/twin/warmup) must still appear in review-queue.json --
        # the old code only queued reasons from ALREADY-SELECTED events.
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            data_dir = tmp_path / "docs"
            data_dir.mkdir()

            good = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000"),
                lang="pl",
            )
            (data_dir / "good.zip").write_bytes(zip_package({"reports/good_pl.xhtml": good}))

            # No duration-concept fact at all -> period_type stays "unknown"
            # -> never selectable -> must still reach the review queue.
            unknown = make_instance(
                contexts=context("c1", instant="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Assets", "c1", "u1", "5000"),
                lang="pl",
            )
            (data_dir / "unknown.xhtml").write_bytes(unknown)

            db_path = _make_db(tmp_path, with_financial_facts_row=False)
            conn = sqlite3.connect(db_path)
            conn.execute("UPDATE report_documents SET local_path='good.zip', title='ZZZ skonsolidowane' WHERE id='doc1'")
            conn.execute(
                "INSERT INTO report_documents VALUES ('doc2','c1','unknown.xhtml','ZZZ note',"
                "'http://example.org/zzz2','application/xhtml+xml',NULL,'fetched','2026-03-02T00:00:00Z')"
            )
            conn.commit()
            conn.close()

            out = tmp_path / "out"
            bf.build_frame(str(db_path), str(data_dir), ["ZZZ"], str(out))
            review_queue = json.loads((out / "review-queue.json").read_text())
            unknown_sha = bf.sha256_hex(unknown)
            entry = next((r for r in review_queue if r["sha256"] == unknown_sha), None)
            self.assertIsNotNone(entry, "the unselected unknown candidate must still be queued")

    def test_non_ixbrl_file_never_becomes_an_event_and_corpus_holds_only_selected(self):
        # Real-data fixes #1/#3 (owner DB, 2026-09-12): a board letter/opinion
        # style .xhtml with NO iXBRL markup at all must never become an
        # event (not even a "parse failure" one) -- it is skipped, listed in
        # skipped-non-ixbrl.json, and never copied into corpus/. Only the
        # genuine ESEF package becomes the (one) floor event.
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            data_dir = tmp_path / "docs"
            data_dir.mkdir()

            real_package = make_instance(
                contexts=context("c1", start="2025-01-01", end="2025-12-31"),
                units=unit("u1"),
                facts=non_fraction("ifrs-full:Revenue", "c1", "u1", "1000"),
                lang="pl",
            )
            pkg = zip_package({"ZZZ-2025-12-31-1-pl/reports/zzz-2025-12-31-1-pl.xhtml": real_package})
            (data_dir / "zzz.zip").write_bytes(pkg)

            board_letter = b"<html><body><p>Dear shareholders, thank you for your continued trust.</p></body></html>"
            (data_dir / "board_letter.xhtml").write_bytes(board_letter)

            db_path = _make_db(tmp_path, with_financial_facts_row=False)
            conn = sqlite3.connect(db_path)
            conn.execute("UPDATE report_documents SET local_path='zzz.zip', title='ZZZ skonsolidowane' WHERE id='doc1'")
            conn.execute(
                "INSERT INTO report_documents VALUES ('doc2','c1','board_letter.xhtml','Letter to shareholders',"
                "'http://example.org/zzz-letter','application/xhtml+xml',NULL,'fetched','2026-03-02T00:00:00Z')"
            )
            conn.commit()
            conn.close()

            out = tmp_path / "out"
            manifest = bf.build_frame(str(db_path), str(data_dir), ["ZZZ"], str(out))

            self.assertEqual(len(manifest["events"]), 1)
            self.assertEqual(manifest["events"][0]["role"], "floor")

            skipped = json.loads((out / "skipped-non-ixbrl.json").read_text())
            self.assertEqual(len(skipped), 1)
            self.assertEqual(skipped[0]["document_id"], "doc2")

            review_queue = json.loads((out / "review-queue.json").read_text())
            self.assertFalse(
                any(r["sha256"] == bf.sha256_hex(board_letter) for r in review_queue),
                "non-iXBRL markup must never reach the review queue",
            )

            corpus_files = sorted(p.name for p in (out / "corpus").iterdir())
            self.assertEqual(corpus_files, ["zzz.zip"])  # never the board letter

            summary = " ".join(manifest["honest_limitations"])
            self.assertIn("1 non-iXBRL markup file(s) skipped", summary)


if __name__ == "__main__":
    unittest.main()


class RegistryHashFormulaTests(unittest.TestCase):
    """The registry hash is the cross-language corpus identity — the Rust
    harness recomputes it from the files; both sides must agree byte for byte."""

    def test_registry_hash_is_sha256_over_sorted_deduplicated_digests_no_separator(self):
        import hashlib

        import build_frame as bf

        a = "b" * 64
        b = "a" * 64
        expected = hashlib.sha256((b + a).encode("ascii")).hexdigest()
        self.assertEqual(bf.registry_hash([a, b, a]), expected)
