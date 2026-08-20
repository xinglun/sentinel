#!/usr/bin/env python3
"""Query evidence-bound Implementation Knowledge records deterministically."""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import asdict, dataclass
from datetime import date
from pathlib import Path
from typing import Any

from ai_check_knowledge_index import check_index

KNOWLEDGE_STATES = {"verified", "partial", "unknown", "superseded"}
DATE_PATTERN = re.compile(r"^\d{4}-\d{2}-\d{2}$")


class KnowledgeQueryError(ValueError):
    """Raised when the authoritative knowledge input cannot be queried safely."""


@dataclass(frozen=True)
class QueryFilters:
    """Exact structured filters supported by the query interface."""

    work_item_id: str | None = None
    topic: str | None = None
    component: str | None = None
    commit: str | None = None
    date_exact: str | None = None
    status: str | None = None
    date_from: str | None = None
    date_to: str | None = None

    def validate(self) -> None:
        if self.status is not None and self.status not in KNOWLEDGE_STATES:
            allowed = ", ".join(sorted(KNOWLEDGE_STATES))
            raise KnowledgeQueryError(f"status must be one of: {allowed}")
        if self.commit is not None and not re.fullmatch(r"[0-9a-f]{40}", self.commit):
            raise KnowledgeQueryError("commit must be a 40-character lowercase hexadecimal SHA")
        for field, value in (
            ("date", self.date_exact),
            ("date-from", self.date_from),
            ("date-to", self.date_to),
        ):
            if value is not None:
                _parse_date(value, field)
        if self.date_from and self.date_to and self.date_from > self.date_to:
            raise KnowledgeQueryError("date-from must not be later than date-to")


def _parse_date(value: str, label: str) -> date:
    if not DATE_PATTERN.fullmatch(value):
        raise KnowledgeQueryError(f"{label} must use YYYY-MM-DD")
    try:
        return date.fromisoformat(value)
    except ValueError as exc:
        raise KnowledgeQueryError(f"{label} is not a calendar date") from exc


def _load_object(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise KnowledgeQueryError(f"cannot load {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise KnowledgeQueryError(f"{path} must contain a JSON object")
    return value


def _safe_repo_path(path_text: str, *, repo_root: Path) -> Path:
    path = Path(path_text)
    if path.is_absolute() or ".." in path.parts or "" in path.parts:
        raise KnowledgeQueryError(f"knowledge path is not repository-relative: {path_text}")
    candidate = (repo_root / path).resolve()
    try:
        candidate.relative_to(repo_root.resolve())
    except ValueError as exc:
        raise KnowledgeQueryError(f"knowledge path escapes repository: {path_text}") from exc
    return candidate


def _record_date(record: dict[str, Any]) -> str | None:
    value = record.get("date")
    return value if isinstance(value, str) and value else None


def _matches(record: dict[str, Any], filters: QueryFilters) -> bool:
    if filters.work_item_id is not None and record.get("workItemId") != filters.work_item_id:
        return False
    if filters.topic is not None and filters.topic not in record.get("topics", []):
        return False
    if filters.component is not None and filters.component not in record.get("components", []):
        return False
    if filters.commit is not None and record.get("mergedCommit") != filters.commit:
        return False
    if filters.status is not None and record.get("knowledgeState") != filters.status:
        return False

    requested_dates = any(
        value is not None for value in (filters.date_exact, filters.date_from, filters.date_to)
    )
    if not requested_dates:
        return True
    value = _record_date(record)
    if value is None:
        return False
    candidate = _parse_date(value, "record date")
    if filters.date_exact is not None and value != filters.date_exact:
        return False
    if filters.date_from is not None and candidate < _parse_date(filters.date_from, "date-from"):
        return False
    return not (filters.date_to is not None and candidate > _parse_date(filters.date_to, "date-to"))


def _validate_authoritative_inputs(*, index_path: Path, records_dir: Path, repo_root: Path) -> None:
    issues = check_index(index_path, records_dir=records_dir, repo_root=repo_root)
    if issues:
        raise KnowledgeQueryError("knowledge index is invalid: " + "; ".join(issues))


def _supersession_projection(
    records: dict[str, dict[str, Any]],
) -> dict[str, tuple[str | None, str]]:
    """Resolve only explicit supersession edges; never infer recency."""
    superseded_by: dict[str, list[str]] = {work_item_id: [] for work_item_id in records}
    for work_item_id, record in records.items():
        targets = record.get("supersedes", [])
        if not isinstance(targets, list):
            raise KnowledgeQueryError(f"supersedes must be an array: {work_item_id}")
        for target in targets:
            if not isinstance(target, str) or target not in records:
                raise KnowledgeQueryError(
                    f"supersession target is missing: {work_item_id} -> {target}"
                )
            superseded_by[target].append(work_item_id)

    result: dict[str, tuple[str | None, str]] = {}

    def latest(work_item_id: str, trail: tuple[str, ...]) -> tuple[set[str], bool]:
        if work_item_id in trail:
            raise KnowledgeQueryError("supersession cycle: " + " -> ".join((*trail, work_item_id)))
        children = superseded_by[work_item_id]
        if not children:
            return {work_item_id}, False
        leaves: set[str] = set()
        for child in children:
            child_leaves, _ = latest(child, (*trail, work_item_id))
            leaves.update(child_leaves)
        return leaves, False

    for work_item_id in sorted(records):
        leaves, _ = latest(work_item_id, ())
        if len(leaves) == 1:
            latest_id = next(iter(leaves))
            result[work_item_id] = (
                latest_id,
                "current" if latest_id == work_item_id else "superseded",
            )
        else:
            result[work_item_id] = (None, "conflict")
    return result


def query_knowledge(
    *,
    repo_root: Path,
    index_path: Path,
    records_dir: Path,
    filters: QueryFilters,
) -> dict[str, Any]:
    """Return a stable, read-only query result over validated knowledge records."""
    filters.validate()
    repo_root = repo_root.resolve()
    index_path = index_path.resolve()
    records_dir = records_dir.resolve()
    _validate_authoritative_inputs(
        index_path=index_path, records_dir=records_dir, repo_root=repo_root
    )
    index = _load_object(index_path)
    entries = index.get("workItems")
    if not isinstance(entries, list):
        raise KnowledgeQueryError("knowledge index workItems must be an array")

    records: dict[str, dict[str, Any]] = {}
    knowledge_paths: dict[str, str] = {}
    for entry in entries:
        if not isinstance(entry, dict):
            raise KnowledgeQueryError("knowledge index contains a non-object entry")
        knowledge_path = entry.get("knowledgePath")
        if not isinstance(knowledge_path, str) or not knowledge_path:
            raise KnowledgeQueryError("knowledge index entry has no knowledgePath")
        record_path = _safe_repo_path(knowledge_path, repo_root=repo_root)
        if not record_path.is_file():
            raise KnowledgeQueryError(f"missing record: {knowledge_path}")
        record = _load_object(record_path)
        if record.get("workItemId") != entry.get("workItemId"):
            raise KnowledgeQueryError(f"record identity mismatch: {knowledge_path}")
        work_item_id = record.get("workItemId")
        if not isinstance(work_item_id, str) or work_item_id in records:
            raise KnowledgeQueryError(f"duplicate or missing Work Item identity: {knowledge_path}")
        records[work_item_id] = record
        knowledge_paths[work_item_id] = knowledge_path

    supersession = _supersession_projection(records)
    matches: list[dict[str, Any]] = []
    for work_item_id, record in records.items():
        if _matches(record, filters):
            latest_id, supersession_status = supersession[work_item_id]
            matches.append(
                {
                    "workItemId": work_item_id,
                    "knowledgePath": knowledge_paths[work_item_id],
                    "state": record.get("knowledgeState", "unknown"),
                    "latestKnownRecord": latest_id,
                    "supersessionStatus": supersession_status,
                    "record": record,
                }
            )

    matches.sort(key=lambda item: (item["record"]["workItemId"], item["knowledgePath"]))
    return {
        "schemaVersion": 1,
        "query": asdict(filters),
        "matchedCount": len(matches),
        "results": matches,
        # ``matches`` remains a compatibility alias for callers of the first
        # query projection.  It is deliberately the same deterministic list.
        "matches": matches,
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--index", type=Path)
    parser.add_argument("--records-dir", type=Path)
    parser.add_argument("--work-item-id", "--work-item", dest="work_item_id")
    parser.add_argument("--topic")
    parser.add_argument("--component")
    parser.add_argument("--commit")
    parser.add_argument("--date", dest="date_exact")
    parser.add_argument("--status", choices=sorted(KNOWLEDGE_STATES))
    parser.add_argument("--date-from")
    parser.add_argument("--date-to")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    repo_root = args.repo_root.resolve()
    index_path = args.index or repo_root / ".ai" / "knowledge" / "index.json"
    records_dir = args.records_dir or repo_root / ".ai" / "knowledge" / "work-items"
    filters = QueryFilters(
        work_item_id=args.work_item_id,
        topic=args.topic,
        component=args.component,
        commit=args.commit,
        date_exact=args.date_exact,
        status=args.status,
        date_from=args.date_from,
        date_to=args.date_to,
    )
    try:
        result = query_knowledge(
            repo_root=repo_root,
            index_path=index_path,
            records_dir=records_dir,
            filters=filters,
        )
    except KnowledgeQueryError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
