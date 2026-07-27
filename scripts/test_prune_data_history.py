#!/usr/bin/env python3
"""data history cleanup policy の回帰テスト。"""
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from prune_data_history import prune_reports


def write_history(directory: Path, name: str, rows: list[dict[str, object]]) -> None:
    (directory / name).write_text(
        "".join(json.dumps(row, ensure_ascii=False) + "\n" for row in rows), encoding="utf-8"
    )


class PruneDataHistoryTest(unittest.TestCase):
    def test_does_not_delete_when_below_limit(self) -> None:
        with tempfile.TemporaryDirectory() as raw_dir:
            directory = Path(raw_dir)
            rows = [{"date": f"2026-07-{day:02d}", "value": day} for day in range(1, 7)]
            write_history(directory, "decision_history.jsonl", rows)
            write_history(directory, "state_transitions.jsonl", rows)
            result = prune_reports(directory, max_bytes=10_000, min_days=5, dry_run=False)
            self.assertEqual(result["action"], "none")
            self.assertEqual(len((directory / "decision_history.jsonl").read_text().splitlines()), 6)

    def test_prunes_old_days_using_one_cutoff(self) -> None:
        with tempfile.TemporaryDirectory() as raw_dir:
            directory = Path(raw_dir)
            rows = [{"date": f"2026-07-{day:02d}", "value": "x" * 20} for day in range(1, 8)]
            write_history(directory, "decision_history.jsonl", rows)
            write_history(directory, "state_transitions.jsonl", rows)
            result = prune_reports(directory, max_bytes=350, min_days=5, dry_run=False)
            self.assertEqual(result["action"], "pruned")
            self.assertEqual(result["cutoff"], "2026-07-02")
            self.assertEqual(
                (directory / "decision_history.jsonl").read_text().splitlines()[0],
                json.dumps(rows[1]),
            )
            self.assertEqual(
                (directory / "state_transitions.jsonl").read_text().splitlines()[0],
                json.dumps(rows[1]),
            )

    def test_rejects_invalid_json(self) -> None:
        with tempfile.TemporaryDirectory() as raw_dir:
            directory = Path(raw_dir)
            (directory / "decision_history.jsonl").write_text("not-json\n", encoding="utf-8")
            write_history(directory, "state_transitions.jsonl", [{"date": "2026-07-01"}])
            with self.assertRaisesRegex(ValueError, "invalid JSON"):
                prune_reports(directory, max_bytes=10, min_days=1, dry_run=False)


if __name__ == "__main__":
    unittest.main()
