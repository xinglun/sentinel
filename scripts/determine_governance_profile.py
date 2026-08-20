#!/usr/bin/env python3
"""Select an evidence-backed AI Cockpit governance profile for a Git diff."""

from __future__ import annotations

import argparse
import fnmatch
import json
import subprocess  # nosec B404 - used only for fixed list-form Git inspection
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

from ai_common import parse_yaml
from ai_verification_policy import (
    classify_immutable_workflow_pin_change,
    strict_quality_routing,
)

PROFILE_ORDER = ("light", "standard", "strict")
PROFILE_DEPTHS = {"light": "focused", "standard": "project", "strict": "full"}
MANDATORY_CONTROLS = ("scope", "trust", "lifecycle", "evidence_integrity")
EXPECTED_TARGETS = {
    "light": "quality-fast",
    "standard": "quality-standard",
    "strict": "quality-full",
}
DEFAULT_POLICY = Path(".ai/quality/governance-routing.yaml")


def _non_empty(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _string_list(value: Any, field: str, *, non_empty: bool = True) -> list[str]:
    if (
        not isinstance(value, list)
        or (non_empty and not value)
        or any(not _non_empty(item) for item in value)
    ):
        requirement = "a non-empty list" if non_empty else "a list"
        raise ValueError(f"{field} must be {requirement} of non-empty strings")
    return [str(item) for item in value]


def load_policy(path: Path) -> dict[str, Any]:
    """Load and strictly validate the versioned routing policy."""
    try:
        data = parse_yaml(path)
    except (OSError, ValueError) as exc:
        raise ValueError(f"unable to load governance routing policy: {exc}") from exc
    if not isinstance(data, dict):
        raise TypeError("governance routing policy must be an object")
    if data.get("schemaVersion") != "1":
        raise ValueError("governance routing policy schemaVersion must be 1")
    order = tuple(_string_list(data.get("profileOrder"), "profileOrder"))
    if order != PROFILE_ORDER:
        raise ValueError(f"profileOrder must be {list(PROFILE_ORDER)}")
    unknown = data.get("unknownProfile")
    if unknown not in PROFILE_ORDER[1:]:
        raise ValueError("unknownProfile must be standard or strict")
    evidence_patterns = _string_list(data.get("evidenceOnlyPatterns"), "evidenceOnlyPatterns")
    for pattern in evidence_patterns:
        _validate_relative_path(pattern, label=f"policy pattern {pattern!r}")
    for pattern in _string_list(data.get("releaseOwnedPatterns"), "releaseOwnedPatterns"):
        _validate_relative_path(pattern, label=f"release pattern {pattern!r}")
    profiles = data.get("profiles")
    if not isinstance(profiles, dict) or set(profiles) != set(PROFILE_ORDER):
        raise ValueError(f"profiles must define exactly {list(PROFILE_ORDER)}")
    for name in PROFILE_ORDER:
        config = profiles.get(name)
        if not isinstance(config, dict):
            raise TypeError(f"profiles.{name} must be an object")
        patterns = _string_list(config.get("patterns"), f"profiles.{name}.patterns")
        for pattern in patterns:
            _validate_relative_path(pattern, label=f"policy pattern {pattern!r}")
        _string_list(config.get("requiredGroups"), f"profiles.{name}.requiredGroups")
        if config.get("dispatchTarget") != EXPECTED_TARGETS[name]:
            raise ValueError(f"profiles.{name}.dispatchTarget must be {EXPECTED_TARGETS[name]}")
        if config.get("verificationDepth") != PROFILE_DEPTHS[name]:
            raise ValueError(f"profiles.{name}.verificationDepth must be {PROFILE_DEPTHS[name]}")
        required_evidence = _string_list(
            config.get("requiredEvidence"), f"profiles.{name}.requiredEvidence"
        )
        optional_checks = _string_list(
            config.get("optionalChecks"), f"profiles.{name}.optionalChecks"
        )
        mandatory_controls = tuple(
            _string_list(config.get("mandatoryControls"), f"profiles.{name}.mandatoryControls")
        )
        if mandatory_controls != MANDATORY_CONTROLS:
            raise ValueError(
                f"profiles.{name}.mandatoryControls must be {list(MANDATORY_CONTROLS)}"
            )
        if set(optional_checks) & set(mandatory_controls):
            raise ValueError(f"profiles.{name}.optionalChecks cannot disable mandatory controls")
        if name != PROFILE_ORDER[0]:
            previous = data["profiles"][PROFILE_ORDER[PROFILE_ORDER.index(name) - 1]]
            previous_evidence = set(previous["requiredEvidence"])
            if not previous_evidence.issubset(required_evidence):
                raise ValueError(
                    f"profiles.{name}.requiredEvidence must include the lower profile requirements"
                )
    return data


def _validate_relative_path(value: str, *, label: str = "changed path") -> str:
    normalized = value.replace("\\", "/")
    pure = PurePosixPath(normalized)
    if not normalized or pure.is_absolute() or ".." in pure.parts or normalized.startswith("./"):
        raise ValueError(f"unsafe {label}: {value}")
    return pure.as_posix()


def normalize_paths(paths: list[str], repository: Path | None = None) -> list[str]:
    """Normalize changed paths and reject traversal or symlink repository escape."""
    root = repository.resolve() if repository is not None else None
    normalized: set[str] = set()
    for raw_path in paths:
        path = _validate_relative_path(raw_path)
        if root is not None:
            candidate = (root / path).resolve(strict=False)
            try:
                candidate.relative_to(root)
            except ValueError as exc:
                raise ValueError(f"changed path escapes repository: {raw_path}") from exc
        normalized.add(path)
    return sorted(normalized)


def changed_paths(base: str, head: str, repository: Path) -> list[str]:
    commands = (
        ["git", "diff", "--name-only", f"{base}...{head}", "--"],
        ["git", "diff", "--name-only", head, "--"],
        ["git", "ls-files", "--others", "--exclude-standard"],
    )
    paths: list[str] = []
    for command in commands:
        result = subprocess.run(  # nosec B603 - executable and arguments are fixed below
            command,
            cwd=repository,
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode != 0:
            detail = result.stderr.strip() or result.stdout.strip()
            raise RuntimeError(
                f"unable to determine changed paths: {detail or 'git inspection failed'}"
            )
        paths.extend(result.stdout.splitlines())
    return normalize_paths(paths, repository)


def _rank(profile: str) -> int:
    try:
        return PROFILE_ORDER.index(profile)
    except ValueError as exc:
        raise ValueError(f"unsupported governance profile: {profile}") from exc


def _classify(path: str, policy: dict[str, Any]) -> tuple[str, list[str]]:
    if any(fnmatch.fnmatchcase(path, pattern) for pattern in policy["releaseOwnedPatterns"]):
        return "strict", [f"release-owned resource requires strict: {path}"]
    evidence_matches = sorted(
        pattern for pattern in policy["evidenceOnlyPatterns"] if fnmatch.fnmatchcase(path, pattern)
    )
    if evidence_matches:
        return "evidence_only", [
            f"generated Work Item evidence {pattern}: {path}" for pattern in evidence_matches
        ]
    matches: list[tuple[str, str]] = []
    for profile in PROFILE_ORDER:
        for pattern in policy["profiles"][profile]["patterns"]:
            if fnmatch.fnmatchcase(path, pattern):
                matches.append((profile, pattern))
    if not matches:
        profile = str(policy["unknownProfile"])
        return profile, [f"unknown path defaults to {profile}: {path}"]
    selected = max((profile for profile, _ in matches), key=_rank)
    patterns = sorted(pattern for profile, pattern in matches if profile == selected)
    return selected, [f"{selected} pattern {pattern}: {path}" for pattern in patterns]


def _read_git_file(repository: Path, revision: str, path: str) -> str:
    result = subprocess.run(  # nosec B603 B607 - fixed list-form Git evidence lookup
        ["git", "show", f"{revision}:{path}"],
        cwd=repository,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise ValueError(result.stderr.strip() or f"unable to read {revision}:{path}")
    return result.stdout


def _immutable_pin_facts(
    paths: list[str], *, repository: Path | None, base: str, head: str
) -> dict[str, Any] | None:
    """Build immutable-pin facts from base and current repository content."""
    if repository is None or len(paths) != 1 or not base:
        return None
    path = paths[0]
    current_path = repository / path
    try:
        before = _read_git_file(repository, base, path)
        if current_path.is_file():
            after = current_path.read_text(encoding="utf-8")
        else:
            after = _read_git_file(repository, head, path)
    except (OSError, ValueError) as exc:
        return {
            "path": path,
            "kind": "immutable_workflow_pin",
            "eligible": False,
            "reason": f"base/current evidence unavailable: {exc}",
            "replacementCount": 0,
        }
    return classify_immutable_workflow_pin_change(path, before, after)


def _release_escalation(
    paths: list[str], policy: dict[str, Any], contract: dict[str, Any]
) -> list[str]:
    reasons = [
        f"release-owned resource: {path}"
        for path in paths
        if any(fnmatch.fnmatchcase(path, pattern) for pattern in policy["releaseOwnedPatterns"])
    ]
    classes = contract.get("operationClasses", [])
    if isinstance(classes, list) and "release" in classes:
        reasons.append("Contract operationClasses includes release")
    operation = contract.get("requestedOperation", {})
    operation_text = str(operation).lower() if isinstance(operation, dict) else ""
    release_operation_terms = (
        "release",
        "create_tag",
        "publish",
        "distribution",
        "sbom",
        "provenance",
        "checksum",
        "signature",
        "signing",
    )
    if any(term in operation_text for term in release_operation_terms):
        reasons.append("requestedOperation declares release context")
    claims = contract.get("capabilityClaims", [])
    intent = contract.get("declaredIntent", {})
    if isinstance(intent, dict):
        claims = list(claims) if isinstance(claims, list) else []
        requested_capabilities = intent.get("requestedCapabilities", [])
        if isinstance(requested_capabilities, list):
            claims.extend(requested_capabilities)
    if isinstance(claims, list) and {"release_ready", "distribution_verified"} & set(claims):
        reasons.append("Contract capability claim requires release evidence")
    return sorted(set(reasons))


def _parse_timestamp(value: str) -> datetime:
    try:
        parsed = datetime.fromisoformat(value)
    except ValueError as exc:
        raise ValueError("expiresAt must be an ISO-8601 timestamp") from exc
    if parsed.tzinfo is None:
        raise ValueError("expiresAt must include a timezone")
    return parsed.astimezone(UTC)


def _override_issues(contract: dict[str, Any], override: Any, *, now: datetime) -> list[str]:
    if not isinstance(override, dict):
        return ["override must be an object"]
    issues: list[str] = []
    for key in ("approvalEvidence", "reason"):
        if not _non_empty(override.get(key)):
            issues.append(f"override.{key} is required")
    for key in ("risks", "notRunChecks"):
        values = override.get(key)
        if (
            not isinstance(values, list)
            or not values
            or any(not _non_empty(item) for item in values)
        ):
            issues.append(f"override.{key} must contain evidence")
    if override.get("workItemOnly") is True:
        if override.get("workItemId") != contract.get("workItemId"):
            issues.append("override Work Item scope does not match")
    elif _non_empty(override.get("expiresAt")):
        try:
            if _parse_timestamp(str(override["expiresAt"])) <= now:
                issues.append("override has expired")
        except ValueError as exc:
            issues.append(str(exc))
    else:
        issues.append("override requires expiry or current Work Item scope")
    return issues


def determine(
    paths: list[str],
    policy: dict[str, Any],
    *,
    repository: Path | None = None,
    contract: dict[str, Any] | None = None,
    requested: str | None = None,
    now: datetime | None = None,
    base: str = "",
    head: str = "HEAD",
) -> dict[str, Any]:
    """Return a deterministic receipt; risk can only fall via valid Contract evidence."""
    normalized = normalize_paths(paths, repository)
    decisions: list[dict[str, Any]] = []
    for path in normalized:
        profile, path_reasons = _classify(path, policy)
        decisions.append({"path": path, "profile": profile, "reasons": path_reasons})
    risk_decisions = [item for item in decisions if item["profile"] in PROFILE_ORDER]
    if risk_decisions:
        automatic = max((item["profile"] for item in risk_decisions), key=_rank)
        reasons = sorted(
            reason
            for item in risk_decisions
            if item["profile"] == automatic
            for reason in item["reasons"]
        )
    else:
        automatic = str(policy["unknownProfile"])
        reason_kind = "evidence-only diff" if decisions else "empty diff"
        reasons = [f"{reason_kind} defaults to {automatic}"]

    selected = automatic
    source = "automatic"
    override_result: dict[str, Any] = {"applied": False, "issues": []}
    contract_data = contract if isinstance(contract, dict) else {}
    profile_record = contract_data.get("governanceProfile")
    if isinstance(profile_record, dict):
        contract_selected = profile_record.get("selected")
        contract_source = profile_record.get("source")
        if contract_selected not in PROFILE_ORDER:
            override_result["issues"].append("Contract selected profile is invalid")
        elif contract_source == "human_override" and _rank(contract_selected) < _rank(automatic):
            current_time = now or datetime.now(UTC)
            issues = _override_issues(
                contract_data, profile_record.get("override"), now=current_time
            )
            if issues:
                override_result["issues"].extend(issues)
            else:
                selected = contract_selected
                source = "human_override"
                reasons = sorted(str(item) for item in profile_record.get("reasons", []))
                override_result = {"applied": True, "issues": []}
        elif _rank(contract_selected) >= _rank(automatic):
            selected = contract_selected
            source = "automatic"
            reasons = (
                sorted(str(item) for item in profile_record.get("reasons", []) if _non_empty(item))
                or reasons
            )
        elif contract_source == "automatic":
            override_result["issues"].append(
                "Contract automatic profile cannot lower path classification"
            )

    if requested is not None:
        requested_rank = _rank(requested)
        if requested_rank < _rank(automatic):
            raise ValueError(
                f"explicit profile {requested} cannot lower automatic profile {automatic}"
            )
        if requested_rank > _rank(selected):
            selected = requested
            source = "explicit_escalation"
            reasons = [f"explicit escalation to {requested}"]

    release_reasons = _release_escalation(normalized, policy, contract_data)
    if release_reasons and _rank(selected) < _rank("strict"):
        selected = "strict"
        source = "release_escalation"
        reasons = sorted({*reasons, "release context requires strict governance"})
    escalations = ["release_preflight", "distribution"] if release_reasons else []
    config = policy["profiles"][selected]
    optional_checks = list(config["optionalChecks"])
    if release_reasons:
        optional_checks.extend(escalations)
    profile_projection = {
        "verificationDepth": config["verificationDepth"],
        "requiredEvidence": list(config["requiredEvidence"]),
        "optionalChecks": sorted(set(optional_checks)),
        "mandatoryControls": list(config["mandatoryControls"]),
    }
    explicit_strict = requested == "strict" or (
        isinstance(profile_record, dict)
        and profile_record.get("source") == "human_override"
        and selected == "strict"
    )
    risk_paths = [item["path"] for item in risk_decisions]
    routing_paths = risk_paths or normalized
    immutable_pin_facts = (
        _immutable_pin_facts(
            routing_paths,
            repository=repository,
            base=base,
            head=head,
        )
        if selected == "strict"
        else None
    )
    if selected == "strict":
        quality_routing = strict_quality_routing(
            routing_paths,
            release=bool(release_reasons),
            explicit_strict=explicit_strict,
            immutable_pin_facts=immutable_pin_facts,
        )
        required_groups = list(quality_routing["requiredGroups"])
        dispatch_target = str(quality_routing["target"])
    else:
        quality_routing = {
            "target": config["dispatchTarget"],
            "requiredGroups": list(config["requiredGroups"]),
            "reason": f"{selected} governance uses its profile dispatch target",
        }
        required_groups = list(config["requiredGroups"])
        dispatch_target = config["dispatchTarget"]
    return {
        "schemaVersion": 1,
        "base": base,
        "head": head,
        "automaticProfile": automatic,
        "selectedProfile": selected,
        "source": source,
        "reasons": reasons,
        "changedPaths": normalized,
        "pathDecisions": decisions,
        "requiredGroups": required_groups,
        "dispatchTarget": dispatch_target,
        "qualityRouting": quality_routing,
        "operationClasses": ["release"] if release_reasons else [],
        "verificationEscalations": escalations,
        "releaseEscalationReasons": release_reasons,
        "immutablePinChange": immutable_pin_facts,
        "profileProjection": profile_projection,
        "override": override_result,
    }


def _load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError(f"unable to load Contract: {exc}") from exc
    if not isinstance(value, dict):
        raise TypeError("Contract must be an object")
    return value


def discover_contract(repository: Path) -> Path | None:
    candidates = sorted((repository / ".ai/work-items/active").glob("*.contract.json"))
    if len(candidates) > 1:
        raise ValueError("multiple active Work Item Contracts found")
    return candidates[0] if candidates else None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base")
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--repository", default=".")
    parser.add_argument("--policy", default=str(DEFAULT_POLICY))
    parser.add_argument("--contract")
    parser.add_argument("--profile", choices=PROFILE_ORDER)
    parser.add_argument("--output", default="target/quality/governance-profile.json")
    args = parser.parse_args()

    repository = Path(args.repository).resolve()
    contract_path = Path(args.contract) if args.contract else discover_contract(repository)
    if contract_path is not None and not contract_path.is_absolute():
        contract_path = repository / contract_path
    contract = _load_json(contract_path) if contract_path is not None else None
    base = args.base or (contract.get("baseCommit") if contract else "HEAD")
    if not _non_empty(base):
        raise ValueError("--base or an active Contract baseCommit is required")
    policy_path = Path(args.policy)
    if not policy_path.is_absolute():
        policy_path = repository / policy_path
    result = determine(
        changed_paths(str(base), args.head, repository),
        load_policy(policy_path),
        repository=repository,
        contract=contract,
        requested=args.profile,
        base=str(base),
        head=args.head,
    )
    output = Path(args.output)
    if not output.is_absolute():
        output = repository / output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
