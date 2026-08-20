"""Managed-region boundary for installer-owned path classification."""


def is_managed_region(relative: str) -> bool:
    return relative.startswith((".ai/", ".cursor/", "scripts/"))
