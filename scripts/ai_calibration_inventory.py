#!/usr/bin/env python3
"""Build and validate the shared Calibration Inventory evidence matrix."""

from __future__ import annotations

import argparse
import json
import sys
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from ai_common import InvalidDataShapeError, parse_yaml
from ai_project_profile import load_profile

STATUS_VALUES = (
    "complete",
    "warning",
    "incomplete",
    "unknown",
    "not_applicable",
)
CONFIRMATION_VALUES = {"none", "static", "command", "human", "external"}
EVIDENCE_KIND_VALUES = ("fixture", "hosted", "adopter_execution", "not_verified")
INVENTORY_KEYS = (
    "profile",
    "guards",
    "quality",
    "coverage",
    "complexity",
    "review",
    "security",
    "ci",
    "installed_lifecycle",
    "documentation",
)


def _now() -> datetime:
    return datetime.now(UTC)


def _parse_timestamp(value: Any) -> datetime | None:
    if not isinstance(value, str) or not value.strip():
        return None
    try:
        return datetime.fromisoformat(value)
    except ValueError:
        return None


def _profile_role(root: Path) -> str | None:
    path = root / ".ai" / "project_profile.yaml"
    if not path.is_file():
        return None
    try:
        data = parse_yaml(path)
    except (OSError, ValueError):
        return None
    return data.get("repositoryRole") if isinstance(data, dict) else None


def _entry(
    key: str,
    status: str,
    *,
    source: str,
    confirmation: str = "static",
    evidence: list[str] | None = None,
    stale_at: str | None = None,
    owner: str = "repository",
    blocking_reason: str | None = None,
    evidence_kind: str = "not_verified",
) -> dict[str, Any]:
    return {
        "key": key,
        "status": status,
        "source": source,
        "confirmation": confirmation,
        "evidenceKind": evidence_kind,
        "evidence": list(evidence or []),
        "staleAt": stale_at,
        "owner": owner,
        "blockingReason": blocking_reason,
    }


def _static_entry(
    key: str,
    path: Path,
    *,
    present_status: str = "warning",
    absent_status: str = "incomplete",
    present_reason: str | None = None,
    absent_reason: str | None = None,
) -> dict[str, Any]:
    relative = path.as_posix()
    if path.is_file() or path.is_dir():
        return _entry(
            key,
            present_status,
            source=relative,
            evidence=[f"path exists: {relative}"],
            blocking_reason=present_reason,
        )
    return _entry(
        key,
        absent_status,
        source=relative,
        evidence=[],
        blocking_reason=absent_reason or f"missing evidence source: {relative}",
    )


def _quality_entry(root: Path) -> dict[str, Any]:
    path = root / "Makefile.ai.stack"
    if not path.is_file():
        return _entry(
            "quality",
            "unknown",
            source="Makefile.ai.stack",
            confirmation="none",
            blocking_reason="quality commands have not been supplied or executed",
        )
    text = path.read_text(encoding="utf-8", errors="replace")
    names = ("PROJECT_FORMAT_CHECK", "PROJECT_TEST", "PROJECT_LINT")
    configured = all(name in text for name in names)
    return _entry(
        "quality",
        "warning" if configured else "incomplete",
        source="Makefile.ai.stack",
        evidence=["quality command configuration is statically present"] if configured else [],
        blocking_reason=(
            "static configuration is present; command evidence is not supplied"
            if configured
            else "one or more project quality commands are missing"
        ),
    )


def _profile_entry(root: Path) -> dict[str, Any]:
    path = root / ".ai" / "project_profile.yaml"
    profile, issues = load_profile(path, require_approval=True)
    role = profile.get("repositoryRole") if isinstance(profile, dict) else None
    if not issues:
        confirmation = "human" if profile.get("approval", {}).get("reviewed") is True else "static"
        return _entry(
            "profile",
            "complete",
            source=".ai/project_profile.yaml",
            confirmation=confirmation,
            evidence=["confirmed Project Profile schema and approval fields"],
            owner=str(profile.get("approval", {}).get("reviewedBy") or "repository"),
        )
    return _entry(
        "profile",
        "incomplete",
        source=".ai/project_profile.yaml",
        confirmation="none",
        evidence=[f"profile role={role}"] if role else [],
        blocking_reason="; ".join(issues[:3]),
    )


def _guards_entry(root: Path) -> dict[str, Any]:
    profile_path = root / ".ai" / "project_profile.yaml"
    coverage = root / ".ai" / "guards" / "coverage_policy.yaml"
    complexity = root / ".ai" / "guards" / "governance_complexity_policy.yaml"
    if not profile_path.is_file():
        return _entry(
            "guards",
            "incomplete",
            source=".ai/guards/",
            blocking_reason="confirmed Profile is required before Guard calibration can be evaluated",
        )
    if coverage.is_file() and complexity.is_file():
        return _entry(
            "guards",
            "warning",
            source=".ai/guards/",
            evidence=["coverage and complexity guard policy files are present"],
            blocking_reason="static Guard presence does not prove every Guard command passed",
        )
    return _entry(
        "guards",
        "incomplete",
        source=".ai/guards/",
        blocking_reason="one or more required Guard policy files are missing",
    )


def _coverage_entry(root: Path) -> dict[str, Any]:
    path = root / ".ai" / "guards" / "coverage_policy.yaml"
    if not path.is_file():
        return _entry(
            "coverage",
            "incomplete",
            source=".ai/guards/coverage_policy.yaml",
            confirmation="none",
            blocking_reason="coverage policy is missing",
        )
    text = path.read_text(encoding="utf-8", errors="replace")
    reviewed = "adoptionReviewed: true" in text
    return _entry(
        "coverage",
        "warning" if reviewed else "incomplete",
        source=".ai/guards/coverage_policy.yaml",
        evidence=["adoptionReviewed=true is statically configured"] if reviewed else [],
        blocking_reason=(
            "static coverage configuration does not prove a fresh coverage run"
            if reviewed
            else "adoptionReviewed is not true"
        ),
    )


def _complexity_entry(root: Path) -> dict[str, Any]:
    path = root / ".ai" / "guards" / "governance_complexity_policy.yaml"
    if not path.is_file():
        return _entry(
            "complexity",
            "incomplete",
            source=".ai/guards/governance_complexity_policy.yaml",
            confirmation="none",
            blocking_reason="complexity policy is missing",
        )
    try:
        data = parse_yaml(path)
    except (OSError, ValueError) as exc:
        return _entry(
            "complexity",
            "incomplete",
            source=path.as_posix(),
            confirmation="none",
            blocking_reason=f"complexity policy cannot be parsed: {exc}",
        )
    proposal = data.get("proposal", {}) if isinstance(data, dict) else {}
    status = proposal.get("status") if isinstance(proposal, dict) else None
    if status == "confirmed":
        return _entry(
            "complexity",
            "complete",
            source=path.as_posix(),
            confirmation="human",
            evidence=["complexity policy proposal is confirmed"],
        )
    return _entry(
        "complexity",
        "warning" if status else "incomplete",
        source=path.as_posix(),
        evidence=[f"proposal status={status}"] if status else [],
        blocking_reason="complexity policy requires explicit confirmation"
        if status != "confirmed"
        else None,
    )


def _ci_entry(root: Path) -> dict[str, Any]:
    workflows = root / ".github" / "workflows"
    gitlab = root / ".gitlab-ci.yml"
    if workflows.is_dir() or gitlab.is_file():
        source = ".github/workflows/" if workflows.is_dir() else ".gitlab-ci.yml"
        return _entry(
            "ci",
            "warning",
            source=source,
            evidence=["CI configuration is statically present"],
            blocking_reason="CI configuration is present but no bound workflow run evidence was supplied",
        )
    return _entry(
        "ci",
        "incomplete",
        source=".github/workflows/ or .gitlab-ci.yml",
        blocking_reason="CI configuration is missing",
    )


def _inventory_without_overrides(root: Path) -> dict[str, dict[str, Any]]:
    role = _profile_role(root)
    entries = {
        "profile": _profile_entry(root),
        "guards": _guards_entry(root),
        "quality": _quality_entry(root),
        "coverage": _coverage_entry(root),
        "complexity": _complexity_entry(root),
        "review": _static_entry(
            "review",
            root / ".ai" / "guards" / "ai_review_policy.yaml",
            present_status="warning",
            present_reason="local review policy presence does not prove external review approval",
        ),
        "security": _static_entry(
            "security",
            root / "SECURITY.md",
            present_status="warning",
            present_reason="security policy file is present; private channel effectiveness is not proven by static inspection",
        ),
        "ci": _ci_entry(root),
        "installed_lifecycle": _entry(
            "installed_lifecycle",
            "not_applicable",
            source=".ai/cockpit/version.json",
            evidence=["template repository has no adopter installation target"]
            if role == "template"
            else [],
            blocking_reason="template repository; adopter-installed lifecycle evidence is not applicable"
            if role == "template"
            else None,
        )
        if role == "template"
        else _static_entry(
            "installed_lifecycle",
            root / ".ai" / "cockpit" / "version.json",
            present_status="warning",
            present_reason="installed version is present; lifecycle command execution evidence is not supplied",
        ),
        "documentation": _static_entry(
            "documentation",
            root / "docs" / "reference" / "capability-truth-matrix.json",
            present_status="complete",
            present_reason=None,
        ),
    }
    return entries


def _apply_override(item: dict[str, Any], override: Any, *, now: datetime) -> dict[str, Any]:
    if not isinstance(override, dict):
        raise InvalidDataShapeError(f"command evidence for {item['key']} must be an object")
    status = override.get("status")
    confirmation = override.get("confirmation")
    evidence_kind = override.get("evidenceKind", "not_verified")
    source = override.get("source")
    evidence = override.get("evidence", [])
    if status not in STATUS_VALUES:
        raise ValueError(f"command evidence for {item['key']} has unsupported status: {status}")
    if confirmation not in CONFIRMATION_VALUES:
        raise ValueError(
            f"command evidence for {item['key']} has unsupported confirmation: {confirmation}"
        )
    if evidence_kind not in EVIDENCE_KIND_VALUES:
        raise ValueError(
            f"command evidence for {item['key']} has unsupported evidenceKind: {evidence_kind}"
        )
    if not isinstance(source, str) or not source.strip():
        raise ValueError(f"command evidence for {item['key']} requires source")
    if not isinstance(evidence, list) or any(
        not isinstance(value, str) or not value for value in evidence
    ):
        raise ValueError(f"command evidence for {item['key']} requires string evidence")
    stale_at = override.get("staleAt")
    if stale_at is not None and _parse_timestamp(stale_at) is None:
        raise ValueError(f"command evidence for {item['key']} has invalid staleAt")
    updated = _entry(
        item["key"],
        status,
        source=source,
        confirmation=confirmation,
        evidence_kind=evidence_kind,
        evidence=evidence,
        stale_at=stale_at,
        owner=str(override.get("owner") or item["owner"]),
        blocking_reason=override.get("blockingReason"),
    )
    stale_time = _parse_timestamp(stale_at)
    if stale_time is not None and stale_time <= now and status == "complete":
        updated["status"] = "incomplete"
        updated["blockingReason"] = "evidence is stale"
    return updated


def build_inventory(
    root: Path,
    *,
    command_evidence: dict[str, Any] | None = None,
    now: datetime | None = None,
) -> dict[str, Any]:
    """Build a conservative, JSON-serializable inventory for one repository."""
    root = root.resolve()
    now = now or _now()
    items = _inventory_without_overrides(root)
    for key, override in (command_evidence or {}).items():
        if key not in items:
            raise ValueError(f"unknown calibration inventory key: {key}")
        items[key] = _apply_override(items[key], override, now=now)
    summary = {
        status: sum(item["status"] == status for item in items.values()) for status in STATUS_VALUES
    }
    return {
        "schemaVersion": 1,
        "generatedAt": now.isoformat(),
        "repositoryRole": _profile_role(root) or "unknown",
        "items": {key: items[key] for key in INVENTORY_KEYS},
        "summary": summary,
        "blockingItems": [
            key for key in INVENTORY_KEYS if items[key]["status"] in {"incomplete", "unknown"}
        ],
    }


def validate_inventory(data: Any) -> list[str]:
    issues: list[str] = []
    if not isinstance(data, dict) or data.get("schemaVersion") != 1:
        return ["inventory schemaVersion must be 1"]
    items = data.get("items")
    if not isinstance(items, dict):
        return ["inventory items must be an object"]
    if tuple(items) != INVENTORY_KEYS:
        issues.append("inventory items must contain the canonical ordered keys")
    for key in INVENTORY_KEYS:
        item = items.get(key)
        if not isinstance(item, dict):
            issues.append(f"inventory item {key} must be an object")
            continue
        if item.get("key") != key:
            issues.append(f"inventory item {key}.key must match")
        if item.get("status") not in STATUS_VALUES:
            issues.append(f"inventory item {key}.status is unsupported")
        if not isinstance(item.get("source"), str) or not item["source"]:
            issues.append(f"inventory item {key}.source is required")
        if item.get("confirmation") not in CONFIRMATION_VALUES:
            issues.append(f"inventory item {key}.confirmation is unsupported")
        if item.get("evidenceKind") not in EVIDENCE_KIND_VALUES:
            issues.append(f"inventory item {key}.evidenceKind is unsupported")
        if not isinstance(item.get("evidence"), list):
            issues.append(f"inventory item {key}.evidence must be a list")
        if not isinstance(item.get("owner"), str) or not item["owner"]:
            issues.append(f"inventory item {key}.owner is required")
        if "staleAt" not in item or (
            item["staleAt"] is not None and _parse_timestamp(item["staleAt"]) is None
        ):
            issues.append(f"inventory item {key}.staleAt must be null or an ISO timestamp")
        if "blockingReason" not in item:
            issues.append(f"inventory item {key}.blockingReason is required")
    return issues


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".")
    parser.add_argument(
        "--evidence", help="JSON object containing command/human evidence overrides."
    )
    parser.add_argument("--output", help="Write the inventory JSON to this path.")
    parser.add_argument("--check", action="store_true", help="Validate the generated inventory.")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        overrides = (
            json.loads(Path(args.evidence).read_text(encoding="utf-8")) if args.evidence else None
        )
        inventory = build_inventory(Path(args.root), command_evidence=overrides)
        issues = validate_inventory(inventory)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"calibration inventory failed: {exc}", file=sys.stderr)
        return 1
    if args.check and issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        return 1
    output = json.dumps(inventory, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        destination = Path(args.output)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(output, encoding="utf-8")
        print(f"calibration inventory written: {destination}")
    else:
        print(output, end="")
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
