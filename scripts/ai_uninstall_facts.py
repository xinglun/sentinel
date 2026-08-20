"""Collect deterministic, fail-closed facts for installed Runtime removal."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

from ai_install_facts import canonical_json, digest_file, validate_fact_bundle


class UninstallFactsError(ValueError):
    """Raised when installed facts cannot be trusted for removal."""


def _relative_path(value: Any) -> str:
    if not isinstance(value, str) or not value or "\x00" in value:
        raise UninstallFactsError("unsafe managed path")
    path = Path(value)
    if path.is_absolute() or value.replace("\\", "/").startswith("/"):
        raise UninstallFactsError(f"unsafe managed path: {value}")
    normalized = value.replace("\\", "/")
    parts = [part for part in normalized.split("/") if part not in ("", ".")]
    if not parts or ".." in parts:
        raise UninstallFactsError(f"unsafe managed path: {value}")
    return "/".join(parts)


def _safe_path(root: Path, relative: str) -> Path:
    normalized = _relative_path(relative)
    candidate = root.joinpath(*normalized.split("/"))
    current = root
    for component in normalized.split("/"):
        current = current / component
        if current.is_symlink():
            raise UninstallFactsError(f"symlink in managed path: {relative}")
    try:
        candidate.relative_to(root)
    except ValueError as exc:
        raise UninstallFactsError(f"unsafe managed path: {relative}") from exc
    return candidate


def _repository_identity(root: Path, manifest: dict[str, Any]) -> str:
    identity = {
        "root": str(root.resolve()),
        "installationId": manifest["installationId"],
        "manifestDigest": digest_file(root / ".ai/install/manifest.json"),
    }
    return "sha256:" + hashlib.sha256(canonical_json(identity)).hexdigest()


def collect_uninstall_facts(root: Path, session_id: str) -> dict[str, Any]:
    """Return a stable facts record, or stop before producing removal authority."""
    root = root.resolve()
    if not isinstance(session_id, str) or not session_id:
        raise UninstallFactsError("session id is missing")
    manifest_path = root / ".ai/install/manifest.json"
    try:
        raw = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise UninstallFactsError("invalid installation manifest") from exc
    if not isinstance(raw, dict) or not isinstance(raw.get("files"), list):
        raise UninstallFactsError("invalid installation manifest")
    seen: set[str] = set()
    for item in raw["files"]:
        if not isinstance(item, dict):
            raise UninstallFactsError("invalid installation manifest entry")
        normalized = _relative_path(item.get("path"))
        if normalized in seen:
            raise UninstallFactsError(f"duplicate managed path: {normalized}")
        seen.add(normalized)
        _safe_path(root, normalized)

    try:
        bundle = validate_fact_bundle(root)
    except Exception as exc:
        if isinstance(exc, UninstallFactsError):
            raise
        raise UninstallFactsError(str(exc)) from exc
    manifest = bundle["manifest"]
    runtime_files: list[dict[str, Any]] = []
    preserve_paths: list[str] = []
    for item in manifest["files"]:
        relative = _relative_path(item["path"])
        path = _safe_path(root, relative)
        if path.is_symlink():
            raise UninstallFactsError(f"symlink managed path: {relative}")
        if not path.is_file():
            raise UninstallFactsError(f"managed path is not a file: {relative}")
        current_digest = digest_file(path)
        if current_digest != item.get("installedDigest"):
            raise UninstallFactsError(f"drift in managed path: {relative}")
        ownership = item.get("ownership")
        if ownership == "template" and item.get("sourcePath"):
            runtime_files.append(
                {
                    "path": relative,
                    "digest": current_digest,
                    "ownership": ownership,
                    "type": "file",
                }
            )
        else:
            preserve_paths.append(relative)
    runtime_files.sort(key=lambda item: item["path"])
    preserve_paths.sort()
    return {
        "schemaVersion": 1,
        "state": "ready",
        "sessionId": session_id,
        "installationId": manifest["installationId"],
        "repositoryIdentity": _repository_identity(root, manifest),
        "runtimeFiles": runtime_files,
        "preservePaths": preserve_paths,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--session-id", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        facts = collect_uninstall_facts(args.root, args.session_id)
    except UninstallFactsError as exc:
        result = {"state": "blocked", "reason": str(exc), "writes": []}
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(result, ensure_ascii=False, sort_keys=True))
        return 2
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(facts, ensure_ascii=False, sort_keys=True, indent=2) + "\n",
        encoding="utf-8",
    )
    print(json.dumps(facts, ensure_ascii=False, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
