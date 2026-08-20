#!/usr/bin/env python3
"""Validate proportional Calibration Profiles and bounded transitions."""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

from ai_common import PROJECT_ROOT, non_empty_string, parse_yaml

DEFAULT_POLICY = PROJECT_ROOT / ".ai" / "calibration" / "profiles.yaml"
EXPECTED_LEVELS = ("lite", "standard", "strict")


class CalibrationProfileError(ValueError):
    """Raised when the profile policy cannot be proven valid."""


@dataclass(frozen=True)
class CalibrationProfilePolicy:
    levels: tuple[str, ...]
    additions: dict[str, tuple[str, ...]]

    def required_controls(self, level: str) -> list[str]:
        if level not in self.levels:
            raise CalibrationProfileError(f"unknown calibration profile level: {level}")
        controls: list[str] = []
        for current in self.levels:
            controls.extend(self.additions[current])
            if current == level:
                return controls
        raise AssertionError("validated level ordering is incomplete")

    def deferred_controls(self, level: str) -> list[str]:
        required = set(self.required_controls(level))
        return [
            control
            for current in self.levels
            for control in self.additions[current]
            if control not in required
        ]


def _string_list(value: object, field: str, *, allow_empty: bool = False) -> list[str]:
    if not isinstance(value, list) or (not value and not allow_empty):
        raise CalibrationProfileError(f"{field} must be a list of non-empty strings")
    if any(not non_empty_string(item) for item in value):
        raise CalibrationProfileError(f"{field} must be a list of non-empty strings")
    return [str(item).strip() for item in value]


def load_policy(path: Path = DEFAULT_POLICY) -> CalibrationProfilePolicy:
    try:
        value = parse_yaml(path)
    except (OSError, ValueError) as exc:
        raise CalibrationProfileError(f"failed to load calibration profile policy: {exc}") from exc
    if not isinstance(value, dict):
        raise CalibrationProfileError("calibration profile policy root must be an object")
    if value.get("version") not in {1, "1"}:
        raise CalibrationProfileError("calibration profile policy version must be 1")
    levels = tuple(_string_list(value.get("levels"), "levels"))
    if levels != EXPECTED_LEVELS:
        raise CalibrationProfileError("calibration profile levels must be lite, standard, strict")
    controls = value.get("controls")
    if not isinstance(controls, dict) or set(controls) != set(levels):
        raise CalibrationProfileError("controls must declare exactly lite, standard, strict")
    additions: dict[str, tuple[str, ...]] = {}
    seen: set[str] = set()
    for level in levels:
        items = _string_list(controls.get(level), f"controls.{level}")
        duplicates = sorted(seen.intersection(items))
        if duplicates:
            raise CalibrationProfileError(
                "duplicate control across calibration levels: " + ", ".join(duplicates)
            )
        if len(items) != len(set(items)):
            raise CalibrationProfileError(f"duplicate control in controls.{level}")
        seen.update(items)
        additions[level] = tuple(items)
    return CalibrationProfilePolicy(levels=levels, additions=additions)


def _valid_timestamp(value: object) -> bool:
    if not non_empty_string(value):
        return False
    try:
        parsed = datetime.fromisoformat(str(value))
    except ValueError:
        return False
    return parsed.tzinfo is not None


def _list_of_strings(value: object, *, allow_empty: bool = False) -> list[str] | None:
    if not isinstance(value, list) or (not allow_empty and not value):
        return None
    if any(not non_empty_string(item) for item in value):
        return None
    return [str(item).strip() for item in value]


def validate_selection(
    value: object,
    policy: CalibrationProfilePolicy,
    *,
    previous_level: str | None = None,
    require_human: bool = True,
) -> list[str]:
    if not isinstance(value, dict):
        return ["calibrationProfile must be an object"]
    level = value.get("level")
    if level not in policy.levels:
        return [f"calibrationProfile.level must be one of {list(policy.levels)}"]

    issues: list[str] = []
    pending = value.get("selectedBy") == "pending_human"
    if require_human or not pending:
        if value.get("selectedBy") != "human":
            issues.append("calibrationProfile.selectedBy must be human")
        if not _valid_timestamp(value.get("selectedAt")):
            issues.append("calibrationProfile.selectedAt must be an ISO-8601 timestamp")
        if _list_of_strings(value.get("reasons")) is None:
            issues.append("calibrationProfile.reasons must contain at least one non-empty string")
    elif value.get("selectedAt") != "pending" or value.get("reasons") != []:
        issues.append("pending calibrationProfile must keep selectedAt pending and reasons empty")
    if value.get("requiredControls") != policy.required_controls(str(level)):
        issues.append("calibrationProfile.requiredControls do not match the selected level")
    if value.get("deferredControls") != policy.deferred_controls(str(level)):
        issues.append("calibrationProfile.deferredControls do not match the selected level")

    if previous_level is None:
        return issues
    if previous_level not in policy.levels:
        issues.append(f"previous calibration profile must be one of {list(policy.levels)}")
        return issues
    previous_index = policy.levels.index(previous_level)
    current_index = policy.levels.index(str(level))
    transition = value.get("transition")
    if isinstance(transition, dict):
        if transition.get("originalLevel") != previous_level:
            issues.append(
                "calibrationProfile.transition.originalLevel does not match previous level"
            )
        if transition.get("newLevel") != level:
            issues.append("calibrationProfile.transition.newLevel does not match selected level")
    if current_index >= previous_index:
        if transition is not None and not isinstance(transition, dict):
            issues.append("calibrationProfile.transition must be an object when declared")
        return issues
    if not isinstance(transition, dict):
        issues.append("calibrationProfile downgrade requires transition evidence")
        return issues

    for field in ("reason", "riskAcceptedBy"):
        if not non_empty_string(transition.get(field)):
            issues.append(f"calibrationProfile.transition.{field} must be a non-empty string")
    effective_scope = _list_of_strings(transition.get("effectiveScope"))
    if effective_scope is None:
        issues.append(
            "calibrationProfile.transition.effectiveScope must contain at least one path scope"
        )
    expected_closed = [
        control
        for control in policy.required_controls(previous_level)
        if control not in policy.required_controls(str(level))
    ]
    if transition.get("closedControls") != expected_closed:
        issues.append("calibrationProfile.transition.closedControls do not match the downgrade")
    return issues


def load_selection(path: Path) -> object:
    try:
        value = parse_yaml(path)
    except (OSError, ValueError) as exc:
        raise CalibrationProfileError(f"failed to load Project Profile: {exc}") from exc
    if not isinstance(value, dict):
        raise CalibrationProfileError("Project Profile root must be an object")
    return value.get("calibrationProfile")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", default=".ai/project_profile.yaml")
    parser.add_argument("--policy", default=".ai/calibration/profiles.yaml")
    parser.add_argument("--previous-level", choices=EXPECTED_LEVELS)
    parser.add_argument("--output")
    args = parser.parse_args(argv)
    try:
        policy = load_policy(Path(args.policy))
        selection = load_selection(Path(args.profile))
        issues = validate_selection(selection, policy, previous_level=args.previous_level)
    except CalibrationProfileError as exc:
        issues = [str(exc)]
    receipt = {
        "status": "passed" if not issues else "blocked",
        "profile": args.profile,
        "policy": args.policy,
        "previousLevel": args.previous_level,
        "issues": issues,
    }
    if args.output:
        output = Path(args.output)
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        return 1
    print("calibration Profile validation passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
