#!/usr/bin/env python3
"""Report AI Cockpit environment and adoption readiness without modifying files."""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

from ai_check_adoption_ready import (
    readiness_failures,
    readiness_role_message,
    template_exemption,
)
from ai_common import clean_git_environment
from ai_install_facts import InstallFactsError, validate_fact_bundle
from ai_lifecycle_truth import validate_successor_receipt
from ai_project_profile import load_profile
from ai_start import linked_worktree_identity_report


def command_ok(root: Path, *command: str) -> bool:
    try:
        return (
            subprocess.run(
                command,
                cwd=root,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
                env=clean_git_environment(),
            ).returncode
            == 0
        )
    except OSError:
        return False


def git_records(output: str) -> list[str]:
    if "\0" in output:
        return [item for item in output.split("\0") if item]
    return [line for line in output.splitlines() if line]


def installation_diagnosis(root: Path) -> dict[str, Any]:
    """Read immutable installation facts and retain every contradiction."""
    try:
        facts = validate_fact_bundle(root)
    except InstallFactsError as exc:
        return {"available": False, "reason": str(exc), "conflicts": []}
    version = facts["version"]
    manifest = facts["manifest"]
    identity = facts["releaseIdentity"]
    requested = manifest["source"].get("distributionVersion")
    installed = version.get("distributionVersion")
    conflicts: list[str] = []
    if requested != installed:
        conflicts.append("requested version does not match installed version")
    return {
        "available": True,
        "requestedVersion": requested,
        "installedVersion": installed,
        "sourceCommit": version.get("sourceCommit"),
        "releaseTag": identity.get("releaseTag"),
        "assetDigests": identity.get("artifactDigests", {}),
        "conflicts": conflicts,
    }


def runtime_diagnosis(root: Path) -> dict[str, Any]:
    """Collect local lifecycle, command, Outcome, and snapshot readiness facts."""
    makefile = root / "Makefile"
    targets: list[str] = []
    if makefile.is_file():
        for line in makefile.read_text(encoding="utf-8").splitlines():
            match = re.fullmatch(r"([A-Za-z0-9][A-Za-z0-9_.-]*):", line)
            if match and match.group(1).startswith("ai-"):
                targets.append(match.group(1))
    active = root / ".ai" / "work-items" / "active"
    outcomes: list[dict[str, Any]] = []
    if active.is_dir():
        for path in sorted(active.glob("*.outcome.json")):
            try:
                outcome = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, ValueError):
                outcomes.append({"path": str(path.relative_to(root)), "state": "unreadable"})
                continue
            if isinstance(outcome, dict):
                outcomes.append({"path": str(path.relative_to(root)), **outcome})
    snapshot = root / "target" / "hosted-verification-snapshot.json"
    hosted: dict[str, Any]
    if snapshot.is_file():
        try:
            value = json.loads(snapshot.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            hosted = {
                "state": "blocked",
                "recovery": "Regenerate the hosted verification snapshot from a clean dedicated Work Item branch",
            }
        else:
            hosted = {"state": "ready" if isinstance(value, dict) else "blocked", "receipt": value}
    else:
        hosted = {
            "state": "not_ready",
            "recovery": "Run make ai-prepare-hosted-verification-snapshot only when the active Contract explicitly requires hosted verification",
        }
    return {
        "availableTargets": sorted(set(targets)),
        "outcomes": outcomes,
        "hostedSnapshot": hosted,
    }


def diagnose(root: Path) -> tuple[list[str], list[str], list[str]]:
    passed: list[str] = []
    warnings: list[str] = []
    failures: list[str] = []

    install = installation_diagnosis(root)
    if not install.get("available", True):
        warnings.append(
            "installation facts are unavailable (template-maintenance or uninstalled runtime): "
            f"{install['reason']}. Recovery: run the published installer before diagnosing an adopter installation"
        )
    for key in ("requestedVersion", "installedVersion", "sourceCommit", "releaseTag"):
        if install.get(key) is not None:
            passed.append(f"installation {key}={install[key]}")
    asset_digests = install.get("assetDigests", {})
    if isinstance(asset_digests, dict):
        for name, value in sorted(asset_digests.items()):
            if isinstance(name, str) and isinstance(value, str):
                passed.append(f"installation assetDigest {name}={value}")
    for conflict in install.get("conflicts", []):
        failures.append(
            "installation contradiction: "
            f"{conflict}. Recovery: reinstall from the selected immutable release tag and re-run make ai-doctor"
        )

    runtime = runtime_diagnosis(root)
    targets = runtime["availableTargets"]
    passed.append("available ai Make targets: " + (", ".join(targets) if targets else "none"))
    for outcome in runtime["outcomes"]:
        if outcome.get("status") == "blocked":
            warnings.append(
                "Outcome blocked "
                f"(color={outcome.get('humanStatusColor', 'unknown')}, gate={outcome.get('failedGate', 'unknown')}, "
                f"recovery={outcome.get('recoveryCondition', 'not recorded')})"
            )
    hosted = runtime["hostedSnapshot"]
    if hosted.get("state") != "ready":
        warnings.append(
            f"hosted snapshot {hosted.get('state')}; Recovery: {hosted.get('recovery', 'inspect hosted snapshot evidence')}"
        )

    python_version = (sys.version_info.major, sys.version_info.minor)
    if python_version >= (3, 11):
        passed.append(f"Python {python_version[0]}.{python_version[1]} satisfies 3.11+")
    else:
        failures.append("Python 3.11 or newer is required")
    for command in ("git", "make"):
        (passed if shutil.which(command) else failures).append(
            f"{command} is available" if shutil.which(command) else f"{command} is required on PATH"
        )
    if os.name == "posix":
        passed.append("POSIX runtime detected")
    else:
        failures.append("A POSIX shell environment is required; use WSL on Windows")

    if command_ok(root, "git", "rev-parse", "--is-inside-work-tree"):
        passed.append("Git repository detected")
    else:
        failures.append("Run inside a Git repository")
    if command_ok(root, "git", "rev-parse", "--verify", "HEAD"):
        passed.append("Initial Git commit detected")
    else:
        failures.append("Create an initial Git commit before ai-start or --create-adoption")
    active = root / ".ai" / "work-items" / "active"
    if active.is_dir():
        for outcome_path in sorted(active.glob("*.outcome.json")):
            try:
                outcome = __import__("json").loads(outcome_path.read_text(encoding="utf-8"))
            except (OSError, ValueError):
                warnings.append(f"Lifecycle Outcome is unreadable: {outcome_path}")
                continue
            if isinstance(outcome, dict) and outcome.get("status") == "blocked":
                warnings.append(
                    "Lifecycle Outcome is blocked "
                    f"(color={outcome.get('humanStatusColor', 'unknown')}, "
                    f"gate={outcome.get('failedGate', outcome.get('failedCheck', 'unknown'))}, "
                    f"recovery={outcome.get('recoveryCondition', 'not recorded')})"
                )
            receipt_path = outcome_path.with_name(
                outcome_path.name.removesuffix(".outcome.json") + ".successor-receipt.json"
            )
            if receipt_path.is_file():
                try:
                    receipt = __import__("json").loads(receipt_path.read_text(encoding="utf-8"))
                except (OSError, ValueError):
                    failures.append(f"Successor receipt is unreadable: {receipt_path}")
                else:
                    reason = validate_successor_receipt(
                        predecessor_outcome=outcome_path,
                        predecessor_work_item_id=str(outcome.get("workItemId", "")),
                        receipt=receipt,
                    )
                    if reason is None:
                        warnings.append(
                            "Lifecycle successor route is recorded "
                            f"(color=yellow, transition={receipt['transition']}, "
                            f"successor={receipt['successorWorkItemId']}); it is not closure authorization"
                        )
                    else:
                        failures.append(f"Successor receipt is invalid ({reason}): {receipt_path}")
    identities, identity_errors = linked_worktree_identity_report(root=root)
    warnings.extend(identity_errors)
    for identity in identities:
        if identity.branch != f"codex/{identity.task}":
            warnings.append(
                "Linked worktree active Work Item identity "
                f"{identity.branch} != codex/{identity.task}: {identity.worktree}; "
                "it is isolated for unrelated starts and remains fail-closed for its own task."
            )
    try:
        dirty = subprocess.run(
            ["git", "status", "--porcelain", "-z"],
            cwd=root,
            text=True,
            capture_output=True,
            check=False,
            env=clean_git_environment(),
        ).stdout
    except OSError:
        dirty = ""
    if git_records(dirty):
        warnings.append("Git worktree is dirty; --create-adoption requires a clean worktree")
    else:
        passed.append("Git worktree is clean")

    stack = root / "Makefile.ai.stack"
    if not stack.is_file():
        warnings.append("Makefile.ai.stack is missing; install or select a stack preset")
    else:
        text = stack.read_text(encoding="utf-8")
        if "configure PROJECT_" in text or "No project" in text:
            warnings.append("Project quality commands are still placeholders/fail-closed defaults")
        else:
            passed.append("Project quality commands are configured")
    coverage = root / ".ai" / "guards" / "coverage_policy.yaml"
    if coverage.is_file():
        warnings.append("Review Coverage Guard production/test paths against the project layout")
    else:
        warnings.append("Coverage Guard policy is missing")
    if (root / ".github" / "workflows").is_dir() or (root / ".gitlab-ci.yml").is_file():
        passed.append("CI configuration detected; verify merge-base wiring manually")
    else:
        warnings.append("No GitHub Actions or GitLab CI configuration detected for check-ai-pr")
    profile, profile_issues = load_profile(
        root / ".ai" / "project_profile.yaml", require_approval=True
    )
    maintenance_mode = (
        not profile_issues
        and template_exemption(profile, root, execution_mode="template_maintenance")[0]
    )
    previous_mode = os.environ.get("AI_COCKPIT_EXECUTION_MODE")
    if maintenance_mode:
        os.environ["AI_COCKPIT_EXECUTION_MODE"] = "template_maintenance"
    try:
        readiness = readiness_failures(root)
        role_message = readiness_role_message(root)
    finally:
        if previous_mode is None:
            os.environ.pop("AI_COCKPIT_EXECUTION_MODE", None)
        else:
            os.environ["AI_COCKPIT_EXECUTION_MODE"] = previous_mode
    if readiness:
        warnings.append(role_message)
        warnings.append("Run make check-ai-adoption-ready before enabling production gates")
    else:
        passed.append(role_message)
        passed.append("Adoption readiness configuration is complete")
    return passed, warnings, failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="Repository root to inspect.")
    args = parser.parse_args()
    root = Path(args.root).resolve()
    passed, warnings, failures = diagnose(root)
    for item in passed:
        print(f"[PASS] {item}")
    for item in warnings:
        print(f"[WARN] {item}")
    for item in failures:
        print(f"[FAIL] {item}")
    print(
        f"doctor summary: {len(passed)} passed, {len(warnings)} warning(s), {len(failures)} failure(s)"
    )
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
