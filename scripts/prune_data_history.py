#!/usr/bin/env python3
"""data ブランチの append-only JSONL を trading-day 単位で整理する。"""
from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
from collections import defaultdict
from dataclasses import dataclass
from datetime import date
from pathlib import Path
from typing import Iterable


DEFAULT_MAX_BYTES = 90 * 1024 * 1024
DEFAULT_MIN_DAYS = 5
HISTORY_FILES = ("decision_history.jsonl", "state_transitions.jsonl")
TRANSITION_CSV = "state_transitions.csv"
LEGACY_TRANSITION_GLOB = "state_transitions_legacy_*.csv"
OPTIONAL_HISTORY_FILES = ("observation_timeline.jsonl", "leader_observations.jsonl")
SNAPSHOT_DIRECTORIES = (
    "decision_snapshots",
    "timeline_snapshots",
    "leader_snapshots",
    "snapshots",
)


@dataclass(frozen=True)
class Record:
    market_date: date
    payload: bytes


def _is_iso_date(value: str) -> bool:
    try:
        date.fromisoformat(value)
    except ValueError:
        return False
    return True


def _market_date(value: object, *, path: Path, line_number: int) -> date:
    if not isinstance(value, dict):
        raise ValueError(f"{path}: line {line_number} is not a JSON object")

    raw = value.get("date")
    if not isinstance(raw, str):
        raw = value.get("market_date")
    if not isinstance(raw, str):
        timestamp = value.get("timestamp") or value.get("generated_at")
        raw = timestamp[:10] if isinstance(timestamp, str) else None
    if not isinstance(raw, str):
        entries = value.get("entries") if isinstance(value, dict) else None
        if isinstance(entries, list):
            dates = [entry.get("date") for entry in entries if isinstance(entry, dict)]
            raw = max((item for item in dates if isinstance(item, str)), default=None)
    if not isinstance(raw, str):
        raise ValueError(f"{path}: line {line_number} has no trading-day date")
    try:
        return date.fromisoformat(raw[:10])
    except ValueError as exc:
        raise ValueError(f"{path}: line {line_number} has invalid date {raw!r}") from exc


def read_records(path: Path) -> list[Record]:
    records: list[Record] = []
    with path.open("rb") as handle:
        for line_number, payload in enumerate(handle, start=1):
            if not payload.strip():
                continue
            try:
                value = json.loads(payload)
            except json.JSONDecodeError as exc:
                raise ValueError(f"{path}: line {line_number} is invalid JSON") from exc
            records.append(Record(_market_date(value, path=path, line_number=line_number), payload))
    if not records:
        raise ValueError(f"{path}: no dated JSONL records found")
    return records


def read_json_record(path: Path) -> list[Record]:
    """整形済みの単一 JSON snapshot をファイル単位で読み取る。"""
    payload = path.read_bytes()
    if not payload.strip():
        raise ValueError(f"{path}: no dated JSON record found")
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise ValueError(f"{path}: line 1 is invalid JSON") from exc
    decoder = json.JSONDecoder()
    records: list[Record] = []
    offset = 0
    try:
        while offset < len(text):
            while offset < len(text) and text[offset].isspace():
                offset += 1
            if offset == len(text):
                break
            start = offset
            value, offset = decoder.raw_decode(text, offset)
            records.append(
                Record(
                    _market_date(value, path=path, line_number=1),
                    text[start:offset].encode("utf-8"),
                )
            )
    except json.JSONDecodeError as exc:
        raise ValueError(f"{path}: line 1 is invalid JSON") from exc
    if not records:
        raise ValueError(f"{path}: no dated JSON record found")
    return [records[-1]]


def _group_size(records: Iterable[Record]) -> dict[date, int]:
    sizes: dict[date, int] = defaultdict(int)
    for record in records:
        sizes[record.market_date] += len(record.payload)
    return dict(sizes)


def _retained_size(records: Iterable[Record], cutoff: date) -> int:
    return sum(len(record.payload) for record in records if record.market_date >= cutoff)


def choose_cutoff(
    records_by_file: dict[Path, list[Record]],
    *,
    max_bytes: int,
    min_days: int,
) -> date | None:
    all_dates = sorted({record.market_date for records in records_by_file.values() for record in records})
    if not all_dates:
        raise ValueError("no trading-day dates are available")

    current_sizes = {path: sum(len(record.payload) for record in records) for path, records in records_by_file.items()}
    if all(size <= max_bytes for size in current_sizes.values()):
        return None

    for cutoff in all_dates:
        retained_dates = sum(trading_day >= cutoff for trading_day in all_dates)
        if retained_dates < min_days:
            continue
        retained_sizes = {
            path: _retained_size(records, cutoff) for path, records in records_by_file.items()
        }
        if all(size <= max_bytes for size in retained_sizes.values()):
            return cutoff

    latest = all_dates[-1]
    latest_sizes = {
        path: _retained_size(records, latest) for path, records in records_by_file.items()
    }
    oversized = [str(path) for path, size in latest_sizes.items() if size > max_bytes]
    if oversized:
        raise ValueError(
            "latest trading-day record is larger than the configured limit: " + ", ".join(oversized)
        )
    raise ValueError(f"cannot retain the required minimum of {min_days} trading days below the limit")


def _write_records(path: Path, records: list[Record], cutoff: date) -> int:
    kept = [record.payload for record in records if record.market_date >= cutoff]
    if not kept:
        path.unlink()
        return 0
    directory = path.parent
    with tempfile.NamedTemporaryFile("wb", dir=directory, prefix=f".{path.name}.", delete=False) as handle:
        temporary_path = Path(handle.name)
        for payload in kept:
            handle.write(payload)
    os.chmod(temporary_path, path.stat().st_mode)
    os.replace(temporary_path, path)
    return sum(len(payload) for payload in kept)


def _write_transition_csv(path: Path, records: list[Record], cutoff: date, lines: list[str]) -> int:
    if not lines or not lines[0].strip():
        raise ValueError(f"{path}: CSV header is missing")
    data_lines = lines[1:]
    if len(data_lines) != len(records):
        raise ValueError(f"{path}: CSV rows do not match state transition records")
    kept = [
        line
        for record, line in zip(records, data_lines)
        if record.market_date >= cutoff
    ]
    directory = path.parent
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", newline="", dir=directory, prefix=f".{path.name}.", delete=False) as handle:
        temporary_path = Path(handle.name)
        handle.write(lines[0])
        handle.writelines(kept)
    os.chmod(temporary_path, path.stat().st_mode)
    os.replace(temporary_path, path)
    return sum(len(line.encode("utf-8")) for line in [lines[0], *kept])


def _read_snapshot_records(directory: Path) -> list[Record]:
    records: list[Record] = []
    for path in sorted(directory.glob("*.json")):
        candidates = path.stem.split("_")
        snapshot_date = next(
            (date.fromisoformat(candidate) for candidate in candidates if _is_iso_date(candidate)),
            None,
        )
        if snapshot_date is None:
            continue
        payload = path.read_bytes()
        if not payload.strip():
            continue
        try:
            json.loads(payload)
        except json.JSONDecodeError as exc:
            raise ValueError(f"{path}: invalid JSON") from exc
        records.append(Record(snapshot_date, payload))
    return records


def _write_snapshot_records(directory: Path, cutoff: date) -> int:
    total = 0
    for path in sorted(directory.glob("*.json")):
        candidates = path.stem.split("_")
        snapshot_date = next(
            (date.fromisoformat(candidate) for candidate in candidates if _is_iso_date(candidate)),
            None,
        )
        if snapshot_date is None:
            continue
        if snapshot_date < cutoff:
            path.unlink()
        else:
            total += path.stat().st_size
    return total


def prune_reports(reports_dir: Path, *, max_bytes: int, min_days: int, dry_run: bool) -> dict[str, object]:
    file_paths = [reports_dir / name for name in HISTORY_FILES]
    file_paths.extend(
        reports_dir / name
        for name in OPTIONAL_HISTORY_FILES
        if (reports_dir / name).is_file()
    )
    file_paths.extend(
        path
        for path in sorted(reports_dir.glob("observation_timeline_*.json"))
        if path.name != "observation_timeline_latest.json"
    )
    snapshot_paths = [reports_dir / name for name in SNAPSHOT_DIRECTORIES if (reports_dir / name).is_dir()]
    transition_csv_path = reports_dir / TRANSITION_CSV
    transition_csv_lines = (
        transition_csv_path.read_text(encoding="utf-8").splitlines(keepends=True)
        if transition_csv_path.is_file()
        else None
    )
    legacy_transition_paths = sorted(reports_dir.glob(LEGACY_TRANSITION_GLOB))
    missing = [str(path) for path in file_paths if not path.is_file()]
    if missing:
        raise ValueError("required history file is missing: " + ", ".join(missing))
    records_by_file = {
        path: (
            read_json_record(path)
            if path.name.startswith("observation_timeline_") and path.suffix == ".json"
            else read_records(path)
        )
        for path in file_paths
    }
    records_by_file.update(
        {
            path: records
            for path in snapshot_paths
            if (records := _read_snapshot_records(path))
        }
    )
    if transition_csv_lines is not None:
        transition_records = records_by_file[reports_dir / "state_transitions.jsonl"]
        transition_data_lines = transition_csv_lines[1:]
        if len(transition_data_lines) != len(transition_records):
            raise ValueError(f"{transition_csv_path}: CSV rows do not match state transition records")
        records_by_file[transition_csv_path] = [
            Record(record.market_date, line.encode("utf-8"))
            for record, line in zip(transition_records, transition_data_lines)
        ]
    cutoff = choose_cutoff(records_by_file, max_bytes=max_bytes, min_days=min_days)
    before = {
        str(path): (
            path.stat().st_size
            if path.is_file()
            else sum(item.stat().st_size for item in path.glob("*.json"))
        )
        for path in [*file_paths, *snapshot_paths, *([transition_csv_path] if transition_csv_lines is not None else []), *legacy_transition_paths]
    }
    if cutoff is None:
        if legacy_transition_paths:
            after = dict(before)
            for legacy_path in legacy_transition_paths:
                after[str(legacy_path)] = 0
                if not dry_run:
                    legacy_path.unlink()
            return {
                "action": "dry_run" if dry_run else "legacy_cleanup",
                "cutoff": None,
                "before_bytes": before,
                "after_bytes": after,
            }
        if not dry_run:
            for path, records in records_by_file.items():
                if (
                    path.name.startswith("observation_timeline_")
                    and path.suffix == ".json"
                    and path.read_bytes() != records[0].payload
                ):
                    _write_records(path, records, records[0].market_date)
        return {"action": "none", "cutoff": None, "before_bytes": before, "after_bytes": before}

    after = (
        {
            str(path): _retained_size(records, cutoff)
            for path, records in records_by_file.items()
            if path != transition_csv_path
        }
        if dry_run
        else {
            **{
                str(path): _write_records(path, records, cutoff)
                for path, records in records_by_file.items()
                if path.is_file() and path != transition_csv_path
            },
            **{
                str(path): _write_snapshot_records(path, cutoff)
                for path in snapshot_paths
                if path in records_by_file
            },
        }
    )
    if transition_csv_lines is not None:
        if dry_run:
            after[str(transition_csv_path)] = sum(
                len(line.encode("utf-8"))
                for line in [
                    transition_csv_lines[0],
                    *[
                        line
                        for record, line in zip(
                            records_by_file[reports_dir / "state_transitions.jsonl"],
                            transition_csv_lines[1:],
                        )
                        if record.market_date >= cutoff
                    ],
                ]
            )
        else:
            after[str(transition_csv_path)] = _write_transition_csv(
                transition_csv_path,
                records_by_file[reports_dir / "state_transitions.jsonl"],
                cutoff,
                transition_csv_lines,
            )
    for legacy_path in legacy_transition_paths:
        legacy_path.unlink()
        after[str(legacy_path)] = 0
    state_path = reports_dir / "observation_history_state.json"
    if cutoff is not None and not dry_run and state_path.is_file():
        state = json.loads(state_path.read_text(encoding="utf-8"))
        retained_dates = sorted(
            {
                record.market_date
                for path, records in records_by_file.items()
                if path.name in {"timeline_snapshots", "observation_timeline.jsonl"}
                for record in records
                if record.market_date >= cutoff
            }
        )
        if retained_dates:
            state["count"] = len(retained_dates)
            state["last_market_date"] = retained_dates[-1].isoformat()
            state_path.write_text(json.dumps(state, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return {
        "action": "dry_run" if dry_run else "pruned",
        "cutoff": cutoff.isoformat(),
        "min_days": min_days,
        "max_bytes": max_bytes,
        "before_bytes": before,
        "after_bytes": after,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reports-dir", type=Path, default=Path("reports"))
    parser.add_argument("--max-bytes", type=int, default=DEFAULT_MAX_BYTES)
    parser.add_argument("--min-days", type=int, default=DEFAULT_MIN_DAYS)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    if args.max_bytes <= 0 or args.min_days <= 0:
        parser.error("--max-bytes and --min-days must be positive")
    try:
        result = prune_reports(
            args.reports_dir,
            max_bytes=args.max_bytes,
            min_days=args.min_days,
            dry_run=args.dry_run,
        )
    except (OSError, ValueError) as exc:
        print(f"ERROR: data history cleanup failed: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
