"""Tests for HDB reconciliation helpers."""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "verify_hdb.py"
SPEC = importlib.util.spec_from_file_location("verify_hdb", SCRIPT)
verify_hdb = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = verify_hdb
SPEC.loader.exec_module(verify_hdb)


class HdbReconciliationTests(unittest.TestCase):
    """Model comparisons and canonical digests."""

    def test_compare_matching_rows(self) -> None:
        """Check that matching counts produce PASS values."""
        rows = dict.fromkeys(verify_hdb.DATASETS, 1)
        manifest = {"row_counts": rows}
        verify = {"rows": [1] * len(verify_hdb.DATASETS)}
        queries = {
            "journal_rows": 1,
            "message_rows": 1,
            "level_rows": 1,
            "instrument_rows": 1,
            "coverage_symbols": 1,
        }
        checks = verify_hdb.compare(verify, queries, manifest)
        assert all(value == "PASS" for value in checks.values())

    def test_compare_rejects_mismatch(self) -> None:
        """Check that a row mismatch produces FAIL."""
        rows = dict.fromkeys(verify_hdb.DATASETS, 1)
        manifest = {"row_counts": rows}
        verify = {"rows": [0] * len(verify_hdb.DATASETS)}
        queries = {
            "journal_rows": 1,
            "message_rows": 1,
            "level_rows": 1,
            "instrument_rows": 1,
            "coverage_symbols": 1,
        }
        checks = verify_hdb.compare(verify, queries, manifest)
        assert checks["q_counts"] == "FAIL"


if __name__ == "__main__":
    unittest.main()
