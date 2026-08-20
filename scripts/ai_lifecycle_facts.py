#!/usr/bin/env python3
"""Emit the repository's read-only lifecycle facts as deterministic JSON."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from ai_onboard import lifecycle_state
from bootstrap_wizard import BOOTSTRAP_CREATION_MODE, BOOTSTRAP_LIFECYCLE


def lifecycle_facts(root: Path) -> dict[str, object]:
    """Return observed lifecycle facts without asserting readiness."""
    ai_dir = root / ".ai"
    active_dir = ai_dir / "work-items" / "active"
    contracts = sorted(active_dir.glob("*.contract.json")) if active_dir.is_dir() else []
    summaries = sorted(active_dir.glob("*.summary.json")) if active_dir.is_dir() else []
    state = lifecycle_state(root)
    if not contracts and not summaries and active_dir.exists():
        state = "no_active_work_item"
    return {
        "schemaVersion": 1,
        "state": state,
        "lifecycleType": BOOTSTRAP_LIFECYCLE if state == "bootstrap" else None,
        "creationMode": BOOTSTRAP_CREATION_MODE if state == "bootstrap" else None,
        "profile": {
            "confirmed": (ai_dir / "project_profile.yaml").is_file(),
            "proposed": (ai_dir / "project_profile.proposed.yaml").is_file(),
        },
        "activeWorkItems": {
            "contractCount": len(contracts),
            "summaryCount": len(summaries),
            "contractPaths": [str(path.relative_to(root)) for path in contracts],
            "summaryPaths": [str(path.relative_to(root)) for path in summaries],
        },
        "readiness": "not_claimed",
        "enterpriseAssurance": "not_claimed",
        "notRun": ["provider_assets", "external_enterprise_assurance"],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("."))
    args = parser.parse_args()
    print(
        json.dumps(
            lifecycle_facts(args.root.resolve()), ensure_ascii=False, sort_keys=True, indent=2
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
