"""Typed, read-only detection used by the future Installation Wizard."""

from __future__ import annotations

import json
import shutil
from collections.abc import Iterable
from dataclasses import asdict, dataclass
from pathlib import Path

from ai_installer_repository import RepositoryFacts, read_repository_facts

REQUIRED_TOOLS = ("git", "python", "make", "curl", "sh")


@dataclass(frozen=True)
class InstallationPlan:
    """Human-reviewable plan; it contains no write operation."""

    facts: dict[str, object]
    recommendation: str
    impact: str
    examples: tuple[str, ...]
    write_boundary: str
    expected_result: str
    stop_condition: str
    checklist: tuple[str, ...]

    def to_dict(self) -> dict[str, object]:
        data = asdict(self)
        data["writeBoundary"] = data.pop("write_boundary")
        data["expectedResult"] = data.pop("expected_result")
        data["stopCondition"] = data.pop("stop_condition")
        data["examples"] = list(self.examples)
        data["checklist"] = list(self.checklist)
        return data


@dataclass(frozen=True)
class InstallationDetection:
    """Complete detection result for New Adoption or Upgrade."""

    facts: RepositoryFacts
    mode: str
    available_tools: tuple[str, ...]
    missing_tools: tuple[str, ...]
    stacks: tuple[str, ...]
    readiness: str
    blocking_reasons: tuple[str, ...]
    plan: InstallationPlan


def _impact(facts: RepositoryFacts, missing: tuple[str, ...], blockers: tuple[str, ...]) -> str:
    if blockers or facts.symlink_risks:
        return "high"
    if missing or facts.tracked_hygiene:
        return "medium"
    return "low"


def detect_installation(
    *, facts: RepositoryFacts, mode: str, available_tools: Iterable[str], stacks: Iterable[str]
) -> InstallationDetection:
    """Build a deterministic read-only detection result from supplied observations."""
    if mode not in {"new_adoption", "upgrade"}:
        raise ValueError("mode must be new_adoption or upgrade")
    tools = tuple(sorted(set(available_tools)))
    missing = tuple(tool for tool in REQUIRED_TOOLS if tool not in tools)
    stack_values = tuple(sorted(set(stacks)))
    blockers: list[str] = []
    if facts.symlink_risks:
        blockers.append("symlink_risk")
    if facts.conflicts:
        blockers.append("conflicts")
    if not facts.clean:
        blockers.append("dirty_worktree")
    if mode == "upgrade" and facts.active_work_items:
        blockers.append("active_work_items")
    impact = _impact(facts, missing, tuple(blockers))
    readiness = "blocked" if blockers else "ready_for_confirmation"
    recommendation = "upgrade" if mode == "upgrade" else "new_adoption"
    checklist = (
        "confirm target repository and remote",
        "review readiness, impact, and unknowns",
        "confirm write boundary before invoking Installer",
    )
    plan = InstallationPlan(
        facts={
            **facts.to_dict(),
            "mode": mode,
            "availableTools": list(tools),
            "missingTools": list(missing),
            "stacks": list(stack_values),
        },
        recommendation=recommendation,
        impact=impact,
        examples=("python scripts/install_ai_cockpit.py --dry-run", "make ai-install"),
        write_boundary="none_before_confirmation",
        expected_result="A human-confirmed Installer transaction produces the selected installation or upgrade.",
        stop_condition="Stop before writes when readiness is blocked, facts are unknown, or confirmation is not affirmative.",
        checklist=checklist,
    )
    return InstallationDetection(
        facts, mode, tools, missing, stack_values, readiness, tuple(blockers), plan
    )


def collect_installation_detection(
    root: str | Path, *, mode: str, stacks: Iterable[str] = ()
) -> InstallationDetection:
    """Collect local facts and tool availability without mutating ``root``."""
    facts = read_repository_facts(Path(root))
    available = {name for name in REQUIRED_TOOLS if shutil.which(name)}
    return detect_installation(facts=facts, mode=mode, available_tools=available, stacks=stacks)


def serialize_plan(plan: InstallationPlan) -> str:
    """Serialize an Installation Plan with stable key and sequence ordering."""
    return (
        json.dumps(plan.to_dict(), ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n"
    )


def missing_runtime_scripts(names: set[str], available: set[str]) -> list[str]:
    return sorted(names - available)
