from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path
from typing import Any

from ai_install_facts import (
    InstallFactsError,
    digest_file,
    read_json,
    validate_fact_bundle,
    write_json,
)


class ApplyError(ValueError):
    """Raised when an update cannot safely be applied."""


OPTIONS = (
    {
        "id": "apply_safe_updates",
        "label": "Apply safe updates",
        "effect": "Apply only confirmed safe and new files.",
    },
    {
        "id": "exclude_files",
        "label": "Exclude files",
        "effect": "Keep selected paths unchanged and record exclusions.",
    },
    {"id": "cancel", "label": "Cancel", "effect": "Perform no writes."},
    {
        "id": "change_target_version",
        "label": "Change target version",
        "effect": "Return to proposal generation.",
    },
    {
        "id": "review_migration",
        "label": "Review Migration",
        "effect": "Defer until the migration Work Item is confirmed.",
    },
    {"id": "return", "label": "Return", "effect": "Return without changing the installation."},
)


def _confirmation(proposal: dict[str, Any]) -> dict[str, Any]:
    return {
        "state": "needs_human_confirmation",
        "readOnly": True,
        "proposalId": proposal.get("proposalId"),
        "options": list(OPTIONS),
        "recommendedOption": "apply_safe_updates",
        "resumeCondition": "Provide confirmation marker APPLY after reviewing conflicts, removals, shared regions, and migration impact.",
        "steps": [],
    }


def _load_proposal(path: Path) -> dict[str, Any]:
    try:
        proposal = read_json(path)
    except InstallFactsError as exc:
        raise ApplyError(str(exc)) from exc
    if not isinstance(proposal, dict) or proposal.get("schemaVersion") != 1:
        raise ApplyError("proposal schema is unsupported")
    if proposal.get("readOnly") is not True:
        raise ApplyError("proposal must declare readOnly=true")
    if not isinstance(proposal.get("changes"), list) or not isinstance(
        proposal.get("source"), dict
    ):
        raise ApplyError("proposal changes and source are required")
    return proposal


def _drift_check(root: Path, proposal: dict[str, Any]) -> list[str]:
    try:
        facts = validate_fact_bundle(root)
    except InstallFactsError as exc:
        return [str(exc)]
    current = {item["path"]: item["currentDigest"] for item in facts["manifest"]["files"]}
    issues: list[str] = []
    for item in proposal["changes"]:
        path = item.get("path")
        if not isinstance(path, str) or item.get("classification") in {
            "historical_file",
            "generated_file",
        }:
            continue
        if current.get(path) != item.get("currentDigest"):
            issues.append(f"repository drift: {path}")
    return issues


def _snapshot(root: Path, proposal: dict[str, Any], snapshot: Path) -> None:
    facts = validate_fact_bundle(root)
    snapshot.mkdir(parents=True, exist_ok=True)
    for name in ("manifest.json", "version.json", "release-identity.json", "managed-regions.json"):
        shutil.copy2(root / ".ai" / "install" / name, snapshot / f"{name[:-5]}.before.json")
    (snapshot / "project-config-hash.txt").write_text(
        facts["rollbackBaseline"].get("fileDigests", {}).get(".ai/project_profile.yaml", "missing")
        + "\n",
        encoding="utf-8",
    )
    (snapshot / "migration-plan.json").write_text(
        json.dumps(proposal.get("migration", {}), indent=2) + "\n", encoding="utf-8"
    )
    (snapshot / "rollback-instructions.md").write_text(
        "Restore the before files and re-run integrity checks; preserve project-owned paths.\n",
        encoding="utf-8",
    )


def _update_facts(root: Path) -> None:
    manifest = read_json(root / ".ai" / "install" / "manifest.json")
    entries = {item["path"]: item for item in manifest["files"]}
    for path in list(entries):
        file = root / path
        if not file.is_file():
            del entries[path]
        else:
            entries[path]["installedDigest"] = digest_file(file)
            entries[path]["currentDigest"] = entries[path]["installedDigest"]
            entries[path]["projectModified"] = False
    manifest["files"] = [entries[path] for path in sorted(entries)]
    manifest_hash = write_json(root / ".ai" / "install" / "manifest.json", manifest)
    identity = read_json(root / ".ai" / "install" / "release-identity.json")
    identity["manifestHash"] = manifest_hash
    identity_hash = write_json(root / ".ai" / "install" / "release-identity.json", identity)
    version = read_json(root / ".ai" / "install" / "version.json")
    version["manifestHash"] = manifest_hash
    version["releaseIdentityHash"] = identity_hash
    write_json(root / ".ai" / "install" / "version.json", version)
    baseline = read_json(root / ".ai" / "install" / "rollback-baseline.json")
    baseline["manifestHash"] = manifest_hash
    baseline["fileDigests"] = {item["path"]: item["installedDigest"] for item in manifest["files"]}
    write_json(root / ".ai" / "install" / "rollback-baseline.json", baseline)
    validate_fact_bundle(root)


def apply_proposal(
    proposal_path: Path,
    *,
    root: Path,
    confirmation: str | None = None,
    excluded: set[str] | None = None,
) -> dict[str, Any]:
    proposal = _load_proposal(proposal_path)
    if confirmation != "APPLY":
        return _confirmation(proposal)
    issues = _drift_check(root, proposal)
    conflicts = [
        item["path"]
        for item in proposal["changes"]
        if item.get("classification") in {"conflict", "project_modified_template_file"}
    ]
    if issues or conflicts:
        return {
            "state": "blocked",
            "readOnly": True,
            "proposalId": proposal.get("proposalId"),
            "conflicts": issues + [f"unresolved conflict: {path}" for path in conflicts],
            "resumeCondition": "Regenerate or explicitly resolve the proposal before confirmation.",
            "steps": [{"name": "drift", "state": "failed"}],
        }
    excluded = excluded or set()
    new_root = Path(str(proposal["source"].get("newTemplate", "")))
    if not new_root.is_dir():
        raise ApplyError("proposal candidate template is unavailable")
    proposal_id = str(proposal.get("proposalId"))
    snapshot = root / ".ai" / "upgrade" / "snapshots" / proposal_id
    steps = [{"name": "drift", "state": "passed"}]
    _snapshot(root, proposal, snapshot)
    steps.append({"name": "snapshot", "state": "passed", "path": str(snapshot)})
    for item in proposal["changes"]:
        path = item["path"]
        category = item["classification"]
        if path in excluded or category in {
            "project_owned_file",
            "historical_file",
            "generated_file",
            "shared_managed_region",
            "unchanged_template_file",
            "conflict",
            "project_modified_template_file",
        }:
            continue
        if category not in {"safe_template_update", "new_template_file"}:
            continue
        source = new_root / path
        target = root / path
        if not source.is_file():
            raise ApplyError(f"candidate file is missing: {path}")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
    steps.extend(
        [
            {"name": "safe_files", "state": "passed"},
            {"name": "new_files", "state": "passed"},
            {
                "name": "removed_files",
                "state": "preserved",
                "reason": "removals require a separate explicit review",
            },
            {"name": "shared_regions", "state": "preserved"},
            {"name": "project_owned_retained", "state": "passed"},
            {"name": "migration", "state": "deferred"},
            {"name": "generated_regeneration", "state": "deferred"},
        ]
    )
    _update_facts(root)
    steps.extend(
        [
            {"name": "manifest_update", "state": "passed"},
            {"name": "integrity", "state": "passed"},
            {"name": "adoption_readiness", "state": "deferred"},
            {"name": "smoke_test", "state": "deferred"},
        ]
    )
    result = {
        "state": "applied",
        "readOnly": False,
        "proposalId": proposal_id,
        "steps": steps,
        "excluded": sorted(excluded),
        "summaryPath": str(root / ".ai" / "upgrade" / "summaries" / f"{proposal_id}.json"),
        "workItem": proposal.get("workItem", {}),
        "recalibrationImpact": proposal.get("recalibrationImpact", {}),
        "prHandoff": {
            "required": True,
            "state": "not_started",
            "nextAction": "create and merge the Work Item PR",
        },
        "rollbackEvidence": {"required": True, "snapshot": str(snapshot), "state": "captured"},
    }
    summary_path = root / ".ai" / "upgrade" / "summaries" / f"{proposal_id}.json"
    summary_path.parent.mkdir(parents=True, exist_ok=True)
    summary_path.write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--proposal", type=Path, required=True)
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--confirm")
    parser.add_argument("--exclude", action="append", default=[])
    args = parser.parse_args()
    try:
        result = apply_proposal(
            args.proposal.resolve(),
            root=args.root.resolve(),
            confirmation=args.confirm,
            excluded=set(args.exclude),
        )
    except ApplyError as exc:
        parser.error(str(exc))
    print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
    return 0 if result["state"] not in {"blocked", "error"} else 2


if __name__ == "__main__":
    raise SystemExit(main())
