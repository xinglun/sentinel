from __future__ import annotations

import hashlib
import json
import re
import uuid
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

FACT_DIR = Path(".ai/install")
FACT_NAMES = (
    "manifest.json",
    "version.json",
    "release-identity.json",
    "managed-regions.json",
    "rollback-baseline.json",
)
OWNERSHIPS = {"template", "project", "shared", "generated", "historical"}
IGNORED_ROOTS = {"target"}


class InstallFactsError(ValueError):
    pass


def ownership_label(value: str) -> str:
    return value if value in {"shared", "generated", "historical"} else f"{value}_owned"


def canonical_json(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode()


def digest_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def digest_file(path: Path) -> str:
    return digest_bytes(path.read_bytes())


def write_json(path: Path, value: Any) -> str:
    payload = canonical_json(value)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    return digest_bytes(payload)


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise InstallFactsError(f"invalid installation fact: {path}") from exc


def classify_path(relative: str) -> str:
    if relative in {"README.md", "README.zh-CN.md", "README.ja.md"}:
        return "project"
    if relative.startswith(".ai/work-items/archive/"):
        return "historical"
    if relative.startswith((".ai/cockpit/", ".ai/install/")):
        return "generated"
    if relative.startswith((".ai/guards/", ".cursor/")):
        return "shared"
    if relative.startswith(".ai/project") or relative == ".ai/glossary.md":
        return "project"
    return "template"


def _source_commit(source: Path) -> str | None:
    try:
        import subprocess

        result = subprocess.run(
            ["git", "-C", str(source), "rev-parse", "HEAD"],
            text=True,
            capture_output=True,
            check=False,
        )
    except OSError:
        return None
    return result.stdout.strip() if result.returncode == 0 else None


def build_manifest(
    *,
    source: Path,
    target: Path,
    distribution_version: dict[str, Any],
    source_commit: str | None = None,
) -> dict[str, Any]:
    files: list[dict[str, Any]] = []
    for path in sorted(target.rglob("*")):
        if not path.is_file():
            continue
        relative_path = path.relative_to(target)
        relative = relative_path.as_posix()
        if relative_path.parts and relative_path.parts[0] in IGNORED_ROOTS:
            continue
        if FACT_DIR.as_posix() in relative:
            continue
        if relative == ".ai/cockpit/.install.lock":
            continue
        if relative.startswith(".git/") or relative in {".gitignore"}:
            continue
        if relative.startswith(".ai/work-items/active/"):
            continue
        source_path = relative if (source / relative).is_file() else ""
        files.append(
            {
                "path": relative,
                "sourcePath": source_path,
                "ownership": classify_path(relative),
                "installedDigest": digest_file(path),
                "currentDigest": digest_file(path),
                "projectModified": False,
                "ownershipClass": ownership_label(classify_path(relative)),
            }
        )
    installed_at = datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    return {
        "schemaVersion": 1,
        "installationId": str(uuid.uuid4()),
        "installedAt": installed_at,
        "source": {
            "distributionVersion": distribution_version.get("distributionVersion"),
            "releaseVersion": distribution_version.get("releaseVersion"),
            "contractSchema": distribution_version.get("contractSchema"),
            "sourceCommit": source_commit if source_commit is not None else _source_commit(source),
        },
        "files": files,
    }


def _canonical_release_tag(value: str) -> str:
    match = re.fullmatch(r"v?(\d+)\.(\d+)\.(\d+)", value)
    if not match:
        raise InstallFactsError(f"release identity version is not semantic: {value!r}")
    return "v" + ".".join(match.groups())


def _release_identity(
    *,
    distribution_version: dict[str, Any],
    source_commit: str | None,
    identity: dict[str, Any] | None,
) -> dict[str, Any]:
    release_version = distribution_version.get("releaseVersion")
    if identity is None:
        return {
            "schemaVersion": 1,
            "identityKind": "local_source",
            "releaseTag": None,
            "releaseVersion": release_version,
            "sourceCommit": source_commit,
            "tagTarget": None,
            "metadataCommit": None,
            "artifactDigests": {},
        }
    required = ("releaseTag", "releaseVersion", "sourceCommit", "tagTarget", "metadataCommit")
    if not isinstance(identity, dict) or any(
        not isinstance(identity.get(key), str) for key in required
    ):
        raise InstallFactsError("release identity requires tag, version, and commit fields")
    tag = _canonical_release_tag(identity["releaseTag"])
    if (
        _canonical_release_tag(identity["releaseVersion"]) != tag
        or not isinstance(release_version, str)
        or _canonical_release_tag(release_version) != tag
    ):
        raise InstallFactsError("release identity tag does not match distribution version")
    commits = {identity[key] for key in ("sourceCommit", "tagTarget", "metadataCommit")}
    if len(commits) != 1 or not all(re.fullmatch(r"[0-9a-f]{40}", value) for value in commits):
        raise InstallFactsError("release identity commit fields must be one matching SHA-1")
    if source_commit is not None and source_commit != identity["sourceCommit"]:
        raise InstallFactsError("release identity source commit does not match installer source")
    artifacts = identity.get("artifactDigests")
    if (
        not isinstance(artifacts, dict)
        or not artifacts
        or any(
            not isinstance(name, str)
            or not isinstance(digest, str)
            or not re.fullmatch(r"[0-9a-f]{64}", digest)
            for name, digest in artifacts.items()
        )
    ):
        raise InstallFactsError("release identity requires non-empty SHA-256 artifact digests")
    return {
        "schemaVersion": 1,
        "identityKind": "tagged_release",
        "releaseTag": tag,
        "releaseVersion": release_version,
        "sourceCommit": identity["sourceCommit"],
        "tagTarget": identity["tagTarget"],
        "metadataCommit": identity["metadataCommit"],
        "artifactDigests": dict(sorted(artifacts.items())),
    }


def write_fact_bundle(
    *,
    source: Path,
    target: Path,
    distribution_version: dict[str, Any],
    release_identity: dict[str, Any] | None = None,
) -> dict[str, Any]:
    source_commit = _source_commit(source)
    identity = _release_identity(
        distribution_version=distribution_version,
        source_commit=source_commit,
        identity=release_identity,
    )
    manifest = build_manifest(
        source=source,
        target=target,
        distribution_version=distribution_version,
        source_commit=identity["sourceCommit"],
    )
    manifest_path = target / FACT_DIR / "manifest.json"
    manifest_hash = write_json(manifest_path, manifest)
    identity = {
        **identity,
        "installationId": manifest["installationId"],
        "manifestHash": manifest_hash,
    }
    identity_hash = write_json(target / FACT_DIR / "release-identity.json", identity)
    version = {
        "schemaVersion": 1,
        "installationId": manifest["installationId"],
        "installedAt": manifest["installedAt"],
        "distributionVersion": distribution_version.get("distributionVersion"),
        "releaseVersion": distribution_version.get("releaseVersion"),
        "contractSchema": distribution_version.get("contractSchema"),
        "sourceCommit": manifest["source"]["sourceCommit"],
        "manifestHash": manifest_hash,
        "releaseIdentityHash": identity_hash,
        "runtimeState": "active",
    }
    regions = {
        "schemaVersion": 1,
        "installationId": manifest["installationId"],
        "regions": [
            {
                "path": item["path"],
                "ownership": item["ownership"],
                "ownershipClass": item["ownershipClass"],
                "region": "full-file",
                "installedDigest": item["installedDigest"],
                "currentDigest": item["currentDigest"],
                "projectModified": item["projectModified"],
            }
            for item in manifest["files"]
            if item["ownership"] == "shared"
        ],
    }
    baseline = {
        "schemaVersion": 1,
        "installationId": manifest["installationId"],
        "createdAt": manifest["installedAt"],
        "manifestHash": manifest_hash,
        "fileDigests": {item["path"]: item["installedDigest"] for item in manifest["files"]},
    }
    write_json(target / FACT_DIR / "version.json", version)
    write_json(target / FACT_DIR / "managed-regions.json", regions)
    write_json(target / FACT_DIR / "rollback-baseline.json", baseline)
    return validate_fact_bundle(target)


def validate_fact_bundle(root: Path) -> dict[str, Any]:
    paths = {name: root / FACT_DIR / name for name in FACT_NAMES}
    if any(not path.is_file() for path in paths.values()):
        missing = [name for name, path in paths.items() if not path.is_file()]
        raise InstallFactsError(f"missing installation facts: {', '.join(missing)}")
    manifest = read_json(paths["manifest.json"])
    version = read_json(paths["version.json"])
    regions = read_json(paths["managed-regions.json"])
    baseline = read_json(paths["rollback-baseline.json"])
    identity = read_json(paths["release-identity.json"])
    if not isinstance(manifest, dict) or manifest.get("schemaVersion") != 1:
        raise InstallFactsError("manifest schema is unsupported")
    if not isinstance(manifest.get("installationId"), str) or not manifest["installationId"]:
        raise InstallFactsError("manifest installationId is missing")
    files = manifest.get("files")
    if not isinstance(files, list) or not files:
        raise InstallFactsError("manifest files are missing")
    current_files: list[dict[str, Any]] = []
    for item in files:
        if not isinstance(item, dict) or not isinstance(item.get("path"), str):
            raise InstallFactsError("manifest contains an invalid file entry")
        if item.get("ownership") not in OWNERSHIPS or not isinstance(
            item.get("installedDigest"), str
        ):
            raise InstallFactsError("manifest contains an invalid ownership or digest")
        path = root / item["path"]
        if not path.is_file():
            raise InstallFactsError(f"installation fact digest mismatch: {item['path']}")
        current_digest = digest_file(path)
        recorded_current = item.get("currentDigest")
        if (
            recorded_current is not None
            and recorded_current != item["installedDigest"]
            and recorded_current == current_digest
        ):
            raise InstallFactsError(f"installation manifest was tampered: {item['path']}")
        if recorded_current is not None and not isinstance(recorded_current, str):
            raise InstallFactsError("manifest contains an invalid current digest")
        if item.get("ownershipClass") != ownership_label(item["ownership"]):
            raise InstallFactsError(f"manifest contains an invalid ownership class: {item['path']}")
        current_files.append(
            {
                **item,
                "currentDigest": current_digest,
                "projectModified": current_digest != item["installedDigest"],
            }
        )
    manifest = {**manifest, "files": current_files}
    expected_manifest_hash = digest_file(paths["manifest.json"])
    if not isinstance(version, dict) or version.get("schemaVersion") != 1:
        raise InstallFactsError("version fact schema is unsupported")
    if (
        version.get("installationId") != manifest["installationId"]
        or version.get("manifestHash") != expected_manifest_hash
    ):
        raise InstallFactsError("version fact is not bound to the manifest")
    if version.get("sourceCommit") != manifest["source"].get("sourceCommit"):
        raise InstallFactsError("version fact source commit does not match the manifest")
    if not isinstance(identity, dict) or identity.get("schemaVersion") != 1:
        raise InstallFactsError("release identity schema is unsupported")
    if (
        identity.get("installationId") != manifest["installationId"]
        or identity.get("manifestHash") != expected_manifest_hash
        or version.get("releaseIdentityHash") != digest_file(paths["release-identity.json"])
    ):
        raise InstallFactsError("release identity is not bound to the installation facts")
    if identity.get("sourceCommit") != manifest["source"].get("sourceCommit") or identity.get(
        "releaseVersion"
    ) != version.get("releaseVersion"):
        raise InstallFactsError("release identity does not match installed facts")
    if identity.get("identityKind") == "tagged_release":
        _release_identity(
            distribution_version=version, source_commit=identity["sourceCommit"], identity=identity
        )
    elif identity.get("identityKind") != "local_source":
        raise InstallFactsError("release identity kind is unsupported")
    if not isinstance(regions, dict) or regions.get("installationId") != manifest["installationId"]:
        raise InstallFactsError("managed-regions fact is not bound to the manifest")
    if (
        not isinstance(baseline, dict)
        or baseline.get("installationId") != manifest["installationId"]
    ):
        raise InstallFactsError("rollback baseline is not bound to the manifest")
    if baseline.get("fileDigests") != {item["path"]: item["installedDigest"] for item in files}:
        raise InstallFactsError("rollback baseline digests do not match the manifest")
    return {
        "manifest": manifest,
        "version": version,
        "releaseIdentity": identity,
        "managedRegions": regions,
        "rollbackBaseline": baseline,
    }
