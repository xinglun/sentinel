"""Pure diff-bound evidence reuse policy built on the WI-07 binding foundation.

This module canonicalizes repository-relative changed paths and evaluates one
diff-bound binding against current source and policy identity.  It produces
evidence only: existing verification gates retain authority to execute checks.
"""

from __future__ import annotations

import re
from collections.abc import Mapping, Sequence
from datetime import UTC, datetime
from pathlib import PurePosixPath
from typing import Any

from ai_evidence_binding import BindingError, canonical_digest, validate_binding

_COMMIT = re.compile(r"^[0-9a-f]{40}$")
_DIGEST = re.compile(r"^sha256:[0-9a-f]{64}$")
_CURRENT_FIELDS = frozenset(
    {"baseCommit", "headCommit", "changedPaths", "scopeDigest", "governanceDigest"}
)


class DiffReuseError(ValueError):
    """Raised when a diff identity cannot be represented canonically."""


def _require_commit(value: object, field: str) -> str:
    if not isinstance(value, str) or not _COMMIT.fullmatch(value):
        raise DiffReuseError(f"{field} must be a 40-character lowercase commit")
    return value


def _require_digest(value: object, field: str) -> str:
    if not isinstance(value, str) or not _DIGEST.fullmatch(value):
        raise DiffReuseError(f"{field} must be a sha256 digest")
    return value


def canonicalize_changed_paths(paths: Sequence[str]) -> tuple[str, ...]:
    """Return a sorted unique tuple of safe repository-relative POSIX paths.

    Git path sets are order-independent, so equivalent ``./`` and repeated
    separators are normalized before sorting.  Duplicate paths after
    normalization are rejected: a malformed producer must rerun rather than
    silently changing the identity it meant to bind.
    """

    if isinstance(paths, (str, bytes)) or not isinstance(paths, Sequence):
        raise DiffReuseError("changedPaths must be a list of repository-relative paths")
    normalized: list[str] = []
    for raw_path in paths:
        if not isinstance(raw_path, str) or not raw_path or raw_path != raw_path.strip():
            raise DiffReuseError("changedPaths contains an empty or malformed path")
        if "\\" in raw_path or "\x00" in raw_path:
            raise DiffReuseError("changedPaths must use repository-relative POSIX paths")
        path = PurePosixPath(raw_path)
        if path.is_absolute() or path.as_posix() in {"", "."} or ".." in path.parts:
            raise DiffReuseError("changedPaths must be repository-relative")
        value = path.as_posix()
        if value in normalized:
            raise DiffReuseError(f"changedPaths contains duplicate path: {value}")
        normalized.append(value)
    return tuple(sorted(normalized))


def changed_paths_digest(paths: Sequence[str]) -> str:
    """Return the digest of the canonical changed-path sequence."""

    return canonical_digest(list(canonicalize_changed_paths(paths)))


def build_current_diff(
    *,
    base_commit: str,
    head_commit: str,
    changed_paths: Sequence[str],
    scope_digest: str,
    governance_digest: str,
) -> dict[str, Any]:
    """Build a validated current diff input without retaining caller state."""

    normalized = canonicalize_changed_paths(changed_paths)
    return {
        "baseCommit": _require_commit(base_commit, "baseCommit"),
        "headCommit": _require_commit(head_commit, "headCommit"),
        "changedPaths": list(normalized),
        "scopeDigest": _require_digest(scope_digest, "scopeDigest"),
        "governanceDigest": _require_digest(governance_digest, "governanceDigest"),
    }


def _unknown_current_reasons(current: object) -> list[str]:
    if not isinstance(current, Mapping):
        return ["current_diff_unknown"]
    missing = _CURRENT_FIELDS - set(current)
    missing_reasons = [
        reason
        for field, reason in (
            ("baseCommit", "base_commit_unknown"),
            ("headCommit", "head_commit_unknown"),
            ("changedPaths", "changed_paths_unknown"),
            ("scopeDigest", "security_scope_unknown"),
            ("governanceDigest", "governance_policy_unknown"),
        )
        if field in missing
    ]
    if missing_reasons:
        return missing_reasons
    if set(current) != _CURRENT_FIELDS:
        return ["current_diff_unknown"]
    try:
        _require_commit(current.get("baseCommit"), "baseCommit")
        _require_commit(current.get("headCommit"), "headCommit")
        canonicalize_changed_paths(current.get("changedPaths"))  # type: ignore[arg-type]
        _require_digest(current.get("scopeDigest"), "scopeDigest")
        _require_digest(current.get("governanceDigest"), "governanceDigest")
    except DiffReuseError:
        return ["current_diff_invalid"]
    return []


def decide_diff_reuse(
    binding: Mapping[str, Any], current: Mapping[str, Any], *, now: datetime
) -> dict[str, Any]:
    """Return deterministic exact-match reuse or fail-closed rerun evidence."""

    try:
        validate_binding(binding)
    except (BindingError, TypeError):
        return {"state": "unknown", "action": "rerun", "reasons": ["binding_invalid"]}
    if binding.get("classification") != "diff-bound":
        return {
            "state": "unknown",
            "action": "rerun",
            "reasons": ["binding_classification_invalid"],
        }
    if now.tzinfo is None:
        return {"state": "unknown", "action": "rerun", "reasons": ["current_time_unknown"]}

    current_reasons = _unknown_current_reasons(current)
    if current_reasons:
        return {"state": "unknown", "action": "rerun", "reasons": current_reasons}

    current_paths = canonicalize_changed_paths(current["changedPaths"])
    bound_diff = binding["dependencies"]["diff"]
    comparisons = (
        (current["baseCommit"], bound_diff["baseCommit"], "base_commit_mismatch"),
        (current["headCommit"], bound_diff["headCommit"], "head_commit_mismatch"),
        (
            changed_paths_digest(current_paths),
            bound_diff["changedPathsDigest"],
            "changed_paths_mismatch",
        ),
        (current["scopeDigest"], binding["scopeDigest"], "security_scope_mismatch"),
        (current["governanceDigest"], binding["governanceDigest"], "governance_policy_mismatch"),
    )
    reasons = [reason for actual, expected, reason in comparisons if actual != expected]
    expires_at = datetime.fromisoformat(binding["expiresAt"])
    if now.astimezone(UTC) > expires_at.astimezone(UTC):
        reasons.append("binding_expired")
    if reasons:
        return {"state": "stale", "action": "rerun", "reasons": reasons}
    return {"state": "fresh", "action": "reuse", "reasons": []}
