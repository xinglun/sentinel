"""Classify changed repository paths for lightweight verification."""

from __future__ import annotations


def classify_path(path: str) -> str:
    """Return the first applicable verification impact domain for *path*."""
    normalized = path.replace("\\", "/")
    normalized = normalized.removeprefix("./")
    if normalized.startswith(("docs/", "README", "CHANGELOG")) or normalized.endswith(".md"):
        return "docs"
    if normalized.startswith(("tests/", "test/")) or normalized.endswith(("_test.py", ".test.py")):
        return "tests"
    if normalized.startswith((".ai/guards/", ".ai/policies/", ".ai/work-items/", ".ai/cockpit/")):
        return "trust"
    if normalized.startswith((".github/workflows/", ".gitlab-ci", ".circleci/")):
        return "release" if "release" in normalized.lower() else "workflow"
    if normalized.startswith(("scripts/install", "install.sh", "scripts/ai_installer")):
        return "installer"
    if normalized.startswith(("release", ".github/release/")) or "sbom" in normalized.lower():
        return "release"
    if normalized.startswith(
        ("requirements", "pyproject.toml", "poetry.lock", "package-lock.json")
    ):
        return "dependency"
    if normalized.startswith((".git/", ".env", "secrets/")):
        return "trust"
    if normalized.startswith("scripts/"):
        return "project_code"
    if normalized.startswith(("src/", "lib/", "app/")):
        return "project_code"
    return "unknown"
