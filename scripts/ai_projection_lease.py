"""Serialize branch-integrated generated projections across linked Work Items."""

from __future__ import annotations

import fcntl
import json
import os
import subprocess  # nosec B404 - fixed list-form Git calls are reviewed below
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path

from ai_common import PROJECT_ROOT, clean_git_environment

BRANCH_INTEGRATED_GENERATED_PATHS = frozenset(
    {
        ".ai/cockpit/current_status.md",
        ".ai/cockpit/task_report.json",
        ".ai/cockpit/task_report.md",
        ".ai/work-items/archive/index.json",
    }
)


class ProjectionLeaseError(RuntimeError):
    """Raised when a branch-integrated projection is not safely owned."""


@dataclass(frozen=True)
class ProjectionLease:
    task: str
    branch: str
    base_commit: str
    acquired_at: str

    def as_dict(self) -> dict[str, str]:
        return {
            "task": self.task,
            "branch": self.branch,
            "baseCommit": self.base_commit,
            "acquiredAt": self.acquired_at,
        }


def requires_lease(contract: object) -> bool:
    """Return whether the Contract explicitly opts into parallel projections."""
    return isinstance(contract, dict) and contract.get("concurrencyBoundary") is not None


def _git(root: Path, *args: str) -> str:
    result = subprocess.run(  # nosec B603 B607 - fixed Git commands
        ["git", *args],
        cwd=root,
        env=clean_git_environment(),
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise ProjectionLeaseError(result.stderr.strip() or f"git {' '.join(args)} failed")
    return result.stdout.strip()


def common_git_dir(*, root: Path = PROJECT_ROOT) -> Path:
    value = _git(root, "rev-parse", "--git-common-dir")
    path = Path(value)
    return path if path.is_absolute() else (root / path).resolve()


def lease_path(*, root: Path = PROJECT_ROOT) -> Path:
    return common_git_dir(root=root) / "ai-cockpit-projection-lease.json"


def _decode(value: str) -> ProjectionLease | None:
    if not value.strip():
        return None
    try:
        payload = json.loads(value)
    except json.JSONDecodeError as exc:
        raise ProjectionLeaseError(
            "branch-integrated projection lease is unreadable; do not remove it"
        ) from exc
    if not isinstance(payload, dict):
        raise ProjectionLeaseError(
            "branch-integrated projection lease is malformed; do not remove it"
        )
    fields = ("task", "branch", "baseCommit", "acquiredAt")
    if any(not isinstance(payload.get(field), str) or not payload[field] for field in fields):
        raise ProjectionLeaseError(
            "branch-integrated projection lease is incomplete; do not remove it"
        )
    return ProjectionLease(
        task=payload["task"],
        branch=payload["branch"],
        base_commit=payload["baseCommit"],
        acquired_at=payload["acquiredAt"],
    )


@contextmanager
def _locked_lease(*, root: Path = PROJECT_ROOT) -> Iterator[tuple[Path, object]]:
    path = lease_path(root=root)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a+", encoding="utf-8") as handle:
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
        try:
            yield path, handle
        finally:
            fcntl.flock(handle.fileno(), fcntl.LOCK_UN)


def _read(handle: object) -> ProjectionLease | None:
    handle.seek(0)  # type: ignore[attr-defined]
    return _decode(handle.read())  # type: ignore[attr-defined]


def _write(handle: object, lease: ProjectionLease | None) -> None:
    handle.seek(0)  # type: ignore[attr-defined]
    handle.truncate()  # type: ignore[attr-defined]
    if lease is not None:
        json.dump(lease.as_dict(), handle, ensure_ascii=False, sort_keys=True)  # type: ignore[arg-type]
        handle.write("\n")  # type: ignore[attr-defined]
    handle.flush()  # type: ignore[attr-defined]
    os.fsync(handle.fileno())  # type: ignore[attr-defined]


def current_branch(*, root: Path = PROJECT_ROOT) -> str:
    branch = _git(root, "branch", "--show-current")
    if not branch:
        raise ProjectionLeaseError(
            "branch-integrated projection lease requires a dedicated Work Item branch"
        )
    return branch


def require_fresh_base(*, root: Path = PROJECT_ROOT) -> str:
    """Require the exact worktree to include current origin/main before writes."""
    _git(root, "fetch", "origin", "main")
    result = subprocess.run(  # nosec B603 B607 - fixed Git command
        ["git", "merge-base", "--is-ancestor", "origin/main", "HEAD"],
        cwd=root,
        env=clean_git_environment(),
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise ProjectionLeaseError(
            "branch-integrated projections require the latest origin/main before Finish/archive; "
            "resume or rebase this Work Item from the merged base, then retry"
        )
    return _git(root, "rev-parse", "origin/main")


def acquire(task: str, *, root: Path = PROJECT_ROOT) -> ProjectionLease:
    """Acquire or resume a persistent lease for the exact Work Item branch."""
    branch = current_branch(root=root)
    expected = f"codex/{task}"
    if branch != expected:
        raise ProjectionLeaseError(
            f"branch-integrated projection lease requires {expected}, found {branch}"
        )
    base = require_fresh_base(root=root)
    with _locked_lease(root=root) as (_path, handle):
        existing = _read(handle)
        if existing is not None and (existing.task != task or existing.branch != branch):
            raise ProjectionLeaseError(
                "branch-integrated projections are owned by "
                f"{existing.task} on {existing.branch} since {existing.acquired_at}; "
                "wait for its PR merge and ai-close-work-item lifecycle closure, then refresh this Work Item from origin/main"
            )
        lease = existing or ProjectionLease(
            task=task,
            branch=branch,
            base_commit=base,
            acquired_at=datetime.now(UTC).isoformat(),
        )
        _write(handle, lease)
        return lease


def release(task: str, branch: str, *, root: Path = PROJECT_ROOT) -> None:
    """Release only the exact owner after successful lifecycle closure."""
    with _locked_lease(root=root) as (_path, handle):
        existing = _read(handle)
        if existing is None:
            return
        if existing.task != task or existing.branch != branch:
            raise ProjectionLeaseError(
                "cannot release branch-integrated projection lease owned by "
                f"{existing.task} on {existing.branch}"
            )
        _write(handle, None)


def inventory() -> dict[str, object]:
    """Return the closed inventory consumed by parallel-start validation."""
    return {
        "schemaVersion": 1,
        "serialized": sorted(BRANCH_INTEGRATED_GENERATED_PATHS),
        "taskNamespaced": [
            ".ai/work-items/starts/<task>.json",
            ".ai/work-items/active/<task>.*",
            ".ai/work-items/archive/YYYY/<task>.*",
        ],
        "policy": "serialized projections require one persistent lease from Finish through ai-close-work-item",
    }
