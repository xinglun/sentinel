"""Upgrade boundary for release-version parsing."""

import re


def release_semver(value: str) -> tuple[int, int, int]:
    match = re.fullmatch(r"v?(\d+)\.(\d+)\.(\d+)", value)
    if not match:
        raise ValueError(f"releaseVersion must be semantic version: {value!r}")
    parts = tuple(int(part) for part in match.groups())
    return parts[0], parts[1], parts[2]
