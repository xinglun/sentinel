"""Build the evidence-bound Implementation Knowledge projection."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
from collections.abc import Iterable
from datetime import date
from pathlib import Path
from typing import Any

STATUSES = {"verified", "unknown", "incomplete"}
KNOWLEDGE_STATES = {"verified", "partial", "unknown", "superseded"}
EFFECTIVE_STATES = {"current", "superseded", "unknown", "historical_or_current_unknown"}
DATE_PATTERN = "%Y-%m-%d"


def _load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError(f"expected JSON object: {path}")
    return value


def _digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _relative(path: Path, root: Path) -> str:
    return path.resolve().relative_to(root.resolve()).as_posix()


def _canonical(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _claim_status(value: Any) -> str:
    if not isinstance(value, dict):
        return "unknown"
    status = value.get("status")
    if status == "complete":
        return "verified"
    if status in STATUSES:
        return status
    return "unknown"


def _approach_is_verified(value: Any) -> bool:
    if not isinstance(value, dict) or value.get("status") != "complete":
        return False
    claims: list[Any] = [value.get("summary"), value.get("mechanism")]
    claims.extend(value.get("affectedComponents", []))
    claims.extend(value.get("designDecisions", []))
    claims.extend(value.get("technicalDetails", []))
    claims.append(value)
    for claim in claims:
        if isinstance(claim, dict) and "status" in claim and _claim_status(claim) != "verified":
            return False
    return True


def _evidence_items(value: Any) -> list[dict[str, Any]]:
    items: list[dict[str, Any]] = []
    if isinstance(value, dict):
        if isinstance(value.get("evidence"), list):
            for item in value["evidence"]:
                if isinstance(item, dict):
                    items.append(item)
        for child in value.values():
            if isinstance(child, (dict, list)):
                items.extend(_evidence_items(child))
    elif isinstance(value, list):
        for child in value:
            if isinstance(child, (dict, list)):
                items.extend(_evidence_items(child))
    return items


def _evidence_path(item: dict[str, Any]) -> str | None:
    for key in ("source", "path"):
        value = item.get(key)
        if isinstance(value, str) and value.strip():
            return value
    return None


def _safe_evidence(path_text: str, root: Path) -> tuple[Path | None, str | None]:
    path = Path(path_text)
    if path.is_absolute() or ".." in path.parts or "" in path.parts:
        return None, "evidence path must be normalized and repository-relative"
    candidate = (root / path).resolve()
    try:
        candidate.relative_to(root.resolve())
    except ValueError:
        return None, "evidence path escapes repository"
    if not candidate.is_file():
        return None, "evidence path does not exist"
    return candidate, None


def _record_evidence(approach: Any, root: Path, issues: list[str]) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    seen: set[str] = set()
    for item in _evidence_items(approach):
        path_text = _evidence_path(item)
        if path_text is None or path_text in seen:
            continue
        seen.add(path_text)
        candidate, error = _safe_evidence(path_text, root)
        if error:
            issues.append(f"{path_text}: {error}")
            continue
        if candidate is None:
            issues.append(f"{path_text}: evidence path resolution returned no file")
            continue
        declared = item.get("digest")
        actual = _digest(candidate)
        if declared is not None and declared != actual:
            issues.append(f"{path_text}: evidence digest is stale")
        evidence_type = item.get("type")
        if not isinstance(evidence_type, str):
            evidence_type = "test" if path_text.startswith("tests/") else "code"
        entry: dict[str, Any] = {"type": evidence_type, "path": path_text}
        # Always freeze the observed evidence digest.  A later checker can
        # therefore detect a changed file even when the source claim omitted a
        # digest, while a stale declared digest still invalidates the claim.
        entry["digest"] = actual
        result.append(entry)
    return result


def _changes(summary: dict[str, Any]) -> list[str]:
    values = summary.get("actualChanges", summary.get("changedFiles", []))
    result: list[str] = []
    if isinstance(values, list):
        for value in values:
            path = value.get("path") if isinstance(value, dict) else value
            if isinstance(path, str) and path not in result:
                result.append(path)
    return result


def _explicit_field(
    name: str,
    sources: list[tuple[str, Any]],
    issues: list[str],
) -> tuple[Any, bool]:
    """Read a field only when an authoritative source explicitly supplies it."""
    values: list[tuple[str, Any]] = []
    for label, source in sources:
        if isinstance(source, dict) and name in source and source[name] is not None:
            values.append((label, source[name]))
    if not values:
        return None, False
    first = values[0][1]
    if any(_canonical(value) != _canonical(first) for _, value in values[1:]):
        labels = ", ".join(label for label, _ in values)
        issues.append(f"explicit {name} values disagree across sources: {labels}")
    return first, True


def _explicit_date(value: Any, issues: list[str]) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str):
        issues.append("explicit date must be a YYYY-MM-DD string")
        return None
    try:
        parsed = date.fromisoformat(value)
    except ValueError:
        issues.append("explicit date must be a calendar date in YYYY-MM-DD format")
        return None
    normalized = parsed.strftime(DATE_PATTERN)
    if value != normalized:
        issues.append("explicit date must be normalized as YYYY-MM-DD")
        return None
    return value


def _explicit_supersedes(value: Any, issues: list[str]) -> list[str]:
    if value is None:
        return []
    if not isinstance(value, list) or any(not isinstance(item, str) or not item for item in value):
        issues.append("supersedes must be an array of non-empty Work Item IDs")
        return []
    return list(dict.fromkeys(value))


def build_record(
    contract_path: Path,
    summary_path: Path,
    outcome_path: Path,
    *,
    repo_root: Path,
) -> dict[str, Any]:
    contract = _load(contract_path)
    summary = _load(summary_path)
    outcome = _load(outcome_path)
    work_item_id = contract.get("workItemId")
    if not isinstance(work_item_id, str) or not work_item_id:
        raise ValueError("Contract workItemId is required")

    issues: list[str] = []
    if summary.get("workItemId") != work_item_id:
        issues.append("Summary workItemId does not match Contract")
    outcome_id = outcome.get("workItemId")
    bindings = outcome.get("bindings")
    if outcome_id is None and isinstance(bindings, dict):
        outcome_id = bindings.get("taskId")
    if outcome_id != work_item_id:
        issues.append("Outcome workItemId does not match Contract")

    summary_approach = summary.get("implementationApproach")
    outcome_sections = outcome.get("sections")
    outcome_approach = (
        outcome_sections.get("implementationApproach")
        if isinstance(outcome_sections, dict)
        else None
    )
    if summary_approach is not None and outcome_approach is not None:
        if _canonical(summary_approach) != _canonical(outcome_approach):
            issues.append("Summary and Outcome Implementation Approach disagree")
    elif summary_approach is not None or outcome_approach is not None:
        issues.append("Summary and Outcome Implementation Approach are incomplete")

    approach = summary_approach if summary_approach is not None else outcome_approach
    evidence = _record_evidence(approach, repo_root, issues) if approach is not None else []
    approach_verified = _approach_is_verified(approach) and bool(evidence) and not issues
    if approach is None:
        implementation = {"summary": "unknown", "status": "unknown"}
        unknowns = ["Implementation Approach was not recorded in Summary/Outcome"]
    else:
        summary_claim = approach.get("summary") if isinstance(approach, dict) else None
        implementation = {
            "summary": summary_claim.get("text", "unknown")
            if isinstance(summary_claim, dict)
            else "unknown",
            "status": "verified" if approach_verified else "incomplete",
        }
        unknowns = list(issues)
        if not approach_verified and not unknowns:
            unknowns.append("Implementation Approach is not evidence-complete")

    decisions = summary.get("designDecisions")
    if not isinstance(decisions, list) and isinstance(approach, dict):
        decisions = approach.get("designDecisions", [])
    if not isinstance(decisions, list):
        decisions = []
    projected_decisions: list[dict[str, Any]] = []
    for decision in decisions:
        if not isinstance(decision, dict):
            continue
        projected_decisions.append(
            {
                "decision": decision.get("decision", "unknown"),
                "reason": decision.get("reason", "unknown"),
                "status": "verified"
                if approach_verified and decision.get("status") == "verified"
                else "unknown",
            }
        )

    source_values = [
        ("Summary", summary),
        ("Outcome", outcome),
        ("Outcome sections", outcome_sections),
        ("Contract", contract),
    ]
    explicit_date_value, has_date = _explicit_field("date", source_values, issues)
    explicit_date = _explicit_date(explicit_date_value, issues)
    explicit_effective_state, has_effective_state = _explicit_field(
        "effectiveState", source_values, issues
    )
    if not has_effective_state:
        effective_state = "historical_or_current_unknown"
    elif explicit_effective_state not in EFFECTIVE_STATES:
        issues.append("effectiveState is not a supported explicit state")
        effective_state = "historical_or_current_unknown"
    else:
        effective_state = explicit_effective_state
    explicit_supersedes, _ = _explicit_field("supersedes", source_values, issues)
    supersedes = _explicit_supersedes(explicit_supersedes, issues)

    generated_from = {
        "contractPath": _relative(contract_path, repo_root),
        "contractDigest": _digest(contract_path),
        "summaryPath": _relative(summary_path, repo_root),
        "summaryDigest": _digest(summary_path),
        "outcomePath": _relative(outcome_path, repo_root),
        "outcomeDigest": _digest(outcome_path),
    }
    merged_commit = None
    if isinstance(bindings, dict) and bindings.get("lifecycleStage") == "post_merge":
        candidate = bindings.get("headCommit")
        if isinstance(candidate, str) and len(candidate) == 40:
            merged_commit = candidate

    # A legacy Work Item is known to exist but lacks the new projection fields.
    # Preserve that distinction: ``partial`` means the record is usable but
    # incomplete, while ``unknown`` is reserved for an unusable/undetermined
    # knowledge state supplied by a future source adapter.
    state = "verified" if approach_verified else "partial"
    record = {
        "schemaVersion": 1,
        "workItemId": work_item_id,
        "title": contract.get("title", work_item_id),
        "topics": summary.get("topics", []) if isinstance(summary.get("topics"), list) else [],
        "components": summary.get("components", [])
        if isinstance(summary.get("components"), list)
        else [],
        "implementation": implementation,
        "configuration": summary.get("configurationApproach"),
        "changes": _changes(summary),
        "designDecisions": projected_decisions,
        "effects": summary.get("effects", []) if isinstance(summary.get("effects"), list) else [],
        "evidence": evidence,
        "mergedCommit": merged_commit,
        "effectiveState": effective_state,
        "currentValidity": "unknown",
        "supersedes": supersedes,
        "generatedFrom": generated_from,
        "knowledgeState": state,
        "unknowns": unknowns,
    }
    if has_date and explicit_date is not None:
        record["date"] = explicit_date
    return record


def _atomic_write(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temp_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            json.dump(payload, handle, ensure_ascii=False, indent=2)
            handle.write("\n")
        os.replace(temp_name, path)
    finally:
        if os.path.exists(temp_name):
            os.unlink(temp_name)


def _write_if_changed(path: Path, payload: dict[str, Any]) -> bool:
    serialized = json.dumps(payload, ensure_ascii=False, indent=2) + "\n"
    if path.is_file() and path.read_text(encoding="utf-8") == serialized:
        return False
    _atomic_write(path, payload)
    return True


def build_index(records_dir: Path) -> dict[str, Any]:
    items: list[dict[str, Any]] = []
    for record_path in sorted(records_dir.glob("*.json")):
        record = _load(record_path)
        work_item_id = record.get("workItemId")
        if not isinstance(work_item_id, str):
            raise TypeError(f"record workItemId is required: {record_path}")
        items.append(
            {
                "workItemId": work_item_id,
                "title": record.get("title", work_item_id),
                "topics": record.get("topics", []),
                "components": record.get("components", []),
                "state": record.get("knowledgeState", "unknown"),
                "knowledgePath": f".ai/knowledge/work-items/{record_path.name}",
            }
        )
    items.sort(key=lambda item: item["workItemId"])
    return {"schemaVersion": 1, "workItems": items}


def _normalize_dependency_path(path_text: str) -> str:
    path = Path(path_text)
    if not path_text or path.is_absolute() or ".." in path.parts or "" in path.parts:
        raise ValueError(f"knowledge dependency path must be repository-relative: {path_text}")
    return path.as_posix()


def dependency_paths(record: dict[str, Any]) -> list[str]:
    """Return the generated source paths that determine one record's content."""
    paths: set[str] = set()
    generated = record.get("generatedFrom")
    if isinstance(generated, dict):
        for key in ("contractPath", "summaryPath", "outcomePath"):
            value = generated.get(key)
            if isinstance(value, str) and value:
                paths.add(_normalize_dependency_path(value))
    evidence = record.get("evidence")
    if isinstance(evidence, list):
        for item in evidence:
            if isinstance(item, dict) and isinstance(item.get("path"), str):
                paths.add(_normalize_dependency_path(item["path"]))
    return sorted(paths)


def build_dependency_index(records_dir: Path) -> dict[str, Any]:
    records: dict[str, dict[str, Any]] = {}
    by_path: dict[str, list[str]] = {}
    for record_path in sorted(records_dir.glob("*.json")):
        record = _load(record_path)
        work_item_id = record.get("workItemId")
        if not isinstance(work_item_id, str) or not work_item_id:
            raise TypeError(f"record workItemId is required: {record_path}")
        dependencies = dependency_paths(record)
        records[work_item_id] = {
            "recordPath": f".ai/knowledge/work-items/{record_path.name}",
            "dependencies": dependencies,
        }
        for path in dependencies:
            by_path.setdefault(path, []).append(work_item_id)
    for work_item_ids in by_path.values():
        work_item_ids.sort()
    return {
        "schemaVersion": 1,
        "records": {work_item_id: records[work_item_id] for work_item_id in sorted(records)},
        "byPath": {path: by_path[path] for path in sorted(by_path)},
    }


def validate_dependency_index(
    payload: Any,
    *,
    records_dir: Path | None = None,
) -> list[str]:
    """Validate dependency routing without hashing any source evidence."""
    issues: list[str] = []
    if not isinstance(payload, dict):
        return ["dependency index is not an object"]
    if payload.get("schemaVersion") != 1:
        issues.append("dependency index schemaVersion is unsupported")
    records = payload.get("records")
    by_path = payload.get("byPath")
    if not isinstance(records, dict):
        issues.append("dependency index records is not an object")
        records = {}
    if not isinstance(by_path, dict):
        issues.append("dependency index byPath is not an object")
        by_path = {}

    if records_dir is not None and records_dir.is_dir():
        expected_ids = {path.stem for path in records_dir.glob("*.json")}
        actual_ids = set(records)
        for work_item_id in sorted(expected_ids - actual_ids):
            issues.append(f"dependency index missing record: {work_item_id}")
        for work_item_id in sorted(actual_ids - expected_ids):
            issues.append(f"dependency index references missing record: {work_item_id}")

    for work_item_id, entry in records.items():
        if not isinstance(work_item_id, str) or not work_item_id:
            issues.append("dependency index record identity is invalid")
            continue
        if not isinstance(entry, dict):
            issues.append(f"dependency index record is not an object: {work_item_id}")
            continue
        expected_record_path = f".ai/knowledge/work-items/{work_item_id}.json"
        if entry.get("recordPath") != expected_record_path:
            issues.append(f"dependency index recordPath is stale: {work_item_id}")
        dependencies = entry.get("dependencies")
        if not isinstance(dependencies, list) or any(
            not isinstance(path, str) for path in dependencies
        ):
            issues.append(f"dependency index dependencies are invalid: {work_item_id}")
            continue
        if dependencies != sorted(set(dependencies)):
            issues.append(f"dependency index dependencies are not normalized: {work_item_id}")
        for path in dependencies:
            try:
                normalized = _normalize_dependency_path(path)
            except ValueError:
                issues.append(f"dependency index path is invalid: {path}")
                continue
            if normalized != path:
                issues.append(f"dependency index path is not normalized: {path}")
            if work_item_id not in by_path.get(path, []):
                issues.append(f"dependency index reverse mapping is incomplete: {path}")

    for path, work_item_ids in by_path.items():
        if not isinstance(path, str):
            issues.append("dependency index reverse path is invalid")
            continue
        try:
            normalized = _normalize_dependency_path(path)
        except ValueError:
            issues.append(f"dependency index reverse path is invalid: {path}")
            normalized = path
        if normalized != path:
            issues.append(f"dependency index reverse path is not normalized: {path}")
        if not isinstance(work_item_ids, list) or work_item_ids != sorted(set(work_item_ids)):
            issues.append(f"dependency index reverse mapping is not normalized: {path}")
            continue
        for work_item_id in work_item_ids:
            entry = records.get(work_item_id)
            if not isinstance(entry, dict) or path not in entry.get("dependencies", []):
                issues.append(f"dependency index reverse mapping is stale: {path}")
    return issues


def _index_entry(record: dict[str, Any]) -> dict[str, Any]:
    work_item_id = record.get("workItemId")
    if not isinstance(work_item_id, str) or not work_item_id:
        raise TypeError("record workItemId is required")
    return {
        "workItemId": work_item_id,
        "title": record.get("title", work_item_id),
        "topics": record.get("topics", []),
        "components": record.get("components", []),
        "state": record.get("knowledgeState", "unknown"),
        "knowledgePath": f".ai/knowledge/work-items/{work_item_id}.json",
    }


def _index_is_well_formed(payload: Any) -> bool:
    if not isinstance(payload, dict) or payload.get("schemaVersion") != 1:
        return False
    items = payload.get("workItems")
    if not isinstance(items, list):
        return False
    ids = [item.get("workItemId") for item in items if isinstance(item, dict)]
    if len(ids) != len(items) or not all(
        isinstance(work_item_id, str) and work_item_id for work_item_id in ids
    ):
        return False
    typed_ids = [work_item_id for work_item_id in ids if isinstance(work_item_id, str)]
    return typed_ids == sorted(set(typed_ids))


def rebuild_dependency_index(
    records_dir: Path,
    output_path: Path,
    *,
    record_updates: dict[str, dict[str, Any]] | None = None,
    full: bool = False,
) -> dict[str, Any]:
    """Persist the dependency projection, routing only updated records normally."""
    if full or record_updates is None or not output_path.is_file():
        result = build_dependency_index(records_dir)
        _write_if_changed(output_path, result)
        return result

    try:
        result = _load(output_path)
    except (OSError, json.JSONDecodeError, TypeError, ValueError):
        result = {}
    if validate_dependency_index(result, records_dir=records_dir):
        result = build_dependency_index(records_dir)
        _write_if_changed(output_path, result)
        return result

    records = dict(result["records"])
    by_path = {path: list(work_item_ids) for path, work_item_ids in result["byPath"].items()}
    for work_item_id, record in record_updates.items():
        old_entry = records.get(work_item_id, {})
        old_dependencies = old_entry.get("dependencies", [])
        if isinstance(old_dependencies, list):
            for path in old_dependencies:
                work_item_ids = [item for item in by_path.get(path, []) if item != work_item_id]
                if work_item_ids:
                    by_path[path] = work_item_ids
                else:
                    by_path.pop(path, None)
        dependencies = dependency_paths(record)
        records[work_item_id] = {
            "recordPath": f".ai/knowledge/work-items/{work_item_id}.json",
            "dependencies": dependencies,
        }
        for path in dependencies:
            by_path.setdefault(path, [])
            if work_item_id not in by_path[path]:
                by_path[path].append(work_item_id)
            by_path[path].sort()
    result = {
        "schemaVersion": 1,
        "records": {work_item_id: records[work_item_id] for work_item_id in sorted(records)},
        "byPath": {path: sorted(by_path[path]) for path in sorted(by_path) if by_path[path]},
    }
    _write_if_changed(output_path, result)
    return result


def rebuild_index(
    records_dir: Path,
    output_path: Path,
    *,
    record_updates: dict[str, dict[str, Any]] | None = None,
    full: bool = False,
) -> dict[str, Any]:
    """Persist the query index and its dependency routing projection."""
    dependency_path = output_path.with_name("dependencies.json")
    if full or record_updates is None or not output_path.is_file():
        result = build_index(records_dir)
        _write_if_changed(output_path, result)
        rebuild_dependency_index(records_dir, dependency_path, full=True)
        return result

    try:
        result = _load(output_path)
    except (OSError, json.JSONDecodeError, TypeError, ValueError):
        result = {}
    if not _index_is_well_formed(result):
        result = build_index(records_dir)
        _write_if_changed(output_path, result)
        rebuild_dependency_index(records_dir, dependency_path, full=True)
        return result

    entries = {item["workItemId"]: item for item in result["workItems"] if isinstance(item, dict)}
    for record in record_updates.values():
        entry = _index_entry(record)
        entries[entry["workItemId"]] = entry
    result = {
        "schemaVersion": 1,
        "workItems": [entries[work_item_id] for work_item_id in sorted(entries)],
    }
    _write_if_changed(output_path, result)
    rebuild_dependency_index(
        records_dir,
        dependency_path,
        record_updates=record_updates,
    )
    return result


def rebuild_existing_projections(
    *,
    repo_root: Path,
    changed_paths: Iterable[str] | None = None,
    include_work_item_ids: Iterable[str] = (),
) -> list[str]:
    """Refresh only archived records routed by changed evidence paths.

    A missing, malformed, or internally inconsistent dependency projection is
    an explicit full-rebuild condition.  The normal path reads the routing
    projection and the selected archived sources only; the authoritative
    checker still validates every record separately.
    """
    records_dir = repo_root / ".ai" / "knowledge" / "work-items"
    index_path = repo_root / ".ai" / "knowledge" / "index.json"
    record_paths = sorted(records_dir.glob("*.json")) if records_dir.is_dir() else []
    if not record_paths and not index_path.is_file():
        return []
    if not records_dir.is_dir():
        raise ValueError("Implementation Knowledge records directory is missing")

    dependency_path = index_path.with_name("dependencies.json")
    include_ids = set(include_work_item_ids)
    explicit_full = changed_paths is None and not include_ids
    dependency_payload: dict[str, Any] | None = None
    if not explicit_full and dependency_path.is_file():
        try:
            candidate = _load(dependency_path)
        except (OSError, json.JSONDecodeError, TypeError, ValueError):
            candidate = None
        if not validate_dependency_index(candidate, records_dir=records_dir):
            dependency_payload = candidate

    record_path_by_id = {path.stem: path for path in record_paths}
    full_rebuild = explicit_full or dependency_payload is None
    if full_rebuild:
        selected_ids = set(record_path_by_id)
    else:
        if dependency_payload is None:
            raise ValueError("Implementation Knowledge dependency index is unavailable")
        selected_ids = set(include_ids)
        for path_text in changed_paths or ():
            normalized = _normalize_dependency_path(path_text)
            selected_ids.update(dependency_payload["byPath"].get(normalized, []))
    unknown_ids = sorted(selected_ids - set(record_path_by_id))
    if unknown_ids:
        raise ValueError(
            "Implementation Knowledge dependency routing references missing records: "
            + ", ".join(unknown_ids)
        )

    archive_dir = repo_root / ".ai" / "work-items" / "archive"
    changed: list[str] = []
    record_updates: dict[str, dict[str, Any]] = {}
    for work_item_id in sorted(selected_ids):
        record_path = record_path_by_id[work_item_id]
        record = _load(record_path)
        loaded_work_item_id = record.get("workItemId")
        if loaded_work_item_id != work_item_id:
            raise ValueError(f"knowledge record has no Work Item identity: {record_path}")
        matches = sorted(archive_dir.glob(f"*/{work_item_id}.contract.json"))
        if len(matches) != 1:
            raise ValueError(
                f"knowledge record {record_path.name} does not have exactly one archived Contract"
            )
        contract_path = matches[0]
        summary_path = contract_path.with_name(f"{work_item_id}.summary.json")
        outcome_path = contract_path.with_name(f"{work_item_id}.outcome.json")
        if not summary_path.is_file() or not outcome_path.is_file():
            raise ValueError(
                f"archived Work Item {work_item_id} is missing Summary or Outcome evidence"
            )
        record_payload = build_record(
            contract_path,
            summary_path,
            outcome_path,
            repo_root=repo_root,
        )
        serialized = json.dumps(record_payload, ensure_ascii=False, indent=2) + "\n"
        if record_path.read_text(encoding="utf-8") != serialized:
            _atomic_write(record_path, record_payload)
            changed.append(_relative(record_path, repo_root))
        record_updates[work_item_id] = record_payload

    before_index = index_path.read_text(encoding="utf-8") if index_path.is_file() else None
    before_dependencies = (
        dependency_path.read_text(encoding="utf-8") if dependency_path.is_file() else None
    )
    rebuild_index(
        records_dir,
        index_path,
        record_updates=record_updates if not full_rebuild else None,
        full=full_rebuild,
    )
    if index_path.read_text(encoding="utf-8") != before_index:
        changed.append(_relative(index_path, repo_root))
    if dependency_path.read_text(encoding="utf-8") != before_dependencies:
        changed.append(_relative(dependency_path, repo_root))
    return changed


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--contract", type=Path, required=True)
    parser.add_argument("--summary", type=Path, required=True)
    parser.add_argument("--outcome", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--index", type=Path, required=True)
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    args = parser.parse_args()
    record = build_record(args.contract, args.summary, args.outcome, repo_root=args.repo_root)
    _write_if_changed(args.output, record)
    rebuild_index(
        args.output.parent,
        args.index,
        record_updates={record["workItemId"]: record},
    )
    print(json.dumps(record, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
