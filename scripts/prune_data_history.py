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


@dataclass(frozen=True)
class Record:
    market_date: date
    payload: bytes


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
    directory = path.parent
    with tempfile.NamedTemporaryFile("wb", dir=directory, prefix=f".{path.name}.", delete=False) as handle:
        temporary_path = Path(handle.name)
        for payload in kept:
            handle.write(payload)
    os.chmod(temporary_path, path.stat().st_mode)
    os.replace(temporary_path, path)
    return sum(len(payload) for payload in kept)


def prune_reports(reports_dir: Path, *, max_bytes: int, min_days: int, dry_run: bool) -> dict[str, object]:
    paths = {reports_dir / name: reports_dir / name for name in HISTORY_FILES}
    missing = [str(path) for path in paths if not path.is_file()]
    if missing:
        raise ValueError("required history file is missing: " + ", ".join(missing))
    records_by_file = {path: read_records(path) for path in paths}
    cutoff = choose_cutoff(records_by_file, max_bytes=max_bytes, min_days=min_days)
    before = {str(path): path.stat().st_size for path in paths}
    if cutoff is None:
        return {"action": "none", "cutoff": None, "before_bytes": before, "after_bytes": before}

    after = (
        {str(path): _retained_size(records, cutoff) for path, records in records_by_file.items()}
        if dry_run
        else {str(path): _write_records(path, records, cutoff) for path, records in records_by_file.items()}
    )
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
