#!/usr/bin/env python3
"""Signal Context の観測事実と coverage 契約を検証する。"""

from __future__ import annotations

import argparse
from datetime import datetime
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def load_cases(path: Path) -> list[dict]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(value, dict):
        value = [value]
    if not isinstance(value, list):
        raise ValueError(f"fixture root must be object or list: {path}")
    return [item for item in value if isinstance(item, dict)]


def all_items(case: dict) -> list[dict]:
    names = (
        "scheduled_macro",
        "corporate_events",
        "geopolitical_events",
        "commodity_events",
        "rates_credit_events",
        "market_structure_events",
    )
    return [item for name in names for item in case.get(name, []) if isinstance(item, dict)]


def parse_timestamp(value: object) -> datetime | None:
    if not isinstance(value, str) or not value.strip():
        return None
    try:
        timestamp = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None
    return timestamp if timestamp.tzinfo is not None else None


def check_case(case: dict) -> list[str]:
    errors: list[str] = []
    info = case.get("overall_information_content")
    quality = case.get("context_quality")
    coverage = case.get("coverage", {})
    overall = coverage.get("overall")
    items = all_items(case)
    primary = case.get("primary_context")
    case_id = case.get("case_id", "unknown")
    items_by_event_id = {
        item.get("event_id"): item
        for item in items
        if isinstance(item.get("event_id"), str) and item.get("event_id")
    }
    observations_by_id = {
        observation.get("observation_id"): observation
        for observation in case.get("observed_market_reactions", [])
        if isinstance(observation, dict)
        and isinstance(observation.get("observation_id"), str)
        and observation.get("observation_id")
    }

    if case.get("decision_weight") != 0 or case.get("trade_signal") is not False:
        errors.append(f"{case_id}: decision boundary is not frozen")
    if any(case.get(name) != "none" for name in ("gate_effect", "execution_effect", "position_sizing_effect")):
        errors.append(f"{case_id}: effect must be none")
    if any(item.get("information_content") in {"HIGH", "MEDIUM"} and not item.get("evidence") for item in items):
        errors.append(f"{case_id}: HIGH/MEDIUM item lacks EvidenceRecord")
    report_run_at = parse_timestamp(case.get("report_run_at"))
    for item in items:
        accepted_at = parse_timestamp(item.get("accepted_at"))
        if report_run_at is not None and accepted_at is not None and accepted_at > report_run_at:
            errors.append(f"{case_id}: Event accepted after report_run_at remains visible")
    for binding in case.get("temporal_bindings", []):
        if not isinstance(binding, dict):
            errors.append(f"{case_id}: temporal binding must be an object")
            continue
        event = items_by_event_id.get(binding.get("event_id"))
        observation = observations_by_id.get(binding.get("observation_id"))
        source_published_at = parse_timestamp(binding.get("source_published_at"))
        observed_at = parse_timestamp(binding.get("observed_at"))
        eligible = binding.get("temporal_eligible") is True
        if eligible and (event is None or observation is None):
            errors.append(f"{case_id}: eligible temporal binding references an unknown record")
        if eligible and (source_published_at is None or observed_at is None):
            errors.append(f"{case_id}: eligible temporal binding has invalid timestamps")
        if eligible and source_published_at is not None and observed_at is not None:
            if source_published_at > observed_at:
                errors.append(f"{case_id}: eligible binding violates publication <= observation")
        if not eligible and source_published_at is not None and observed_at is not None:
            if source_published_at <= observed_at and binding.get("reason") == "source_published_at is before or equal to observed_at":
                errors.append(f"{case_id}: ineligible binding contradicts publication <= observation")
    if any(item.get("information_content") in {"HIGH", "MEDIUM"} for item in case.get("scheduled_macro", [])):
        if info == "LOW" or primary is None:
            errors.append(f"{case_id}: scheduled high/medium event was suppressed")
    if any(item.get("type") in {"CPI", "NONFARM_PAYROLLS", "FOMC_RATE_DECISION"} for item in items):
        if primary is None:
            errors.append(f"{case_id}: primary context is missing for tier-one macro event")
    if case.get("commodity_events") and primary is None and any(
        item.get("information_content") in {"HIGH", "MEDIUM"} for item in case["commodity_events"]
    ):
        errors.append(f"{case_id}: commodity shock has no context")
    if overall != "HEALTHY" and quality == "HIGH":
        errors.append(f"{case_id}: non-healthy coverage cannot have HIGH quality")
    if info == "LOW":
        statuses = [coverage.get(name) for name in (
            "scheduled_macro", "corporate", "geopolitical", "commodity", "rates_credit", "market_structure"
        )]
        if overall != "HEALTHY" or any(status != "HEALTHY" for status in statuses):
            errors.append(f"{case_id}: LOW requires all source groups HEALTHY")
        if any(item.get("information_content") in {"HIGH", "MEDIUM"} for item in items):
            errors.append(f"{case_id}: LOW coexists with HIGH/MEDIUM event")
    text = str(case.get("interpretation", ""))
    if overall != "HEALTHY" and "No major event today" in text:
        errors.append(f"{case_id}: absolute absence wording under incomplete coverage")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixtures", default="tests/fixtures/signal_context")
    args = parser.parse_args()
    fixture_dir = ROOT / args.fixtures
    paths = sorted(fixture_dir.glob("*.json"))
    if not paths:
        print(f"no Signal Context fixtures found: {fixture_dir}", file=sys.stderr)
        return 1
    errors = [error for path in paths for case in load_cases(path) for error in check_case(case)]
    if errors:
        for error in errors:
            print(f"[ERROR] {error}", file=sys.stderr)
        return 1
    print(f"✅ signal context consistency passed: {len(paths)} fixture(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
