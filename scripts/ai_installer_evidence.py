"""Evidence boundary for deterministic installer action summaries."""

from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class InstallationPreview:
    """Read-only action counts shown before installation confirmation."""

    adds: int
    modifies: int
    skips: int
    source_code_changes: bool
    branch: str


def summarize_installation_actions(
    actions: Sequence[object], *, target: Path, branch: str
) -> InstallationPreview:
    """Summarize a dry-run action list for operator review."""
    root = target.resolve()
    adds = 0
    modifies = 0
    skips = 0
    source_code_changes = False
    managed_roots = {".ai", ".cursor", "scripts", "examples"}
    managed_files = {
        ".gitignore",
        "AGENTS.md",
        "CLAUDE.md",
        "GEMINI.md",
        "Makefile",
        "Makefile.ai",
        "Makefile.ai.stack",
    }
    for action in actions:
        kind = str(getattr(action, "kind", ""))
        if kind == "skip":
            skips += 1
            continue
        if kind not in {"write", "overwrite", "append", "replace"}:
            continue
        if kind == "write":
            adds += 1
        else:
            modifies += 1
        path = Path(getattr(action, "path", root))
        try:
            relative = path.resolve().relative_to(root)
        except ValueError:
            source_code_changes = True
            continue
        if not relative.parts:
            source_code_changes = True
            continue
        if relative.parts[0] not in managed_roots and relative.as_posix() not in managed_files:
            source_code_changes = True
    return InstallationPreview(adds, modifies, skips, source_code_changes, branch)


def action_counts(actions: Sequence[object]) -> tuple[int, int]:
    writes = sum(
        getattr(item, "kind", "") in {"write", "overwrite", "append", "replace"} for item in actions
    )
    skips = sum(getattr(item, "kind", "") == "skip" for item in actions)
    return writes, skips
