"""Ownership boundary for project-owned installer destinations."""


def is_project_owned(path: str) -> bool:
    return path.startswith(".ai/guards/") or path in {
        ".ai/project_profile.yaml",
        ".ai/project_profile.proposed.yaml",
    }
