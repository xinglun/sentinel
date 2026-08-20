#!/usr/bin/env python3
"""Validate global consistency across distribution, docs, presets, CI, and checks."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

from ai_common import clean_git_environment
from check_docs_metadata import README_FILES, check_repository
from check_release_distribution import exercise_installer
from install_ai_cockpit import STACKS

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / ".ai" / "cockpit" / "system_invariants.json"


def make_targets(path: Path) -> set[str]:
    return {
        match.group(1)
        for line in path.read_text(encoding="utf-8").splitlines()
        if (match := re.match(r"^([A-Za-z0-9_.-]+)(?:\s+[^:]*)?:", line))
    }


def check_ids(path: Path) -> set[str]:
    text = path.read_text(encoding="utf-8")
    block = text.split("checks:", 1)[1].split("selectionRules:", 1)[0]
    return set(re.findall(r"^  ([A-Za-z][A-Za-z0-9]+):$", block, re.MULTILINE))


def local_link_issues(root: Path, path: Path) -> list[str]:
    issues = []
    text = path.read_text(encoding="utf-8")
    for link in re.findall(r"\[[^\]]+\]\(([^)]+)\)", text):
        if link.startswith(("http://", "https://", "#", "mailto:")):
            continue
        target = (path.parent / link.split("#", 1)[0]).resolve()
        if not target.exists():
            issues.append(f"{path.relative_to(root)}: broken local link: {link}")
    return issues


def workflow_pinning_issues(root: Path) -> list[str]:
    issues: list[str] = []
    for path in sorted((root / ".github" / "workflows").glob("*.y*ml")):
        text = path.read_text(encoding="utf-8")
        for number, line in enumerate(text.splitlines(), start=1):
            if re.search(r"\buses:\s+[^@\s]+@(v\d+(?:\.\d+)*|stable)\b", line):
                issues.append(
                    f"{path.relative_to(root)}:{number}: workflow action references must be pinned to a commit SHA"
                )
    return issues


def archive_summary_version_issues(root: Path) -> list[str]:
    issues: list[str] = []
    archive_root = root / ".ai" / "work-items" / "archive"
    if not archive_root.is_dir():
        return issues
    for path in sorted(archive_root.rglob("*.summary.json")):
        try:
            summary = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            issues.append(f"{path.relative_to(root)}: failed to load archived Summary: {exc}")
            continue
        summary_version = summary.get("summaryVersion")
        if summary_version is None:
            continue
        if type(summary_version) is not int or summary_version not in {1, 2}:
            issues.append(
                f"{path.relative_to(root)}: archived Summary summaryVersion must be absent or 1/2 when present"
            )
    return issues


def release_contract_issues(root: Path, release: dict[str, Any]) -> list[str]:
    target = release.get("publicContract", {}).get("projectQualityTarget")
    if not isinstance(target, str) or not re.fullmatch(r"[A-Za-z0-9_.-]+", target):
        return ["release.json public project quality target is missing or invalid"]
    marker = f"<!-- public-quality-target: {target} -->"
    paths = [
        root / "docs" / "getting-started" / "installation.md",
        root / "docs" / "getting-started" / "installation.ja.md",
        root / "docs" / "getting-started" / "installation.zh-CN.md",
    ]
    return [
        f"{path.relative_to(root)}: public quality target differs from release.json"
        for path in paths
        if marker not in path.read_text(encoding="utf-8")
    ]


def release_archive_digest(root: Path, ref: str) -> str:
    result = subprocess.run(
        ["git", "archive", "--format=tar.gz", "--prefix=ai-cockpit/", ref],
        cwd=root,
        env=clean_git_environment(),
        text=False,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(
            result.stderr.decode("utf-8", errors="replace").strip() or f"failed to archive {ref}"
        )
    return hashlib.sha256(result.stdout).hexdigest()


def invariant_issues(root: Path = ROOT) -> list[str]:
    issues = check_repository(root)
    try:
        manifest: dict[str, Any] = json.loads(
            (root / MANIFEST.relative_to(ROOT)).read_text(encoding="utf-8")
        )
        release = json.loads((root / "release.json").read_text(encoding="utf-8"))
        candidate = json.loads((root / "next-release.json").read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return [*issues, f"failed to load invariant metadata: {exc}"]
    issues.extend(release_contract_issues(root, release))
    stacks = manifest.get("stacks", [])
    if set(stacks) != STACKS:
        issues.append("system manifest stack list differs from installer STACKS")
    presets = {path.stem for path in (root / "templates" / "stacks").glob("*.mk")}
    if presets != STACKS:
        issues.append("stack presets differ from installer STACKS")
    compatibility = manifest.get("compatibility", {})
    verified = compatibility.get("verified", []) if isinstance(compatibility, dict) else []
    workflow_implemented = (
        compatibility.get("workflowImplemented", []) if isinstance(compatibility, dict) else []
    )
    preset_only = compatibility.get("presetOnly", []) if isinstance(compatibility, dict) else []
    tiers = (set(verified), set(workflow_implemented), set(preset_only))
    if set().union(*tiers) != STACKS or any(
        left & right for index, left in enumerate(tiers) for right in tiers[index + 1 :]
    ):
        issues.append("compatibility tiers must partition all supported stacks")
    workflow = (root / ".github" / "workflows" / "compatibility.yml").read_text(encoding="utf-8")
    issues.extend(workflow_pinning_issues(root))
    for stack in [*verified, *workflow_implemented]:
        if stack not in workflow:
            issues.append(f"verified stack is missing from compatibility CI: {stack}")
    tier_marker = (
        f"<!-- stack-tiers: verified={','.join(verified)}; "
        f"workflow-implemented={','.join(workflow_implemented)}; preset-only={','.join(preset_only)} -->"
    )
    flow_marker = f"<!-- governance-flow: {','.join(manifest.get('governanceFlow', []))} -->"
    capability_marker = (
        f"<!-- release-capabilities: {','.join(manifest.get('releaseCapabilities', []))} -->"
    )
    marker_owners = (
        (root / "docs" / "configuration.md", tier_marker, "compatibility tiers"),
        (
            root / "docs" / "operations" / "work-item-lifecycle.md",
            flow_marker,
            "governance flow",
        ),
        (
            root / "docs" / "reference" / "distribution.md",
            capability_marker,
            "release capabilities",
        ),
    )
    for path, marker, label in marker_owners:
        if marker not in path.read_text(encoding="utf-8"):
            issues.append(f"{path.relative_to(root)}: {label} differ from system manifest")
    distribution_text = (root / "docs" / "reference" / "distribution.md").read_text(
        encoding="utf-8"
    )
    install_line = next(
        (line for line in distribution_text.splitlines() if 'sh "$INSTALLER" --stack' in line),
        "",
    )
    for flag in manifest.get("recommendedInstallFlags", []):
        if flag not in install_line:
            issues.append(f"docs/reference/distribution.md: install command is missing {flag}")
    install_script = (root / "install.sh").read_text(encoding="utf-8")
    if not (root / "requirements-dev.lock").is_file():
        issues.append("requirements-dev.lock is missing")
    if not (root / ".ai" / "cockpit" / "bandit_low_risk_baseline.json").is_file():
        issues.append("bandit low-risk baseline is missing")
    if not (root / ".ai" / "cockpit" / "sbom.json").is_file():
        issues.append("supply-chain SBOM baseline is missing")
    if not (root / ".ai" / "cockpit" / "provenance.json").is_file():
        issues.append("supply-chain provenance baseline is missing")
    issues.extend(archive_summary_version_issues(root))
    if not (root / "SECURITY.md").is_file():
        issues.append("SECURITY.md is missing")
    if not (root / "CONTRIBUTING.md").is_file():
        issues.append("CONTRIBUTING.md is missing")
    if not (root / ".github" / "CODEOWNERS").is_file():
        issues.append("CODEOWNERS is missing")
    if not (root / ".github" / "dependabot.yml").is_file():
        issues.append("dependabot.yml is missing")
    smoke = (root / ".github" / "workflows" / "smoke.yml").read_text(encoding="utf-8")
    compatibility = (root / ".github" / "workflows" / "compatibility.yml").read_text(
        encoding="utf-8"
    )
    for workflow_name, text in (("smoke.yml", smoke), ("compatibility.yml", compatibility)):
        if "requirements-dev.lock" not in text:
            issues.append(
                f"{workflow_name}: development dependency installation must use requirements-dev.lock"
            )
        if "requirements-dev.txt" in text:
            issues.append(
                f"{workflow_name}: development dependency installation must not use requirements-dev.txt"
            )
    default_ref_match = re.search(
        r'^REF="\$\{AI_COCKPIT_TEMPLATE_REF:-(v\d+\.\d+\.\d+)\}"', install_script, re.MULTILINE
    )
    default_sha_match = re.search(
        r'^EXPECTED_SHA256="\$\{AI_COCKPIT_TEMPLATE_SHA256:-([0-9a-f]{64})?\}"',
        install_script,
        re.MULTILINE,
    )
    release_tag = release.get("releaseTag")
    installer_tag = (
        candidate.get("releaseTag")
        if candidate.get("releaseState") == "candidate" and candidate.get("published") is False
        else release_tag
    )
    if not default_ref_match or default_ref_match.group(1) != installer_tag:
        issues.append("install.sh default version differs from canonical release candidate")
    if not default_sha_match:
        issues.append("install.sh default archive digest is missing")
    elif default_sha_match.group(1) and isinstance(release_tag, str):
        try:
            expected_digest = release_archive_digest(root, release_tag)
        except RuntimeError as exc:
            issues.append(f"install.sh default archive digest could not be verified: {exc}")
        else:
            if default_sha_match.group(1) != expected_digest:
                issues.append(
                    "install.sh default archive digest differs from the release tag archive"
                )
    host_targets = make_targets(root / "Makefile")
    distribution_targets = make_targets(root / "templates" / "make" / "Makefile.ai")
    targets = host_targets | distribution_targets
    registry_ids = check_ids(root / ".ai" / "cockpit" / "checks.yaml")
    for definition in re.findall(
        r"^    command(?:Template)?:\s+make\s+([A-Za-z0-9_.-]+)",
        (root / ".ai" / "cockpit" / "checks.yaml").read_text(encoding="utf-8"),
        re.MULTILINE,
    ):
        if definition not in targets:
            issues.append(f"checks.yaml references missing Make target: {definition}")
        if definition not in distribution_targets:
            issues.append(
                f"checks.yaml references target missing from installed Makefile.ai: {definition}"
            )
    contract_template = json.loads(
        (root / ".ai" / "work-items" / "_templates" / "work_item_contract.example.json").read_text(
            encoding="utf-8"
        )
    )
    for item in contract_template.get("verification", []):
        if isinstance(item, dict) and item.get("check") not in registry_ids:
            issues.append(f"Contract example references unknown Check ID: {item.get('check')}")
    documentation = [
        *(root / name for name in README_FILES),
        *sorted((root / "docs").rglob("*.md")),
        *sorted((root / "examples").glob("*/README.md")),
    ]
    documented_targets: set[str] = set()
    make_prefix = (
        r"make[ \t]+"
        r"(?:(?:-n|--dry-run|--just-print|--recon)[ \t]+|"
        r"(?:-f|--file)[ \t]+[A-Za-z0-9_./-]+[ \t]+)*"
    )
    for path in documentation:
        text = path.read_text(encoding="utf-8")
        documented_targets.update(
            re.findall(
                rf"(?m)^[ \t]*{make_prefix}(?!-f\b|--file\b)([A-Za-z0-9_.-]+)",
                text,
            )
        )
        documented_targets.update(
            re.findall(rf"`{make_prefix}(?!-f\b|--file\b)([A-Za-z0-9_.-]+)", text)
        )
    for target in documented_targets:
        if target not in targets:
            issues.append(f"documentation references missing Make target: {target}")
    for path in documentation:
        issues.extend(local_link_issues(root, path))
    try:
        archive_capability = release["capabilities"]["sha256ArchiveVerification"]
        if isinstance(archive_capability, dict):
            sha256_supported = (
                archive_capability.get("supported") is True
                and archive_capability.get("verified") is True
            )
        else:
            sha256_supported = archive_capability is True
        exercise_installer(
            (root / "install.sh").read_bytes(),
            tag=str(release["releaseTag"]),
            sha256_supported=sha256_supported,
        )
    except (KeyError, TypeError, RuntimeError) as exc:
        issues.append(f"local distribution capability contract failed: {exc}")
    return issues


def main() -> int:
    issues = invariant_issues()
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        return 1
    print("AI Cockpit system invariants passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
