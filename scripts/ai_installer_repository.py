"""Read-only repository facts and the installer Git operation boundary."""

import subprocess  # nosec B404
from dataclasses import dataclass
from pathlib import Path

from ai_common import clean_git_environment as clean_governance_environment


def git_target_args(target: Path) -> list[str]:
    return [f"--git-dir={target / '.git'}", f"--work-tree={target}"]


def clean_git_environment() -> dict[str, str]:
    environment = clean_governance_environment()
    # Repository detection is strictly read-only; prevent Git's optional background
    # maintenance from creating transient lock files during fact collection.
    environment["GIT_OPTIONAL_LOCKS"] = "0"
    return environment


def run_git(target: Path, args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(  # nosec B603 B607
        ["git", "-c", "maintenance.auto=false", *git_target_args(target), *args],
        cwd=target,
        text=True,
        capture_output=True,
        check=False,
        env=clean_git_environment(),
    )


def git_records(output: str) -> list[str]:
    return (
        [item for item in output.split("\0") if item]
        if "\0" in output
        else [line for line in output.splitlines() if line]
    )


@dataclass(frozen=True)
class RepositoryFacts:
    """Immutable Git facts captured before an installer confirmation."""

    root: Path
    commit: str | None
    branch: str | None
    remote: str | None
    remote_url: str | None
    default_branch: str | None
    clean: bool
    tracked_hygiene: tuple[str, ...]
    conflicts: tuple[str, ...]
    active_work_items: tuple[str, ...]
    symlink_risks: tuple[str, ...]

    def to_dict(self) -> dict[str, object]:
        """Return stable JSON-compatible facts for an Installation Plan."""
        return {
            "root": str(self.root),
            "commit": self.commit,
            "branch": self.branch,
            "remote": self.remote,
            "remoteUrl": self.remote_url,
            "defaultBranch": self.default_branch,
            "clean": self.clean,
            "trackedHygiene": list(self.tracked_hygiene),
            "conflicts": list(self.conflicts),
            "activeWorkItems": list(self.active_work_items),
            "symlinkRisks": list(self.symlink_risks),
        }


def _git_output(target: Path, args: list[str]) -> str:
    result = run_git(target, args)
    return result.stdout if result.returncode == 0 else ""


def read_repository_facts(target: Path) -> RepositoryFacts:
    """Capture installer facts without writing files, branches, or commits."""
    root = target.resolve()
    commit = _git_output(root, ["rev-parse", "--verify", "HEAD"]).strip() or None
    branch = _git_output(root, ["symbolic-ref", "--quiet", "--short", "HEAD"]).strip() or None
    status = _git_output(root, ["status", "--porcelain=v1", "-z"])
    records = git_records(status)
    conflicts = tuple(
        sorted({record[3:] for record in records if len(record) >= 3 and "U" in record[:2]})
    )
    hygiene_names = {".DS_Store", "Thumbs.db"}
    hygiene_suffixes = (".xcuserstate",)
    tracked = git_records(_git_output(root, ["ls-files", "-z"]))
    tracked_hygiene = tuple(
        sorted(
            path
            for path in tracked
            if Path(path).name in hygiene_names or path.endswith(hygiene_suffixes)
        )
    )
    remote = _git_output(root, ["remote"]).splitlines()
    remote_name = min(remote) if remote else None
    remote_url = (
        _git_output(root, ["remote", "get-url", remote_name]).strip() if remote_name else None
    )
    remote_head = (
        _git_output(
            root, ["symbolic-ref", "--quiet", "--short", f"refs/remotes/{remote_name}/HEAD"]
        ).strip()
        if remote_name
        else ""
    )
    default_branch = remote_head.rsplit("/", 1)[-1] if remote_head else None
    active_dir = root / ".ai" / "work-items" / "active"
    active = (
        tuple(sorted(path.name for path in active_dir.glob("*.contract.json")))
        if active_dir.is_dir()
        else ()
    )
    symlink_risks = tuple(
        sorted(
            path.relative_to(root).as_posix()
            for path in root.rglob("*")
            if path.is_symlink() and path.parts[-1] in {".ai", ".cursor", "scripts"}
        )
    )
    return RepositoryFacts(
        root=root,
        commit=commit,
        branch=branch,
        remote=remote_name,
        remote_url=remote_url,
        default_branch=default_branch,
        clean=not records,
        tracked_hygiene=tracked_hygiene,
        conflicts=conflicts,
        active_work_items=active,
        symlink_risks=symlink_risks,
    )
