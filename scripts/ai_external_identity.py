#!/usr/bin/env python3
"""Validate approval identity evidence without claiming external authentication."""

from __future__ import annotations

import argparse
import json
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import Any

from ai_trust_schema import ValidationError, validate_payload

DIRECT_USER_LEVEL = "direct_user_authorized"
HIGH_RISK_LEVELS = {"provider_verified", "enterprise_verified", DIRECT_USER_LEVEL}
COMMIT_SHA = re.compile(r"^[0-9a-fA-F]{7,128}$")
LOW_IDENTITY_STATES = {
    "self_declared": "self_declared",
    "repository_recorded": "repository_recorded_only",
}


def identity_state(record: Any) -> str:
    """Return the honest display state for an approval-like record."""
    if not isinstance(record, dict):
        return "missing"
    level = record.get("identityLevel")
    if level in LOW_IDENTITY_STATES:
        return LOW_IDENTITY_STATES[level]
    if level == DIRECT_USER_LEVEL:
        return DIRECT_USER_LEVEL
    if level in HIGH_RISK_LEVELS:
        return str(level)
    if isinstance(record.get("approvedBy"), str) and record["approvedBy"].strip():
        return "repository_recorded_only"
    return "unknown"


def _present(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def approval_issues(record: Any) -> list[str]:
    """Validate strict structure plus level-specific evidence requirements."""
    try:
        validate_payload("approval", record)
    except (ValidationError, TypeError, ValueError) as exc:
        return [str(exc)]

    if not isinstance(record, dict):
        return ["approval evidence must be an object"]
    level = record["identityLevel"]
    provider = record["provider"]
    evidence = record["evidence"]
    issues: list[str] = []
    if level in LOW_IDENTITY_STATES:
        if provider is not None or evidence:
            issues.append(
                f"{level} evidence must not imply provider verification; use provider=null and evidence={{}}"
            )
    elif level == DIRECT_USER_LEVEL:
        if provider is not None:
            issues.append("direct_user_authorized evidence requires provider=null")
        for field in (
            "directUserInstructionRef",
            "directUserInstructionDigest",
            "authorizedAt",
        ):
            if not _present(evidence.get(field)):
                issues.append(f"direct_user_authorized evidence requires {field}")
        digest = evidence.get("directUserInstructionDigest")
        if _present(digest) and not re.fullmatch(r"sha256:[0-9a-fA-F]{64}", digest):
            issues.append("direct_user_authorized evidence requires a sha256 instruction digest")
        authorized_at = evidence.get("authorizedAt")
        if _present(authorized_at):
            try:
                parsed = authorized_at.replace("Z", "+00:00")
                datetime.fromisoformat(parsed)
            except ValueError:
                issues.append("direct_user_authorized evidence authorizedAt must be ISO-8601")
        forbidden = {
            "repository",
            "pullRequest",
            "reviewId",
            "environmentApprovalId",
            "rulesetId",
            "commitSha",
            "enterpriseSystem",
            "externalReference",
        }
        claimed = sorted(field for field in forbidden if field in evidence)
        if claimed:
            issues.append(
                "direct_user_authorized evidence must not contain provider or enterprise fields: "
                + ", ".join(claimed)
            )
    elif not _present(provider):
        issues.append(f"{level} requires a non-empty provider")

    if level == "provider_verified":
        for field in ("repository", "commitSha"):
            if not _present(evidence.get(field)):
                issues.append(f"provider_verified evidence requires {field}")
        commit_sha = evidence.get("commitSha")
        if _present(commit_sha) and not COMMIT_SHA.fullmatch(commit_sha):
            issues.append("provider_verified evidence commitSha must be a hexadecimal object ID")
        pull_request = evidence.get("pullRequest")
        if not isinstance(pull_request, int) or isinstance(pull_request, bool) or pull_request < 1:
            issues.append("provider_verified evidence requires positive pullRequest")
        provider_ids = ("reviewId", "environmentApprovalId", "rulesetId")
        if not any(
            (
                isinstance(evidence.get(field), int)
                and not isinstance(evidence.get(field), bool)
                and evidence[field] > 0
            )
            or _present(evidence.get(field))
            for field in provider_ids
        ):
            issues.append(
                "provider_verified evidence requires reviewId, environmentApprovalId, or rulesetId"
            )
    elif level == "enterprise_verified":
        for field in ("enterpriseSystem", "externalReference"):
            if not _present(evidence.get(field)):
                issues.append(f"enterprise_verified evidence requires {field}")
    return issues


def high_risk_approval_issues(record: Any, *, required_scope: list[str] | None = None) -> list[str]:
    """Reject high-risk approval unless its declared identity evidence is complete."""
    state = identity_state(record)
    if state not in HIGH_RISK_LEVELS:
        return [
            f"identity evidence is {state}; high-risk approval requires provider_verified or enterprise_verified, or a complete direct_user_authorized record"
        ]
    issues = approval_issues(record)
    if isinstance(record, dict) and record.get("approvalType") != "destructive_change":
        issues.append("high-risk destructive approval requires approvalType destructive_change")
    if required_scope is not None and isinstance(record, dict):
        declared_scope = record.get("scope")
        if not isinstance(declared_scope, list) or set(declared_scope) != set(required_scope):
            issues.append("high-risk approval scope must exactly match destructive allowPatterns")
    return issues


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("record", type=Path)
    parser.add_argument("--high-risk", action="store_true")
    args = parser.parse_args(argv)
    try:
        record = json.loads(args.record.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"external identity evidence failed: {exc}", file=sys.stderr)
        return 1
    issues = high_risk_approval_issues(record) if args.high_risk else approval_issues(record)
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        return 1
    print(f"external identity evidence valid: {identity_state(record)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
