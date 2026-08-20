from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

from ai_install_facts import InstallFactsError, digest_file, validate_fact_bundle
from ai_ownership import OwnershipError, parse_managed_regions

CLASSIFICATIONS = frozenset(
    {
        "unchanged_template_file",
        "safe_template_update",
        "project_modified_template_file",
        "project_owned_file",
        "shared_managed_region",
        "new_template_file",
        "removed_template_file",
        "generated_file",
        "historical_file",
        "conflict",
    }
)


class ProposalError(ValueError):
    """Raised when proposal inputs are incomplete or inconsistent."""


def _json_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode()


def _hash_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _candidate_files(root: Path) -> dict[str, str]:
    if not root.is_dir():
        raise ProposalError(f"template root does not exist: {root}")
    files: dict[str, str] = {}
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        relative = path.relative_to(root).as_posix()
        if relative.startswith(".git/") or relative in {".gitignore"}:
            continue
        if relative.startswith((".ai/work-items/active/", ".ai/install/")):
            continue
        files[relative] = digest_file(path)
    return files


def _manifest_hash(root: Path) -> str | None:
    path = root / ".ai" / "install" / "manifest.json"
    return digest_file(path) if path.is_file() else None


def _ownership(manifest: dict[str, Any]) -> dict[str, str]:
    return {item["path"]: item["ownership"] for item in manifest.get("files", [])}


def _digest_map(manifest: dict[str, Any]) -> dict[str, str]:
    return {item["path"]: item["installedDigest"] for item in manifest.get("files", [])}


def _region_delta(old_path: Path, new_path: Path, current_path: Path) -> dict[str, Any]:
    result: dict[str, Any] = {"regions": [], "error": None}
    try:
        old = (
            {
                item.name: item
                for item in parse_managed_regions(old_path.read_text(encoding="utf-8"))
            }
            if old_path.is_file()
            else {}
        )
        new = (
            {
                item.name: item
                for item in parse_managed_regions(new_path.read_text(encoding="utf-8"))
            }
            if new_path.is_file()
            else {}
        )
        current = (
            {
                item.name: item
                for item in parse_managed_regions(current_path.read_text(encoding="utf-8"))
            }
            if current_path.is_file()
            else {}
        )
    except (OSError, UnicodeError, OwnershipError) as exc:
        result["error"] = str(exc)
        return result
    for name in sorted(set(old) | set(new) | set(current)):
        result["regions"].append(
            {
                "name": name,
                "oldPresent": name in old,
                "newPresent": name in new,
                "currentPresent": name in current,
                "changed": (name in old) != (name in new)
                or (name in old and name in new and old[name] != new[name]),
            }
        )
    return result


def _classification(
    *,
    path: str,
    ownership: str | None,
    old_digest: str | None,
    new_digest: str | None,
    current_digest: str | None,
) -> tuple[str, str]:
    if ownership == "historical":
        return "historical_file", "Historical evidence is preserved and never updated."
    if ownership == "generated":
        return "generated_file", "Generated output is regenerated only by a later apply workflow."
    if ownership == "project":
        return (
            "project_owned_file",
            "Project-owned content is retained for explicit project decisions.",
        )
    if ownership == "shared":
        return (
            "shared_managed_region",
            "Shared content requires managed-region review before any apply.",
        )
    if old_digest is None and new_digest is not None:
        return "new_template_file", "The candidate release adds a template file."
    if old_digest is not None and new_digest is None:
        return "removed_template_file", "The candidate release removes a template file."
    if old_digest == new_digest == current_digest:
        return (
            "unchanged_template_file",
            "Old template, candidate template, and current content agree.",
        )
    if old_digest == new_digest:
        return (
            "unchanged_template_file",
            "The template did not change; current content is drift outside this proposal.",
        )
    if current_digest in {None, old_digest}:
        return (
            "safe_template_update",
            "Current content matches the installed baseline and can follow the candidate.",
        )
    if current_digest == new_digest:
        return "safe_template_update", "Current content already matches the candidate release."
    return (
        "conflict",
        "Current content differs from both the installed and candidate template versions.",
    )


def build_proposal(
    *,
    old_template: Path,
    new_template: Path,
    current_project: Path,
    upgrade_id: str,
    release_evidence: Path | None = None,
) -> dict[str, Any]:
    try:
        facts = validate_fact_bundle(current_project)
    except InstallFactsError as exc:
        raise ProposalError(str(exc)) from exc
    old_files = _candidate_files(old_template)
    new_files = _candidate_files(new_template)
    current_manifest = facts["manifest"]
    current_files = _digest_map(current_manifest)
    ownership = _ownership(current_manifest)
    entries: list[dict[str, Any]] = []
    for path in sorted(set(old_files) | set(new_files) | set(current_files)):
        old_digest, new_digest, current_digest = (
            old_files.get(path),
            new_files.get(path),
            current_files.get(path),
        )
        category, reason = _classification(
            path=path,
            ownership=ownership.get(path),
            old_digest=old_digest,
            new_digest=new_digest,
            current_digest=current_digest,
        )
        item: dict[str, Any] = {
            "path": path,
            "classification": category,
            "oldDigest": old_digest,
            "newDigest": new_digest,
            "currentDigest": current_digest,
            "projectModified": current_digest not in {None, old_digest},
            "ownership": ownership.get(path, "template"),
            "reason": reason,
            "canApplyAutomatically": category == "safe_template_update",
        }
        if category == "shared_managed_region":
            item["managedRegionDiff"] = _region_delta(
                old_template / path, new_template / path, current_project / path
            )
        entries.append(item)

    conflicts = [item for item in entries if item["classification"] == "conflict"]
    shared = [item for item in entries if item["classification"] == "shared_managed_region"]
    safe = [item for item in entries if item["classification"] == "safe_template_update"]
    added = [item for item in entries if item["classification"] == "new_template_file"]
    removed = [item for item in entries if item["classification"] == "removed_template_file"]
    evidence: dict[str, Any] = {"state": "not_run"}
    if release_evidence is not None:
        try:
            evidence = json.loads(release_evidence.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, json.JSONDecodeError) as exc:
            raise ProposalError(f"release evidence is unreadable: {exc}") from exc
        if (
            not isinstance(evidence, dict)
            or not evidence.get("releaseTag")
            or not evidence.get("assetDigest")
        ):
            raise ProposalError("release evidence requires releaseTag and assetDigest")
    baseline = facts["rollbackBaseline"].get("fileDigests", {})
    return {
        "schemaVersion": 1,
        "proposalId": upgrade_id,
        "state": "needs_human_confirmation"
        if conflicts or removed or shared
        else "ready_for_confirmation",
        "readOnly": True,
        "source": {
            "oldTemplate": str(old_template),
            "newTemplate": str(new_template),
            "currentProject": str(current_project),
            "installedVersion": facts["version"].get("releaseVersion")
            or facts["version"].get("distributionVersion"),
            "candidateManifestHash": _manifest_hash(new_template),
            "installedManifestHash": facts["version"].get("manifestHash"),
        },
        "releaseEvidence": evidence,
        "summary": {
            "safeUpdates": [item["path"] for item in safe],
            "newFiles": [item["path"] for item in added],
            "removedFiles": [item["path"] for item in removed],
            "conflicts": [item["path"] for item in conflicts],
            "sharedChanges": [item["path"] for item in shared],
        },
        "changes": entries,
        "migration": {"required": False, "status": "deferred", "references": []},
        "recalibrationImpact": {"required": True, "state": "not_run", "inventory": []},
        "workItem": {
            "required": True,
            "lifecycle": "standard_work_item",
            "artifacts": [
                "snapshot",
                "migration",
                "recalibrationImpact",
                "summary",
                "pr",
                "rollbackEvidence",
            ],
            "nextAction": "create Contract v2 before apply",
        },
        "prHandoff": {"required": True, "state": "not_started", "branch": None, "pr": None},
        "rollbackEvidence": {
            "required": True,
            "source": ".ai/install/rollback-baseline.json",
            "state": "available" if baseline else "missing",
        },
        "baselineImpact": {"snapshotRequired": True, "fileDigests": baseline},
        "documentationImpact": {"required": bool(safe or added or removed or shared), "paths": []},
        "rollbackSnapshot": {
            "required": True,
            "source": ".ai/install/rollback-baseline.json",
            "available": bool(baseline),
        },
        "resumeCondition": "Human reviews conflicts, shared regions, and removals, then invokes the confirmation/apply Work Item.",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old-template", type=Path, required=True)
    parser.add_argument("--new-template", type=Path, required=True)
    parser.add_argument("--current-project", type=Path, default=Path("."))
    parser.add_argument("--upgrade-id", required=True)
    parser.add_argument("--release-evidence", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        proposal = build_proposal(
            old_template=args.old_template.resolve(),
            new_template=args.new_template.resolve(),
            current_project=args.current_project.resolve(),
            upgrade_id=args.upgrade_id,
            release_evidence=args.release_evidence.resolve() if args.release_evidence else None,
        )
    except ProposalError as exc:
        parser.error(str(exc))
    payload = _json_bytes(proposal)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(payload)
    print(payload.decode(), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
