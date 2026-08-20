from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any

from ai_install_facts import InstallFactsError, validate_fact_bundle

LIFECYCLE_STATES = frozenset(
    {
        "active",
        "update_available",
        "update_pending_confirmation",
        "updating",
        "rollback_available",
        "disabled",
        "uninstall_pending",
        "uninstalled_runtime_preserved_evidence",
        "purged",
        "error",
    }
)


def _error(message: str) -> dict[str, Any]:
    return {
        "schemaVersion": 1,
        "state": "error",
        "currentVersion": None,
        "targetVersion": None,
        "runtimeState": "unknown",
        "configurationState": "unknown",
        "conflicts": [message],
        "rollbackAvailable": False,
        "releaseEvidence": "not_run",
        "readOnly": True,
    }


def _release_evidence(
    path: Path | None, expected_commit: str | None, expected_release_version: str | None
) -> tuple[str, str | None]:
    if path is None:
        return "not_run", None
    try:
        evidence = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        return "invalid", f"release evidence is unreadable: {exc}"
    if (
        not isinstance(evidence, dict)
        or not evidence.get("releaseTag")
        or not evidence.get("assetDigest")
    ):
        return "invalid", "release evidence requires releaseTag and assetDigest"
    if expected_commit and evidence.get("sourceCommit") != expected_commit:
        return "invalid", "release evidence source commit does not match installed facts"
    if expected_release_version and evidence.get("releaseTag") != expected_release_version:
        return "invalid", "release evidence release tag does not match installed release version"
    return "verified", None


def _installed_release_identity(
    identity: dict[str, Any], expected_commit: str | None, expected_release_version: str | None
) -> tuple[str, str | None]:
    kind = identity.get("identityKind")
    if kind == "local_source":
        return "not_release_source", None
    if kind != "tagged_release":
        return "invalid", "installed release identity kind is unsupported"
    tag = identity.get("releaseTag")
    if not isinstance(tag, str) or not re.fullmatch(r"v\d+\.\d+\.\d+", tag):
        return "invalid", "installed release identity requires a semantic release tag"
    if expected_release_version and tag.lstrip("v") != expected_release_version.lstrip("v"):
        return "invalid", "installed release identity tag does not match installed release version"
    if expected_commit and identity.get("sourceCommit") != expected_commit:
        return "invalid", "installed release identity source commit does not match installed facts"
    return "verified", None


def installed_status(
    root: Path, target_version: str | None = None, evidence_path: Path | None = None
) -> dict[str, Any]:
    """Return a deterministic read-only status model."""
    try:
        facts = validate_fact_bundle(root)
    except InstallFactsError as exc:
        return _error(str(exc))
    version = facts["version"]
    manifest = facts["manifest"]
    current = version.get("releaseVersion") or version.get("distributionVersion")
    if evidence_path is None:
        evidence_state, evidence_error = _installed_release_identity(
            facts["releaseIdentity"],
            manifest["source"].get("sourceCommit"),
            version.get("releaseVersion"),
        )
    else:
        evidence_state, evidence_error = _release_evidence(
            evidence_path, manifest["source"].get("sourceCommit"), version.get("releaseVersion")
        )
    if evidence_error:
        return {
            **_error(evidence_error),
            "currentVersion": current,
            "targetVersion": target_version,
            "releaseEvidence": evidence_state,
        }
    state = str(version.get("runtimeState", "active"))
    if target_version and target_version != current:
        state = "update_available"
    if state not in LIFECYCLE_STATES:
        return _error(f"unsupported runtime state: {state}")
    return {
        "schemaVersion": 1,
        "state": state,
        "currentVersion": current,
        "targetVersion": target_version,
        "runtimeState": version.get("runtimeState"),
        "configurationState": "confirmed"
        if (root / ".ai" / "project_profile.yaml").is_file()
        else "unconfirmed",
        "conflicts": [],
        "ownership": {
            "files": len(manifest["files"]),
            "projectModified": [
                item["path"] for item in manifest["files"] if item.get("projectModified")
            ],
            "unknown": [
                item["path"]
                for item in manifest["files"]
                if item.get("ownership")
                not in {"template", "project", "shared", "generated", "historical"}
            ],
        },
        "rollbackAvailable": bool(facts["rollbackBaseline"].get("fileDigests")),
        "releaseEvidence": evidence_state,
        "readOnly": True,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("version", "update-check"))
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--target-version")
    parser.add_argument("--release-evidence", type=Path)
    args = parser.parse_args()
    status = installed_status(args.root.resolve(), args.target_version, args.release_evidence)
    if args.command == "version":
        status = {
            key: status[key]
            for key in ("schemaVersion", "state", "currentVersion", "runtimeState", "readOnly")
        }
    print(json.dumps(status, ensure_ascii=False, sort_keys=True, indent=2))
    return 0 if status["state"] != "error" else 2


if __name__ == "__main__":
    raise SystemExit(main())
