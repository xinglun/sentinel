from __future__ import annotations

import re
from dataclasses import dataclass

OWNERSHIP_CLASSES = frozenset({"template", "project", "shared", "generated", "historical"})
BEGIN_PREFIX = "AI COCKPIT MANAGED REGION"
_BEGIN = re.compile(
    r"^\s*(?:#|//|/\*|<!--)\s*BEGIN AI COCKPIT MANAGED REGION: ([A-Za-z0-9._/-]+)\s*(?:\*/|-->)?\s*$"
)
_END = re.compile(
    r"^\s*(?:#|//|/\*|<!--)\s*END AI COCKPIT MANAGED REGION: ([A-Za-z0-9._/-]+)\s*(?:\*/|-->)?\s*$"
)


class OwnershipError(ValueError):
    pass


@dataclass(frozen=True)
class ManagedRegion:
    """An explicitly delimited region that AI Cockpit may manage."""

    name: str
    begin_line: int
    end_line: int


def ownership_label(value: str) -> str:
    return value if value in {"shared", "generated", "historical"} else f"{value}_owned"


def ownership_facts(
    *, path: str, ownership: str, installed_digest: str, current_digest: str
) -> dict[str, object]:
    """Return the canonical per-file ownership and modification evidence."""
    if ownership not in OWNERSHIP_CLASSES:
        raise OwnershipError(f"unknown ownership: {ownership}")
    return {
        "path": path,
        "ownership": ownership,
        "ownershipClass": ownership_label(ownership),
        "installedDigest": installed_digest,
        "currentDigest": current_digest,
        "projectModified": installed_digest != current_digest,
    }


def parse_managed_regions(text: str) -> tuple[ManagedRegion, ...]:
    """Parse non-nested, uniquely named BEGIN/END markers or fail closed."""
    regions: list[ManagedRegion] = []
    open_region: tuple[str, int] | None = None
    names: set[str] = set()
    for line_number, line in enumerate(text.splitlines(), 1):
        begin = _BEGIN.match(line)
        end = _END.match(line)
        if begin and end:
            raise OwnershipError(f"line {line_number}: line cannot begin and end a region")
        if begin:
            if open_region is not None:
                raise OwnershipError(f"line {line_number}: nested managed region")
            name = begin.group(1)
            if name in names:
                raise OwnershipError(f"duplicate managed region: {name}")
            names.add(name)
            open_region = (name, line_number)
        elif end:
            if open_region is None or end.group(1) != open_region[0]:
                raise OwnershipError(f"line {line_number}: unmatched managed region end")
            regions.append(ManagedRegion(open_region[0], open_region[1], line_number))
            open_region = None
    if open_region is not None:
        raise OwnershipError(f"unterminated managed region: {open_region[0]}")
    return tuple(regions)


def ownership_decision(
    *, declared: str | None, path: str, content: str | None = None
) -> dict[str, object]:
    """Return a mutation decision using declared facts, never path heuristics."""
    if declared not in OWNERSHIP_CLASSES:
        return {
            "ownership": "unknown",
            "path": path,
            "canMutate": False,
            "reason": "ownership is unknown",
        }
    if declared in {"project", "historical"}:
        return {
            "ownership": declared,
            "path": path,
            "canMutate": False,
            "reason": f"{declared} content is protected",
        }
    if declared == "shared":
        if content is None:
            return {
                "ownership": declared,
                "path": path,
                "canMutate": False,
                "reason": "managed-region content is missing",
            }
        try:
            regions = parse_managed_regions(content)
        except OwnershipError as exc:
            return {"ownership": declared, "path": path, "canMutate": False, "reason": str(exc)}
        if not regions:
            return {
                "ownership": declared,
                "path": path,
                "canMutate": False,
                "reason": "managed-region boundaries are missing",
            }
        return {
            "ownership": declared,
            "path": path,
            "canMutate": True,
            "reason": "explicit managed regions verified",
            "regions": regions,
        }
    return {
        "ownership": declared,
        "path": path,
        "canMutate": True,
        "reason": f"{declared} content is lifecycle-managed",
    }
