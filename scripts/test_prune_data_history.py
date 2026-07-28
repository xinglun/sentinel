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


def write_snapshots(directory: Path, name: str, rows: list[dict[str, object]]) -> None:
    snapshot_dir = directory / name
    snapshot_dir.mkdir()
    for row in rows:
        (snapshot_dir / f"{row['date']}.json").write_text(
            json.dumps(row, ensure_ascii=False), encoding="utf-8"
        )


def write_transition_csv(directory: Path, rows: list[dict[str, object]]) -> None:
    (directory / "state_transitions.csv").write_text(
        "Timestamp,No_Trade_Persists\n"
        + "".join(f"row-{row['date']}\n" for row in rows),
        encoding="utf-8",
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

    def test_removes_legacy_csv_when_history_is_under_limit(self) -> None:
        with tempfile.TemporaryDirectory() as raw_dir:
            directory = Path(raw_dir)
            rows = [{"date": f"2026-07-{day:02d}", "value": day} for day in range(1, 7)]
            write_history(directory, "decision_history.jsonl", rows)
            write_history(directory, "state_transitions.jsonl", rows)
            legacy_path = directory / "state_transitions_legacy_20260701.csv"
            legacy_path.write_text("legacy header\nlegacy row\n", encoding="utf-8")

            result = prune_reports(directory, max_bytes=10_000, min_days=5, dry_run=False)

            self.assertEqual(result["action"], "legacy_cleanup")
            self.assertFalse(legacy_path.exists())

    def test_dry_run_reports_legacy_csv_without_deleting(self) -> None:
        with tempfile.TemporaryDirectory() as raw_dir:
            directory = Path(raw_dir)
            rows = [{"date": f"2026-07-{day:02d}", "value": day} for day in range(1, 7)]
            write_history(directory, "decision_history.jsonl", rows)
            write_history(directory, "state_transitions.jsonl", rows)
            legacy_path = directory / "state_transitions_legacy_20260701.csv"
            legacy_path.write_text("legacy header\nlegacy row\n", encoding="utf-8")

            result = prune_reports(directory, max_bytes=10_000, min_days=5, dry_run=True)

            self.assertEqual(result["action"], "dry_run")
            self.assertTrue(legacy_path.exists())
            self.assertEqual(result["after_bytes"][str(legacy_path)], 0)

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

    def test_prunes_transition_csv_and_legacy_csv(self) -> None:
        with tempfile.TemporaryDirectory() as raw_dir:
            directory = Path(raw_dir)
            rows = [{"date": f"2026-07-{day:02d}", "value": "x" * 20} for day in range(1, 8)]
            write_history(directory, "decision_history.jsonl", rows)
            write_history(directory, "state_transitions.jsonl", rows)
            write_transition_csv(directory, rows)
            legacy_path = directory / "state_transitions_legacy_20260701.csv"
            legacy_path.write_text("legacy header\nlegacy row\n", encoding="utf-8")

            result = prune_reports(directory, max_bytes=350, min_days=5, dry_run=False)

            self.assertEqual(result["action"], "pruned")
            csv_lines = (directory / "state_transitions.csv").read_text().splitlines()
            self.assertEqual(csv_lines[0], "Timestamp,No_Trade_Persists")
            self.assertEqual(csv_lines[1], "row-2026-07-02")
            self.assertEqual(len(csv_lines), 7)
            self.assertFalse(legacy_path.exists())

    def test_rejects_transition_csv_with_extra_rows(self) -> None:
        with tempfile.TemporaryDirectory() as raw_dir:
            directory = Path(raw_dir)
            rows = [{"date": f"2026-07-{day:02d}", "value": "x" * 20} for day in range(1, 8)]
            write_history(directory, "decision_history.jsonl", rows)
            write_history(directory, "state_transitions.jsonl", rows)
            write_transition_csv(directory, rows + [{"date": "2026-07-08"}])

            with self.assertRaisesRegex(ValueError, "CSV rows do not match"):
                prune_reports(directory, max_bytes=350, min_days=5, dry_run=False)

    def test_prunes_legacy_csv_without_current_csv(self) -> None:
        with tempfile.TemporaryDirectory() as raw_dir:
            directory = Path(raw_dir)
            rows = [{"date": f"2026-07-{day:02d}", "value": "x" * 20} for day in range(1, 8)]
            write_history(directory, "decision_history.jsonl", rows)
            write_history(directory, "state_transitions.jsonl", rows)
            legacy_path = directory / "state_transitions_legacy_20260701.csv"
            legacy_path.write_text("legacy header\nlegacy row\n", encoding="utf-8")

            prune_reports(directory, max_bytes=350, min_days=5, dry_run=False)

            self.assertFalse(legacy_path.exists())

    def test_rejects_invalid_json(self) -> None:
        with tempfile.TemporaryDirectory() as raw_dir:
            directory = Path(raw_dir)
            (directory / "decision_history.jsonl").write_text("not-json\n", encoding="utf-8")
            write_history(directory, "state_transitions.jsonl", [{"date": "2026-07-01"}])
            with self.assertRaisesRegex(ValueError, "invalid JSON"):
                prune_reports(directory, max_bytes=10, min_days=1, dry_run=False)

    def test_rejects_invalid_dated_observation_timeline_json(self) -> None:
        with tempfile.TemporaryDirectory() as raw_dir:
            directory = Path(raw_dir)
            write_history(directory, "decision_history.jsonl", [{"date": "2026-07-01"}])
            write_history(directory, "state_transitions.jsonl", [{"date": "2026-07-01"}])
            (directory / "observation_timeline_2026-07-01.json").write_text(
                "not-json\n", encoding="utf-8"
            )

            with self.assertRaisesRegex(
                ValueError,
                r"observation_timeline_2026-07-01\.json: line 1 is invalid JSON",
            ):
                prune_reports(directory, max_bytes=10, min_days=1, dry_run=False)

    def test_prunes_jsonl_and_formal_snapshot_directories_with_one_cutoff(self) -> None:
        with tempfile.TemporaryDirectory() as raw_dir:
            directory = Path(raw_dir)
            rows = [{"date": f"2026-07-{day:02d}", "value": "x" * 80} for day in range(1, 8)]
            write_history(directory, "decision_history.jsonl", rows)
            write_history(directory, "state_transitions.jsonl", rows)
            write_snapshots(directory, "decision_snapshots", rows)
            write_snapshots(directory, "timeline_snapshots", rows)
            for name in ("leader_snapshots", "snapshots"):
                snapshot_dir = directory / name
                snapshot_dir.mkdir()
                for row in rows:
                    filename = (
                        f"cycle-2026-07-01_{row['date']}.json"
                        if name == "snapshots"
                        else f"{row['date']}.json"
                    )
                    (snapshot_dir / filename).write_text(
                        json.dumps(row, ensure_ascii=False), encoding="utf-8"
                    )

            result = prune_reports(directory, max_bytes=650, min_days=5, dry_run=False)

            self.assertEqual(result["action"], "pruned")
            self.assertEqual(result["cutoff"], "2026-07-03")
            for name in ("decision_history.jsonl", "state_transitions.jsonl"):
                self.assertEqual(len((directory / name).read_text().splitlines()), 5)
            for name in (
                "decision_snapshots",
                "timeline_snapshots",
                "leader_snapshots",
                "snapshots",
            ):
                snapshot_dir = directory / name
                self.assertEqual(
                    sorted(
                        path.name
                        for path in snapshot_dir.glob("*.json")
                    ),
                    [
                        (
                            f"cycle-2026-07-01_2026-07-{day:02d}.json"
                            if name == "snapshots"
                            else f"2026-07-{day:02d}.json"
                        )
                        for day in range(3, 8)
                    ],
                )

    def test_prunes_dated_timeline_files_and_ignores_empty_snapshot_directory(self) -> None:
        with tempfile.TemporaryDirectory() as raw_dir:
            directory = Path(raw_dir)
            rows = [
                {
                    "entries": [{"date": f"2026-07-{day:02d}"}],
                    "summary": "timeline",
                }
                for day in range(1, 8)
            ]
            write_history(
                directory,
                "decision_history.jsonl",
                [{"date": f"2026-07-{day:02d}", "value": "x" * 80} for day in range(1, 8)],
            )
            write_history(
                directory,
                "state_transitions.jsonl",
                [{"date": f"2026-07-{day:02d}", "value": "x" * 80} for day in range(1, 8)],
            )
            for day, row in enumerate(rows, start=1):
                (directory / f"observation_timeline_2026-07-{day:02d}.json").write_text(
                    json.dumps(row), encoding="utf-8"
                )
            (directory / "observation_timeline_latest.json").write_text(
                json.dumps(rows[-1]), encoding="utf-8"
            )
            (directory / "leader_snapshots").mkdir()

            result = prune_reports(directory, max_bytes=650, min_days=5, dry_run=False)

            self.assertEqual(result["cutoff"], "2026-07-03")
            self.assertFalse((directory / "observation_timeline_2026-07-01.json").exists())
            self.assertFalse((directory / "observation_timeline_2026-07-02.json").exists())
            self.assertTrue((directory / "observation_timeline_2026-07-03.json").exists())
            self.assertTrue((directory / "observation_timeline_latest.json").exists())


if __name__ == "__main__":
    unittest.main()
