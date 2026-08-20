"""Read-only repository facts and Bootstrap confirmation drift checks."""

from __future__ import annotations

import subprocess
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any


class BootstrapRepositoryError(RuntimeError):
    """Raised when a path cannot provide the Git facts Bootstrap requires."""


@dataclass(frozen=True)
class Remote:
    """Fetch and push URLs for one Git remote."""

    fetch_url: str
    push_url: str


@dataclass(frozen=True)
class RepositorySnapshot:
    """Immutable, serializable repository facts captured without writes."""

    root: Path
    commit: str
    branch: str | None
    detached: bool
    staged_paths: tuple[str, ...]
    unstaged_paths: tuple[str, ...]
    untracked_paths: tuple[str, ...]
    dirty_paths: tuple[str, ...]
    remotes: dict[str, Remote]
    remote_head: str | None
    local_branches: tuple[str, ...]
    remote_branches: tuple[str, ...]
    conflict_paths: tuple[str, ...]
    installed_cockpit: bool
    bootstrap_base_commit: str | None = None

    def to_dict(self) -> dict[str, Any]:
        """Return a JSON-compatible snapshot for an external Session."""
        data = asdict(self)
        data["root"] = str(self.root)
        data["remotes"] = {name: asdict(remote) for name, remote in self.remotes.items()}
        for key in (
            "staged_paths",
            "unstaged_paths",
            "untracked_paths",
            "dirty_paths",
            "local_branches",
            "remote_branches",
            "conflict_paths",
        ):
            data[key] = list(data[key])
        return data


@dataclass(frozen=True)
class DriftReport:
    """Fail-closed result of comparing confirmed and current repository facts."""

    ok: bool
    mismatches: dict[str, dict[str, Any]]
    current: RepositorySnapshot

    def to_dict(self) -> dict[str, Any]:
        return {"ok": self.ok, "mismatches": self.mismatches, "current": self.current.to_dict()}


def _git(root: Path, *args: str, check: bool = True) -> str:
    result = subprocess.run(
        ["git", "-C", str(root), *args],
        text=True,
        capture_output=True,
        check=False,
    )
    if check and result.returncode != 0:
        raise BootstrapRepositoryError(result.stderr.strip() or f"git {' '.join(args)} failed")
    return result.stdout


def _records(output: str) -> tuple[str, ...]:
    if "\0" not in output:
        return tuple(item for item in output.splitlines() if item)
    return tuple(item for item in output.split("\0") if item)


def _status(
    root: Path,
) -> tuple[tuple[str, ...], tuple[str, ...], tuple[str, ...], tuple[str, ...], tuple[str, ...]]:
    records = _records(_git(root, "status", "--porcelain=v1", "-z"))
    staged: list[str] = []
    unstaged: list[str] = []
    untracked: list[str] = []
    conflicts: list[str] = []
    dirty: list[str] = []
    for record in records:
        if len(record) < 3:
            continue
        code, path = record[:2], record[3:]
        if code == "??":
            untracked.append(path)
        else:
            if code[0] not in {" ", "?"}:
                staged.append(path)
            if code[1] not in {" ", "?"}:
                unstaged.append(path)
            if "U" in code or code in {"AA", "DD"}:
                conflicts.append(path)
        dirty.append(path)
    return (
        tuple(sorted(staged)),
        tuple(sorted(unstaged)),
        tuple(sorted(untracked)),
        tuple(sorted(dirty)),
        tuple(sorted(set(conflicts))),
    )


def _remote_facts(root: Path) -> tuple[dict[str, Remote], str | None]:
    remotes: dict[str, Remote] = {}
    for name in _records(_git(root, "remote")):
        fetch = _git(root, "remote", "get-url", name).strip()
        push_result = subprocess.run(
            ["git", "-C", str(root), "remote", "get-url", "--push", name],
            text=True,
            capture_output=True,
            check=False,
        )
        push = push_result.stdout.strip() if push_result.returncode == 0 else fetch
        remotes[name] = Remote(fetch_url=fetch, push_url=push)
    heads: list[str] = []
    for name in remotes:
        ref = _git(
            root, "symbolic-ref", "--quiet", "--short", f"refs/remotes/{name}/HEAD", check=False
        ).strip()
        if ref:
            heads.append(ref)
    return remotes, (heads[0] if len(heads) == 1 else None)


def detect_repository(
    root: str | Path, *, bootstrap_base_commit: str | None = None
) -> RepositorySnapshot:
    """Capture Git and local Cockpit facts without modifying ``root``."""
    candidate = Path(root).expanduser().resolve()
    probe = subprocess.run(
        ["git", "-C", str(candidate), "rev-parse", "--show-toplevel"],
        text=True,
        capture_output=True,
        check=False,
    )
    if probe.returncode != 0:
        raise BootstrapRepositoryError(f"{candidate} is not a Git repository")
    actual_root = Path(probe.stdout.strip()).resolve()
    commit_result = subprocess.run(
        ["git", "-C", str(actual_root), "rev-parse", "HEAD"],
        text=True,
        capture_output=True,
        check=False,
    )
    if commit_result.returncode != 0:
        raise BootstrapRepositoryError(f"{actual_root} has no commit")
    branch = (
        _git(actual_root, "symbolic-ref", "--quiet", "--short", "HEAD", check=False).strip() or None
    )
    staged, unstaged, untracked, dirty, conflicts = _status(actual_root)
    remotes, remote_head = _remote_facts(actual_root)
    local = tuple(
        sorted(
            _records(_git(actual_root, "for-each-ref", "--format=%(refname:short)", "refs/heads/"))
        )
    )
    remote = tuple(
        sorted(
            _records(
                _git(actual_root, "for-each-ref", "--format=%(refname:short)", "refs/remotes/")
            )
        )
    )
    installed = (actual_root / ".ai" / "cockpit").is_dir() and (
        actual_root / "scripts" / "ai_start.py"
    ).is_file()
    return RepositorySnapshot(
        actual_root,
        commit_result.stdout.strip(),
        branch,
        branch is None,
        staged,
        unstaged,
        untracked,
        dirty,
        remotes,
        remote_head,
        local,
        remote,
        conflicts,
        installed,
        bootstrap_base_commit,
    )


def revalidate_repository(
    confirmed: RepositorySnapshot,
    *,
    current: RepositorySnapshot | None = None,
    root: str | Path | None = None,
) -> DriftReport:
    """Compare confirmed facts with current facts and block on every mismatch."""
    observed = current or detect_repository(
        root or confirmed.root, bootstrap_base_commit=confirmed.bootstrap_base_commit
    )
    fields = (
        "root",
        "branch",
        "commit",
        "dirty_paths",
        "remotes",
        "remote_head",
        "conflict_paths",
        "bootstrap_base_commit",
    )
    mismatches: dict[str, dict[str, Any]] = {}
    for field in fields:
        expected = getattr(confirmed, field)
        actual = getattr(observed, field)
        expected_value = (
            expected.to_dict() if isinstance(expected, RepositorySnapshot) else expected
        )
        actual_value = actual.to_dict() if isinstance(actual, RepositorySnapshot) else actual
        if field == "remotes":
            expected_value = {name: asdict(value) for name, value in expected.items()}
            actual_value = {name: asdict(value) for name, value in actual.items()}
        if isinstance(expected_value, tuple):
            expected_value, actual_value = list(expected_value), list(actual_value)
        if expected_value != actual_value:
            mismatches[field] = {"expected": expected_value, "actual": actual_value}
    return DriftReport(ok=not mismatches, mismatches=mismatches, current=observed)
