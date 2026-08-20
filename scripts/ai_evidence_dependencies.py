#!/usr/bin/env python3
"""Derive Capability Truth matrix dependencies for governance guards."""

from __future__ import annotations

import json
import stat
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from types import MappingProxyType
from typing import Any

from ai_common import matches

MATRIX_PATH = "docs/reference/capability-truth-matrix.json"
MARKDOWN_PATH = "docs/reference/capability-truth-matrix.md"
SOURCE_BOUND_GENERATED_DOCUMENTATION_PATHS = (
    MATRIX_PATH,
    "docs/reference/pre-release-documentation-alignment.json",
    "docs/reference/pre-release-documentation-alignment.md",
)
SOURCE_BOUND_GENERATED_EVIDENCE_MODE = "canonical_generators"


class EvidenceDependencyError(ValueError):
    """Raised when Capability Truth dependencies cannot be loaded safely."""


@dataclass(frozen=True)
class EvidenceDependencies:
    """Immutable evidence paths and the capability rows that depend on them."""

    matrix_path: str
    capability_ids_by_path: Mapping[str, tuple[str, ...]]
    source_paths: tuple[str, ...]
    test_paths: tuple[str, ...]


def source_bound_evidence_is_affected(paths: list[str], dependencies: EvidenceDependencies) -> bool:
    """Return whether a change requires the bounded source-evidence gate.

    The gate is intentionally conditional: unrelated Work Items must not pay
    for a repository-wide evidence recheck, while any changed input or
    generated projection must be validated before expensive quality checks.
    """
    changed = set(paths)
    if changed.intersection(SOURCE_BOUND_GENERATED_DOCUMENTATION_PATHS):
        return True
    return any(path in dependencies.capability_ids_by_path for path in changed)


def _matrix_error(detail: str) -> EvidenceDependencyError:
    return EvidenceDependencyError(f"{MATRIX_PATH}: {detail}")


def _lstat(path: Path) -> Any | None:
    try:
        return path.lstat()
    except FileNotFoundError:
        return None
    except OSError as exc:
        raise _matrix_error(f"cannot inspect configured path: {exc}") from exc


def _normalize_evidence_path(raw_path: str, location: str) -> str:
    path = PurePosixPath(raw_path)
    if path.is_absolute():
        raise _matrix_error(f"{location} must be repository-relative: {raw_path}")
    if not raw_path or path.as_posix() == ".":
        raise _matrix_error(f"{location} must name an evidence file: {raw_path!r}")
    if ".." in path.parts:
        raise _matrix_error(f"{location} escapes repository root: {raw_path}")
    return path.as_posix()


def _require_regular_evidence_file(root: Path, relative_path: str) -> None:
    candidate = root
    final_stat: Any | None = None
    for part in PurePosixPath(relative_path).parts:
        candidate /= part
        try:
            final_stat = candidate.lstat()
        except FileNotFoundError as exc:
            raise _matrix_error(f"capability evidence file is missing: {relative_path}") from exc
        except OSError as exc:
            raise _matrix_error(
                f"capability evidence path cannot be inspected: {relative_path}: {exc}"
            ) from exc
        if stat.S_ISLNK(final_stat.st_mode):
            raise _matrix_error(
                f"capability evidence path must not contain a symbolic link: {relative_path}"
            )

    try:
        candidate.resolve(strict=True).relative_to(root)
    except (OSError, ValueError) as exc:
        raise _matrix_error(
            f"capability evidence path escapes repository root: {relative_path}"
        ) from exc
    if final_stat is None or not stat.S_ISREG(final_stat.st_mode):
        raise _matrix_error(f"capability evidence path must be a regular file: {relative_path}")


def _load_matrix(matrix_path: Path) -> dict[str, Any]:
    matrix_stat = _lstat(matrix_path)
    if matrix_stat is None:
        raise _matrix_error("configured document set is missing its JSON matrix")
    if stat.S_ISLNK(matrix_stat.st_mode) or not stat.S_ISREG(matrix_stat.st_mode):
        raise _matrix_error("configured JSON matrix must be a regular file")
    try:
        value = json.loads(matrix_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise _matrix_error(f"cannot load JSON matrix: {exc}") from exc
    if not isinstance(value, dict):
        raise _matrix_error("matrix root must be an object")
    return value


def load_capability_evidence_dependencies(
    root: Path,
) -> EvidenceDependencies | None:
    """Load the configured matrix and return its validated dependency graph."""
    resolved_root = root.resolve(strict=True)
    matrix_path = resolved_root / MATRIX_PATH
    markdown_path = resolved_root / MARKDOWN_PATH
    matrix_stat = _lstat(matrix_path)
    markdown_stat = _lstat(markdown_path)
    if matrix_stat is None and markdown_stat is None:
        return None

    matrix = _load_matrix(matrix_path)
    rows = matrix.get("capabilities")
    if not isinstance(rows, list) or not rows:
        raise _matrix_error("capabilities must be a non-empty list")

    capability_ids: set[str] = set()
    capability_ids_by_path: dict[str, set[str]] = {}
    source_paths: set[str] = set()
    test_paths: set[str] = set()
    for index, row in enumerate(rows):
        row_location = f"capabilities[{index}]"
        if not isinstance(row, dict):
            raise _matrix_error(f"{row_location} must be an object")
        identifier = row.get("id")
        if not isinstance(identifier, str) or not identifier:
            raise _matrix_error(f"{row_location}.id must be a non-empty string")
        if identifier in capability_ids:
            raise _matrix_error(f"duplicate capability id: {identifier}")
        capability_ids.add(identifier)

        row_paths: set[str] = set()
        for field, aggregate in (
            ("sourceEvidence", source_paths),
            ("testEvidence", test_paths),
        ):
            values = row.get(field)
            field_location = f"{row_location}.{field}"
            if not isinstance(values, list) or not values:
                raise _matrix_error(f"{field_location} must be a non-empty list")
            for path_index, raw_path in enumerate(values):
                location = f"{field_location}[{path_index}]"
                if not isinstance(raw_path, str):
                    raise _matrix_error(f"{location} must be a string")
                normalized = _normalize_evidence_path(raw_path, location)
                if normalized in row_paths:
                    raise _matrix_error(
                        f"duplicate evidence path alias in {row_location}: {raw_path}"
                    )
                row_paths.add(normalized)
                _require_regular_evidence_file(resolved_root, normalized)
                aggregate.add(normalized)
                capability_ids_by_path.setdefault(normalized, set()).add(identifier)

    sorted_mapping = {
        path: tuple(sorted(capability_ids_by_path[path])) for path in sorted(capability_ids_by_path)
    }
    return EvidenceDependencies(
        matrix_path=MATRIX_PATH,
        capability_ids_by_path=MappingProxyType(sorted_mapping),
        source_paths=tuple(sorted(source_paths)),
        test_paths=tuple(sorted(test_paths)),
    )


def _dependency_issue(
    *,
    action: str,
    path: str,
    capability_ids: tuple[str, ...],
    matrix_path: str,
) -> str:
    identifiers = ", ".join(capability_ids)
    return (
        f"Capability Truth evidence dependency requires {action} "
        f"{matrix_path}: {path} is bound to capabilities [{identifiers}]"
    )


def contract_scope_dependency_issues(
    scope: list[str], dependencies: EvidenceDependencies
) -> list[str]:
    """Report scoped evidence whose matrix is not also covered by scope."""
    if any(matches(pattern, dependencies.matrix_path) for pattern in scope):
        return []
    return [
        _dependency_issue(
            action="Contract scope coverage for",
            path=path,
            capability_ids=capability_ids,
            matrix_path=dependencies.matrix_path,
        )
        for path, capability_ids in dependencies.capability_ids_by_path.items()
        if any(matches(pattern, path) for pattern in scope)
    ]


def changed_path_dependency_issues(
    paths: list[str], dependencies: EvidenceDependencies
) -> list[str]:
    """Report changed evidence when the matrix was not actually regenerated."""
    changed = set(paths)
    if dependencies.matrix_path in changed:
        return []
    return [
        _dependency_issue(
            action="a changed",
            path=path,
            capability_ids=capability_ids,
            matrix_path=dependencies.matrix_path,
        )
        for path, capability_ids in dependencies.capability_ids_by_path.items()
        if path in changed
    ]


def source_bound_generated_evidence_policy_issues(
    contract: Mapping[str, Any], dependencies: EvidenceDependencies
) -> list[str]:
    """Require an exact bounded policy when a v2 Contract scopes Capability Truth evidence."""
    scope = contract.get("scope")
    if (
        contract.get("contractVersion") != 2
        or not isinstance(scope, list)
        or not any(
            isinstance(pattern, str) and matches(pattern, path)
            for pattern in scope
            for path in dependencies.capability_ids_by_path
        )
    ):
        return []
    policy = contract.get("sourceBoundGeneratedEvidence")
    if policy is None:
        return []
    if not isinstance(policy, Mapping):
        return ["sourceBoundGeneratedEvidence must be an object"]
    paths = policy.get("generatedPaths")
    if (
        policy.get("mode") != SOURCE_BOUND_GENERATED_EVIDENCE_MODE
        or not isinstance(paths, list)
        or set(paths) != set(SOURCE_BOUND_GENERATED_DOCUMENTATION_PATHS)
        or len(paths) != len(SOURCE_BOUND_GENERATED_DOCUMENTATION_PATHS)
    ):
        return [
            "sourceBoundGeneratedEvidence.generatedPaths must declare exactly the canonical generated paths"
        ]
    missing = [
        path
        for path in SOURCE_BOUND_GENERATED_DOCUMENTATION_PATHS
        if not any(matches(pattern, path) for pattern in scope)
    ]
    return (
        ["sourceBoundGeneratedEvidence requires Contract scope coverage for: " + ", ".join(missing)]
        if missing
        else []
    )


def source_bound_generated_evidence_change_issues(
    contract: Mapping[str, Any], paths: list[str], dependencies: EvidenceDependencies
) -> list[str]:
    """Require canonical outputs for a changed bound source and reject extra docs."""
    if "sourceBoundGeneratedEvidence" not in contract:
        return []
    changed = set(paths)
    if not any(path in changed for path in dependencies.capability_ids_by_path):
        return []
    issues = source_bound_generated_evidence_policy_issues(contract, dependencies)
    if issues:
        return issues
    issues.extend(
        "source-bound generated evidence required generated path is absent from the diff: " + path
        for path in SOURCE_BOUND_GENERATED_DOCUMENTATION_PATHS
        if path not in changed
    )
    issues.extend(
        "sourceBoundGeneratedEvidence does not authorize changed non-generated documentation: "
        + path
        for path in sorted(changed)
        if path.startswith("docs/") and path not in SOURCE_BOUND_GENERATED_DOCUMENTATION_PATHS
    )
    return issues
