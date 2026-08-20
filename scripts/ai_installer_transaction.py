"""Transaction boundary vocabulary for installer actions.

This module deliberately contains only local, fail-closed transaction primitives.
It does not decide whether a source is trusted; callers must classify the source
before acquiring a write lock.
"""

import json
import os
import subprocess  # nosec B404 - invokes fixed git executable with fixed arguments
from dataclasses import dataclass
from enum import Enum
from pathlib import Path


@dataclass(frozen=True)
class TransactionAction:
    kind: str
    path: Path
    detail: str


class SourceMode(str, Enum):
    """Explicit identity of the installer source presented to an operator."""

    RELEASE_VERIFIED = "RELEASE_VERIFIED"
    LOCAL_CLEAN_COMMIT = "LOCAL_CLEAN_COMMIT"
    LOCAL_DIRTY_WORKTREE = "LOCAL_DIRTY_WORKTREE"
    CUSTOM_SOURCE = "CUSTOM_SOURCE"
    PRIVATE_MIRROR = "PRIVATE_MIRROR"
    UNKNOWN_SOURCE = "UNKNOWN_SOURCE"


@dataclass(frozen=True)
class SourceClassification:
    """Read-only source identity and the reason it was classified."""

    mode: SourceMode
    source: Path
    reason: str


def _git(source: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(  # nosec B603 B607 - executable and arguments are fixed
        ["git", "-C", str(source), *args],
        text=True,
        capture_output=True,
        check=False,
        env={**os.environ, "GIT_OPTIONAL_LOCKS": "0"},
    )


def classify_source(source: Path) -> SourceClassification:
    """Classify a source without writing or treating local code as a release.

    A release archive is verified only when its release metadata is structurally
    complete and no Git checkout is present. A checkout is always local/custom,
    even if it happens to contain historical release metadata.
    """
    root = source.resolve()
    release = root / "release.json"
    version = root / ".ai" / "cockpit" / "version.json"
    if not version.is_file():
        return SourceClassification(SourceMode.UNKNOWN_SOURCE, root, "version metadata is missing")
    git = _git(root, "rev-parse", "--is-inside-work-tree")
    if git.returncode == 0 and git.stdout.strip() == "true":
        status = _git(root, "status", "--porcelain")
        if status.returncode != 0:
            return SourceClassification(
                SourceMode.UNKNOWN_SOURCE, root, "Git status could not be read"
            )
        if status.stdout.strip():
            mode = SourceMode.LOCAL_DIRTY_WORKTREE
            reason = "source is a dirty Git worktree"
        else:
            mode = SourceMode.LOCAL_CLEAN_COMMIT
            reason = "source is a clean Git commit"
        if os.environ.get("AI_COCKPIT_TEMPLATE_PRIVATE_MIRROR") == "1":
            mode = SourceMode.PRIVATE_MIRROR
            reason = "source is marked as a private mirror"
        elif os.environ.get("AI_COCKPIT_TEMPLATE_CUSTOM_SOURCE") == "1":
            mode = SourceMode.CUSTOM_SOURCE
            reason = "source is explicitly marked custom"
        return SourceClassification(mode, root, reason)
    if release.is_file():
        try:
            data = json.loads(release.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, json.JSONDecodeError):
            data = None
        if isinstance(data, dict) and all(
            isinstance(data.get(key), str) and data[key]
            for key in ("releaseTag", "releaseEvidenceAuthority", "installerDigest")
        ):
            return SourceClassification(
                SourceMode.RELEASE_VERIFIED, root, "release metadata is structurally complete"
            )
    return SourceClassification(
        SourceMode.UNKNOWN_SOURCE, root, "source identity is not verifiable"
    )


@dataclass
class WritePlan:
    """Ordered, auditable list of intended transaction actions."""

    actions: list[TransactionAction]

    def add(self, action: TransactionAction) -> None:
        if action.path in {item.path for item in self.actions} and action.kind not in {
            "skip",
            "backup",
        }:
            return
        self.actions.append(action)

    def validate(self, target: Path) -> None:
        root = target.resolve()
        for action in self.actions:
            if action.path.is_absolute() and not action.path.resolve().is_relative_to(root):
                raise ValueError(f"write plan escapes target: {action.path}")
            if any(part == ".." for part in action.path.parts):
                raise ValueError(f"write plan contains traversal: {action.path}")


class InstallerLock:
    """Exclusive local lock preventing concurrent installer writers."""

    def __init__(self, path: Path) -> None:
        self.path = path
        self._held = False

    def acquire(self) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        try:
            descriptor = os.open(self.path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
            os.write(descriptor, f"pid={os.getpid()}\n".encode())
            os.close(descriptor)
            self._held = True
        except FileExistsError as exc:
            raise RuntimeError(f"installer transaction is already locked: {self.path}") from exc

    def release(self) -> None:
        if self._held:
            try:
                self.path.unlink()
            except FileNotFoundError:
                pass
            self._held = False
