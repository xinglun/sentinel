"""Validate evidence-bound implementation knowledge records and index drift."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from ai_generate_knowledge_record import (
    _digest,
    _load,
    _safe_evidence,
    build_dependency_index,
    build_index,
    validate_dependency_index,
)

EFFECTIVE_STATES = {"current", "superseded", "unknown", "historical_or_current_unknown"}


def _digest_issue(path: Path, expected: Any, label: str) -> str | None:
    if not isinstance(expected, str):
        return f"{label}: digest is missing"
    normalized = expected.removeprefix("sha256:")
    if normalized != _digest(path):
        return f"{label}: digest is stale"
    return None


def check_record(record_path: Path, *, repo_root: Path) -> list[str]:
    issues: list[str] = []
    try:
        record = _load(record_path)
    except (OSError, json.JSONDecodeError, TypeError, ValueError) as exc:
        return [f"record {record_path}: cannot load ({exc})"]

    work_item_id = record.get("workItemId")
    if not isinstance(work_item_id, str) or not work_item_id:
        issues.append("record workItemId is missing")
    elif record_path.stem != work_item_id:
        issues.append("record filename does not match workItemId")

    effective_state = record.get("effectiveState")
    if effective_state is not None and effective_state not in EFFECTIVE_STATES:
        issues.append("effectiveState is not a supported state")
    supersedes = record.get("supersedes", [])
    if not isinstance(supersedes, list) or any(
        not isinstance(item, str) or not item for item in supersedes
    ):
        issues.append("supersedes must be an array of non-empty Work Item IDs")

    generated = record.get("generatedFrom")
    if not isinstance(generated, dict):
        issues.append("generatedFrom is missing")
    else:
        for path_key, digest_key in (
            ("contractPath", "contractDigest"),
            ("summaryPath", "summaryDigest"),
            ("outcomePath", "outcomeDigest"),
        ):
            path_text = generated.get(path_key)
            if not isinstance(path_text, str):
                issues.append(f"generatedFrom.{path_key} is missing")
                continue
            candidate, error = _safe_evidence(path_text, repo_root)
            if error or candidate is None:
                issues.append(f"generatedFrom.{path_key}: {error or 'invalid path'}")
                continue
            digest_issue = _digest_issue(
                candidate, generated.get(digest_key), f"generatedFrom.{path_key}"
            )
            if digest_issue:
                issues.append(digest_issue)

    evidence = record.get("evidence")
    if not isinstance(evidence, list) or not evidence:
        if record.get("knowledgeState") == "verified":
            issues.append("verified record has no evidence")
    else:
        for item in evidence:
            if not isinstance(item, dict):
                issues.append("evidence entry is not an object")
                continue
            path_text = item.get("path")
            if not isinstance(path_text, str):
                issues.append("evidence path is missing")
                continue
            candidate, error = _safe_evidence(path_text, repo_root)
            if error or candidate is None:
                issues.append(f"evidence {path_text}: {error or 'invalid path'}")
                continue
            digest_issue = _digest_issue(candidate, item.get("digest"), f"evidence {path_text}")
            if digest_issue:
                issues.append(digest_issue)

    if record.get("knowledgeState") == "verified":
        implementation = record.get("implementation")
        if not isinstance(implementation, dict) or implementation.get("status") != "verified":
            issues.append("verified record has no verified implementation claim")
        if record.get("unknowns"):
            issues.append("verified record contains unknowns")
        if issues:
            issues.append("verified record is not currently valid")
    return issues


def check_index(index_path: Path, *, records_dir: Path, repo_root: Path) -> list[str]:
    issues: list[str] = []
    try:
        actual = _load(index_path)
    except (OSError, json.JSONDecodeError, TypeError, ValueError) as exc:
        return [f"index {index_path}: cannot load ({exc})"]

    expected = build_index(records_dir)
    if actual != expected:
        issues.append("index does not match deterministic rebuild")

    dependency_path = index_path.with_name("dependencies.json")
    try:
        dependency_actual = _load(dependency_path)
    except (OSError, json.JSONDecodeError, TypeError, ValueError) as exc:
        issues.append(f"dependency index cannot load ({exc})")
        dependency_actual = None
    if validate_dependency_index(dependency_actual, records_dir=records_dir):
        issues.append("dependency index is malformed or incomplete")
    elif dependency_actual != build_dependency_index(records_dir):
        issues.append("dependency index does not match deterministic rebuild")

    seen: set[str] = set()
    records_by_id: dict[str, dict[str, Any]] = {}
    for record_path in sorted(records_dir.glob("*.json")):
        record_issues = check_record(record_path, repo_root=repo_root)
        issues.extend(f"{record_path.name}: {issue}" for issue in record_issues)
        try:
            record = _load(record_path)
        except (OSError, json.JSONDecodeError, ValueError):
            continue
        work_item_id = record.get("workItemId")
        if isinstance(work_item_id, str):
            if work_item_id in seen:
                issues.append(f"duplicate workItemId: {work_item_id}")
            seen.add(work_item_id)
            records_by_id[work_item_id] = record

    for work_item_id, record in records_by_id.items():
        supersedes = record.get("supersedes", [])
        if not isinstance(supersedes, list):
            continue
        for target in supersedes:
            if isinstance(target, str) and target not in records_by_id:
                issues.append(f"{work_item_id}: supersedes missing record: {target}")

    visit_state: dict[str, int] = {}

    def visit(work_item_id: str) -> None:
        state = visit_state.get(work_item_id, 0)
        if state == 1:
            issues.append(f"supersession cycle includes: {work_item_id}")
            return
        if state == 2:
            return
        visit_state[work_item_id] = 1
        supersedes = records_by_id[work_item_id].get("supersedes", [])
        if isinstance(supersedes, list):
            for target in supersedes:
                if isinstance(target, str) and target in records_by_id:
                    visit(target)
        visit_state[work_item_id] = 2

    for work_item_id in sorted(records_by_id):
        visit(work_item_id)

    indexed_paths = {
        item.get("knowledgePath") for item in actual.get("workItems", []) if isinstance(item, dict)
    }
    for record_path in sorted(records_dir.glob("*.json")):
        expected_path = f".ai/knowledge/work-items/{record_path.name}"
        if expected_path not in indexed_paths:
            issues.append(f"index missing record: {record_path.name}")
    for item in actual.get("workItems", []):
        if not isinstance(item, dict):
            issues.append("index workItems entry is not an object")
            continue
        knowledge_path = item.get("knowledgePath")
        if not isinstance(knowledge_path, str) or not knowledge_path:
            issues.append("index entry knowledgePath is missing")
            continue
        record_name = Path(knowledge_path).name
        if not (records_dir / record_name).is_file():
            issues.append(f"index references missing record: {record_name}")
    return issues


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--index", type=Path, required=True)
    parser.add_argument("--records", type=Path, required=True)
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    args = parser.parse_args()
    issues = check_index(args.index, records_dir=args.records, repo_root=args.repo_root)
    if issues:
        print(json.dumps({"status": "invalid", "issues": issues}, ensure_ascii=False, indent=2))
        return 1
    print(json.dumps({"status": "valid", "index": str(args.index)}, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
