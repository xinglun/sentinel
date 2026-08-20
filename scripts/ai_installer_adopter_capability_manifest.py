#!/usr/bin/env python3
"""Validate the adopter-facing capability manifest and its declared surface."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

STATUS_VALUES = {"implemented", "template_only", "adopter_installed", "planned"}
EXCLUSION_TYPES = {"external", "template_only"}
CAPABILITY_FIELDS = {
    "id",
    "title",
    "adopterFacing",
    "status",
    "truth",
    "ownership",
    "summary",
    "surfaceRole",
    "reservedSurfaceFrom",
    "templateFiles",
    "installedFiles",
    "catalogScripts",
    "makeTargets",
    "schemas",
    "entrypoints",
    "docs",
    "verifyInstalledSurface",
}
EXCLUSION_FIELDS = {
    "id",
    "adopterFacing",
    "status",
    "ownership",
    "exclusionType",
    "reason",
    "templateFiles",
    "installedFiles",
    "makeTargets",
    "schemas",
    "docs",
}
REQUIRED_CAPABILITY_FIELDS = {
    "id",
    "title",
    "adopterFacing",
    "status",
    "truth",
    "ownership",
    "summary",
    "templateFiles",
    "installedFiles",
    "catalogScripts",
    "makeTargets",
    "schemas",
    "entrypoints",
    "docs",
    "verifyInstalledSurface",
}
REQUIRED_EXCLUSION_FIELDS = EXCLUSION_FIELDS


class ManifestValidationError(ValueError):
    """Raised when the manifest is incomplete, ambiguous, or inconsistent."""


def _fail(message: str) -> None:
    raise ManifestValidationError(message)


def _walk_no_null(value: Any, path: str = "manifest") -> None:
    if value is None:
        _fail(f"{path} must not be null")
    if isinstance(value, dict):
        for key, child in value.items():
            _walk_no_null(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _walk_no_null(child, f"{path}[{index}]")


def _relative_path(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value.strip():
        _fail(f"{field} must contain non-empty relative paths")
    path = Path(value)
    if path.is_absolute() or ".." in path.parts:
        _fail(f"{field} contains unsafe path {value!r}")
    return value


def _paths(value: Any, field: str) -> list[str]:
    if not isinstance(value, list):
        _fail(f"{field} must be an array")
    return [_relative_path(item, field) for item in value]


def _source_path(root: Path, relative: str) -> Path:
    if relative == "Makefile.ai":
        return root / "templates" / "make" / "Makefile.ai"
    return root / relative


def _surface_path(root: Path, relative: str, *, installed: bool) -> Path:
    return root / relative if installed else _source_path(root, relative)


def _make_targets(path: Path) -> set[str]:
    if not path.is_file():
        return set()
    return {
        match.group(1)
        for line in path.read_text(encoding="utf-8").splitlines()
        if (match := re.match(r"^([A-Za-z0-9_.-]+):(?!=)", line))
    }


def _validate_surface(
    capability: dict[str, Any],
    *,
    root: Path,
    catalog_scripts: set[str],
    installed: bool,
) -> None:
    identifier = capability["id"]
    template_files = _paths(capability["templateFiles"], f"{identifier}.templateFiles")
    installed_files = _paths(capability["installedFiles"], f"{identifier}.installedFiles")
    scripts = _paths(capability["catalogScripts"], f"{identifier}.catalogScripts")
    targets = _paths(capability["makeTargets"], f"{identifier}.makeTargets")
    schemas = _paths(capability["schemas"], f"{identifier}.schemas")
    docs = _paths(capability["docs"], f"{identifier}.docs")
    entrypoints = capability["entrypoints"]
    if not isinstance(entrypoints, list):
        _fail(f"{identifier}.entrypoints must be an array")

    if not template_files:
        _fail(f"{identifier}.templateFiles must not be empty")
    if capability["verifyInstalledSurface"]:
        for field, values in (
            ("installedFiles", installed_files),
            ("makeTargets", targets),
            ("schemas", schemas),
        ):
            if not values:
                _fail(f"{identifier}.{field} must not be empty when installed parity is required")
    if capability["status"] == "template_only" and (
        installed_files or targets or schemas or entrypoints
    ):
        _fail(f"{identifier}: template_only capability cannot claim an installed surface")
    if capability["status"] == "planned":
        if capability.get("surfaceRole") != "reserved_reference":
            _fail(f"{identifier}: planned capability must declare surfaceRole reserved_reference")
        if not capability.get("reservedSurfaceFrom"):
            _fail(f"{identifier}: planned capability must identify its reserved surface")

    if not installed:
        for relative in template_files:
            if not _source_path(root, relative).is_file():
                _fail(f"{identifier}.templateFiles missing source file: {relative}")
    for relative in schemas:
        if relative not in template_files:
            _fail(f"{identifier}.schemas must also be declared in templateFiles: {relative}")
        if not _surface_path(root, relative, installed=installed).is_file():
            _fail(f"{identifier}.schemas missing file: {relative}")
    if not installed:
        for relative in docs:
            if not _source_path(root, relative).is_file():
                _fail(f"{identifier}.docs missing source file: {relative}")
    for relative in scripts:
        script_name = Path(relative).name
        if script_name not in catalog_scripts:
            _fail(f"{identifier}.catalogScripts is not in installer catalog: {script_name}")
    for entrypoint in entrypoints:
        if not isinstance(entrypoint, dict):
            _fail(f"{identifier}.entrypoints must contain objects")
        path = _relative_path(entrypoint.get("path"), f"{identifier}.entrypoints.path")
        if entrypoint.get("sideEffect") != "none":
            _fail(f"{identifier}.entrypoints must declare sideEffect none: {path}")
        args = entrypoint.get("args")
        if not isinstance(args, list) or not all(isinstance(arg, str) for arg in args):
            _fail(f"{identifier}.entrypoints args must be a string array: {path}")
        if not _surface_path(root, path, installed=installed).is_file():
            _fail(f"{identifier}.entrypoint missing file: {path}")
    if installed:
        for relative in installed_files:
            if not (root / relative).is_file():
                _fail(f"{identifier}.installedFiles missing file: {relative}")
        makefile = root / "Makefile.ai"
        missing_targets = sorted(set(targets) - _make_targets(makefile))
    else:
        makefile = root / "Makefile"
        template_makefile = root / "templates" / "make" / "Makefile.ai"
        missing_targets = sorted(
            set(targets) - (_make_targets(makefile) & _make_targets(template_makefile))
        )
    if missing_targets:
        _fail(
            f"{identifier}.makeTargets missing from installed/source Makefile: {', '.join(missing_targets)}"
        )


def validate_manifest(manifest: dict[str, Any], root: Path, *, installed: bool = False) -> None:
    """Validate manifest shape and every declared source or installed surface."""
    _walk_no_null(manifest)
    required_root = {
        "$schema",
        "schemaVersion",
        "schemaFile",
        "manifestId",
        "statusVocabulary",
        "ownershipVocabulary",
        "capabilityTruthRule",
        "capabilities",
        "exclusions",
    }
    missing_root = sorted(required_root - set(manifest))
    if missing_root:
        _fail(f"manifest missing required field(s): {', '.join(missing_root)}")
    if manifest["schemaVersion"] != 2:
        _fail("manifest schemaVersion must be 2")
    if manifest["schemaFile"] != ".ai/schemas/adopter-capability-manifest.schema.json":
        _fail("manifest schemaFile must identify the checked-in manifest schema")
    statuses = manifest["statusVocabulary"]
    if statuses != ["implemented", "template_only", "adopter_installed", "planned"]:
        _fail("manifest statusVocabulary is not the closed four-value vocabulary")
    ownership = manifest["ownershipVocabulary"]
    if not isinstance(ownership, list) or not ownership:
        _fail("manifest ownershipVocabulary must be a non-empty array")
    capabilities = manifest["capabilities"]
    if not isinstance(capabilities, list) or not capabilities:
        _fail("manifest capabilities must be a non-empty array")
    capability_ids: set[str] = set()
    catalog_path = root / "scripts" / "ai_installer_catalog.json"
    try:
        catalog_scripts = set(json.loads(catalog_path.read_text(encoding="utf-8"))["scripts"])
    except (OSError, KeyError, TypeError, json.JSONDecodeError) as exc:
        raise ManifestValidationError(f"cannot load installer catalog: {exc}") from exc
    for capability in capabilities:
        if not isinstance(capability, dict):
            _fail("manifest capabilities must contain objects")
        unknown = sorted(set(capability) - CAPABILITY_FIELDS)
        if unknown:
            _fail(f"{capability.get('id', '<unknown>')}: unknown field(s): {', '.join(unknown)}")
        missing = sorted(REQUIRED_CAPABILITY_FIELDS - set(capability))
        if missing:
            _fail(
                f"{capability.get('id', '<unknown>')}: missing required field(s): {', '.join(missing)}"
            )
        identifier = capability["id"]
        if not isinstance(identifier, str) or not identifier:
            _fail("capability id must be a non-empty string")
        if identifier in capability_ids:
            _fail(f"duplicate capability id: {identifier}")
        capability_ids.add(identifier)
        if capability["adopterFacing"] is not True:
            _fail(f"{identifier}.adopterFacing must be true")
        if capability["status"] not in STATUS_VALUES or capability["truth"] != capability["status"]:
            _fail(f"{identifier}.status/truth must use the same closed vocabulary value")
        if capability["ownership"] not in ownership:
            _fail(f"{identifier}.ownership is not declared in ownershipVocabulary")
        _validate_surface(
            capability,
            root=root,
            catalog_scripts=catalog_scripts,
            installed=installed,
        )
    exclusions = manifest["exclusions"]
    if not isinstance(exclusions, list) or not exclusions:
        _fail("manifest exclusions must be a non-empty array")
    exclusion_ids: set[str] = set()
    for exclusion in exclusions:
        if not isinstance(exclusion, dict):
            _fail("manifest exclusions must contain objects")
        missing = sorted(REQUIRED_EXCLUSION_FIELDS - set(exclusion))
        if missing:
            _fail(f"exclusion missing required field(s): {', '.join(missing)}")
        identifier = exclusion["id"]
        if identifier in capability_ids or identifier in exclusion_ids:
            _fail(f"duplicate capability/exclusion id: {identifier}")
        exclusion_ids.add(identifier)
        if exclusion["adopterFacing"] is not False:
            _fail(f"{identifier}.adopterFacing must be false")
        if exclusion["status"] != "excluded" or exclusion["exclusionType"] not in EXCLUSION_TYPES:
            _fail(f"{identifier} must be an explicit external/template_only exclusion")
        if exclusion["ownership"] not in ownership:
            _fail(f"{identifier}.ownership is not declared in ownershipVocabulary")
        for field in ("templateFiles", "installedFiles", "makeTargets", "schemas", "docs"):
            for relative in _paths(exclusion[field], f"{identifier}.{field}"):
                if (
                    not installed
                    and field in {"templateFiles", "docs"}
                    and not _source_path(root, relative).is_file()
                ):
                    _fail(f"{identifier}.{field} missing source file: {relative}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--installed", action="store_true")
    args = parser.parse_args(argv)
    root = args.root.resolve()
    manifest_path = (
        args.manifest or root / ".ai" / "project" / "adopter-capability-manifest.json"
    ).resolve()
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        if not isinstance(manifest, dict):
            _fail("manifest root must be an object")
        validate_manifest(manifest, root, installed=args.installed)
    except (OSError, json.JSONDecodeError, ManifestValidationError) as exc:
        print(f"adopter capability manifest check failed: {exc}", file=sys.stderr)
        return 1
    print(f"adopter capability manifest valid: {manifest_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
