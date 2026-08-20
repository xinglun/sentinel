"""Bootstrap boundary for first-adoption record locations."""

from pathlib import Path


def adoption_record_paths(target: Path) -> tuple[Path, Path]:
    active = target / ".ai" / "work-items" / "active"
    return active / "adopt_ai_cockpit.contract.json", active / "adopt_ai_cockpit.summary.json"
