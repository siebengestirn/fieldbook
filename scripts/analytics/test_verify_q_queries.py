"""Tests for the q query reconciliation certificate."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "verify_q_queries.py"
SPEC = importlib.util.spec_from_file_location("verify_q_queries", SCRIPT)
verify_q_queries = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = verify_q_queries
SPEC.loader.exec_module(verify_q_queries)


class QueryReconciliationTests(unittest.TestCase):
    """Model matching and missing q results."""

    def test_matching_results_certify(self) -> None:
        """Check that matching counts produce PASS."""
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            export = root / "export"
            results = root / "results"
            export.mkdir()
            results.mkdir()
            rows = {
                "journal_observation": 10,
                "capture_run": 1,
                "sbe_message": 5,
                "sbe_book_level": 20,
                "capture_instrument": 2,
                "capture_price_range": 0,
                "capture_tick_band": 1,
            }
            manifest = {
                "export_id": "a" * 64,
                "journal_provenance_status": "legacy_incomplete",
                "row_counts": rows,
            }
            (export / "export-manifest.json").write_text(json.dumps(manifest))
            values = {
                "q-preflight.json": {"q_version": 5, "journal_rows": 10},
                "q-capture.json": {
                    "rows": 1,
                    "provenance": "legacy_incomplete",
                    "sbe_messages": 5,
                    "journal_records": 10,
                },
                "q-journal.json": {"rows": 10},
                "q-orderbook.json": {"messages": 5, "levels": 20},
                "q-instruments.json": {
                    "instruments": 2,
                    "ranges": 0,
                    "bands": 1,
                },
            }
            for name, content in values.items():
                (results / name).write_text(json.dumps(content))
            certificate = verify_q_queries.reconcile(export, results)
            self.assertEqual(certificate["status"], "PASS")

    def test_missing_results_are_incomplete(self) -> None:
        """Check that absent q results never certify."""
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            export = root / "export"
            results = root / "results"
            export.mkdir()
            results.mkdir()
            (export / "export-manifest.json").write_text("{}")
            certificate = verify_q_queries.reconcile(export, results)
            self.assertEqual(certificate["status"], "INCOMPLETE")


if __name__ == "__main__":
    unittest.main()
