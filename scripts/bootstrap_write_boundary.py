"""Fail-closed Bootstrap write planning and execution boundary.

The planner is deliberately separate from the Wizard session and repository
detector. It validates every proposed path before reading or writing, and the
executor performs no I/O unless confirmation and an optional drift check pass.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path


class BoundaryError(RuntimeError):
    """Raised when a Bootstrap write would cross a declared safety boundary."""


BEGIN = "# AI_COCKPIT_MANAGED_BEGIN\n"
END = "# AI_COCKPIT_MANAGED_END\n"
CONFIRMATION = "CONFIRM"


@dataclass(frozen=True)
class PlannedWrite:
    """One validated relative-path write."""

    relative_path: str
    content: str


@dataclass(frozen=True)
class WritePlan:
    """A complete, validated set of writes for one repository root."""

    root: Path
    writes: tuple[PlannedWrite, ...]
    dry_run: bool = False


@dataclass(frozen=True)
class ExecutionResult:
    """Observable result of a dry-run, pending, or confirmed execution."""

    planned: tuple[str, ...]
    written: tuple[str, ...]
    dry_run: bool


def _relative_path(value: str) -> Path:
    if not isinstance(value, str) or not value or "\x00" in value:
        raise BoundaryError("invalid path")
    path = Path(value)
    if path.is_absolute() or ".." in path.parts:
        raise BoundaryError(f"path escapes repository: {value}")
    return path


def _allowed(path: Path, allowed_paths: set[str]) -> bool:
    value = path.as_posix()
    return value in allowed_paths or any(
        item.endswith("/") and value.startswith(item) for item in allowed_paths
    )


def _managed_content(existing: str, block: str) -> str:
    if BEGIN in existing or END in existing:
        if existing.count(BEGIN) != 1 or existing.count(END) != 1:
            raise BoundaryError("malformed managed Makefile marker")
        begin = existing.index(BEGIN)
        end = existing.index(END, begin) + len(END)
        return existing[:begin] + BEGIN + block.rstrip("\n") + "\n" + END + existing[end:]
    separator = "" if not existing or existing.endswith("\n\n") else "\n"
    return existing + separator + BEGIN + block.rstrip("\n") + "\n" + END


def build_plan(
    root: str | Path,
    proposed: Mapping[str, str],
    allowed_paths: set[str],
    *,
    managed_makefile_block: str | None = None,
    dry_run: bool = False,
) -> WritePlan:
    """Validate proposed writes and return a plan without modifying ``root``."""
    repository = Path(root).expanduser().resolve()
    if not repository.is_dir():
        raise BoundaryError("repository root does not exist")
    normalized_allowed = {_relative_path(item).as_posix() for item in allowed_paths}
    writes: list[PlannedWrite] = []
    for raw_path, content in proposed.items():
        path = _relative_path(raw_path)
        if not _allowed(path, normalized_allowed):
            raise BoundaryError(f"path is outside the write allowlist: {raw_path}")
        target = repository / path
        if target.is_symlink():
            raise BoundaryError(f"symlink path is not writable: {raw_path}")
        if not isinstance(content, str):
            raise BoundaryError(f"content must be text: {raw_path}")
        value = content
        if path.as_posix() == "Makefile" and managed_makefile_block is not None:
            existing = target.read_text(encoding="utf-8") if target.exists() else ""
            value = _managed_content(existing, managed_makefile_block)
        writes.append(PlannedWrite(path.as_posix(), value))
    return WritePlan(repository, tuple(writes), dry_run=dry_run)


def execute_plan(
    plan: WritePlan,
    *,
    confirmed: bool = False,
    confirmation_value: str | None = None,
    non_interactive: bool = False,
    dry_run: bool | None = None,
    drift_check: Callable[[], Mapping[str, object]] | None = None,
) -> ExecutionResult:
    """Execute a plan only after confirmation and drift validation.

    Dry-run and unconfirmed calls report planned paths and perform zero writes.
    A confirmed call revalidates drift immediately before the first write; any
    failure stops before mutation.
    """
    planned = tuple(item.relative_path for item in plan.writes)
    is_dry_run = plan.dry_run if dry_run is None else dry_run
    if is_dry_run or not confirmed:
        return ExecutionResult(planned=planned, written=(), dry_run=is_dry_run)
    if confirmation_value != CONFIRMATION:
        raise BoundaryError("explicit confirmation is required before writing")
    if non_interactive and confirmation_value != CONFIRMATION:
        raise BoundaryError("non-interactive execution requires confirmation")
    if drift_check is not None:
        report = drift_check()
        if not report.get("ok", False):
            raise BoundaryError(f"repository drift blocks writing: {report.get('mismatches', {})}")
    for item in plan.writes:
        target = plan.root / _relative_path(item.relative_path)
        if target.is_symlink():
            raise BoundaryError(f"symlink path is not writable: {item.relative_path}")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(item.content, encoding="utf-8")
    return ExecutionResult(planned=planned, written=planned, dry_run=False)
