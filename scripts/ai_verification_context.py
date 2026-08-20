"""Immutable, single-read inputs shared by verification stages."""

from __future__ import annotations

import json
import subprocess  # nosec B404 - the executable and argument list are fixed below
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path
from types import MappingProxyType
from typing import Any

from ai_common import parse_yaml
from ai_impact_classifier import classify_path


def _default_json_reader(path: str | Path) -> dict[str, Any]:
    with Path(path).open(encoding="utf-8") as handle:
        value = json.load(handle)
    return value if isinstance(value, dict) else {}


def _default_diff_reader(root: Path) -> list[str]:
    result = subprocess.run(
        ["git", "-C", str(root), "diff", "--name-only", "HEAD"],  # nosec B603 B607
        check=True,
        capture_output=True,
        text=True,
    )
    return [line for line in result.stdout.splitlines() if line]


@dataclass(frozen=True)
class VerificationContext:
    root: Path
    changed_paths: tuple[str, ...]
    domains: Mapping[str, tuple[str, ...]]
    contract: Mapping[str, Any]
    summary: Mapping[str, Any]
    project_profile: Mapping[str, Any]
    impact_policy: Mapping[str, Any]
    complexity_policy: Mapping[str, Any]


def _immutable(value: Any) -> Any:
    if isinstance(value, dict):
        return MappingProxyType({key: _immutable(item) for key, item in value.items()})
    if isinstance(value, list):
        return tuple(_immutable(item) for item in value)
    return value


def build_context(
    root: str | Path,
    contract_path: str | Path,
    summary_path: str | Path,
    *,
    read_json: Callable[[str | Path], dict[str, Any]] = _default_json_reader,
    read_diff: Callable[[], list[str]] | None = None,
) -> VerificationContext:
    """Read all verification inputs once and return a frozen context."""
    root_path = Path(root)
    contract = read_json(contract_path)
    summary = read_json(summary_path)
    profile_path = root_path / ".ai/project_profile.yaml"
    impact_path = root_path / ".ai/policies/verification_impact.yaml"
    complexity_path = root_path / ".ai/policies/complexity_trend.yaml"
    profile = parse_yaml(profile_path) if profile_path.exists() else {}
    impact = parse_yaml(impact_path) if impact_path.exists() else {}
    complexity = parse_yaml(complexity_path) if complexity_path.exists() else {}
    paths = tuple(sorted(set((read_diff or (lambda: _default_diff_reader(root_path)))())))
    domains: dict[str, list[str]] = {}
    for path in paths:
        domains.setdefault(classify_path(path), []).append(path)
    return VerificationContext(
        root=root_path,
        changed_paths=paths,
        domains=MappingProxyType({key: tuple(value) for key, value in domains.items()}),
        contract=_immutable(contract),
        summary=_immutable(summary),
        project_profile=_immutable(profile),
        impact_policy=_immutable(impact),
        complexity_policy=_immutable(complexity),
    )
