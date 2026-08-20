#!/usr/bin/env python3
"""Publish evidence-derived machine-readable status for active Work Items."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess  # nosec B404 - fixed git argv is read-only and no shell is used
import sys
import tempfile
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from ai_governance_compression import derive_governance_status

ROOT = Path(__file__).resolve().parents[1]
ACTIVE = ROOT / ".ai" / "work-items" / "active"
OUTPUT = ROOT / ".ai" / "cockpit" / "work-items"
SAFE_ACTIONS = {
    "green": ["review_evidence", "refresh_status"],
    "yellow": ["review_risks", "request_human_decision", "refresh_status"],
    "red": ["stop", "resolve_blockers", "refresh_status"],
    "unknown": ["inspect_evidence", "refresh_status"],
}
PHASES = {
    "intake",
    "preflight",
    "implementation",
    "verification",
    "review",
    "finish",
    "closure",
    "closed",
    "unknown",
}
STATES = {"green", "yellow", "red", "unknown"}
STATE_BY_RECOMMENDATION = {
    "ready_for_review": "green",
    "ready_with_risks": "yellow",
    "needs_investigation": "red",
    "blocked": "red",
}


def _digest(value: Any) -> str:
    return (
        "sha256:"
        + hashlib.sha256(
            json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest()
    )


def _file_digest(path: Path) -> str | None:
    try:
        return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError:
        return None


def _now() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _timestamp(value: Any) -> str | None:
    if not isinstance(value, str) or not value.strip():
        return None
    try:
        parsed = datetime.fromisoformat(value)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        return None
    return parsed.astimezone(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _identifier_from_path(path: Path) -> str:
    name = path.name
    if name.endswith(".contract.json"):
        return name[: -len(".contract.json")]
    return path.stem


def _base_status(
    work_item: str,
    *,
    now: str,
    base_commit: str | None,
    branch: str | None,
    diagnostics: list[str],
    phase: str = "unknown",
    state: str = "unknown",
    blocking: bool = True,
    human_decision: bool = False,
    blockers: list[dict[str, str]] | None = None,
    last_verification: str | None = None,
    freshness: str = "unknown",
    freshness_reason: str = "status evidence is unavailable",
    source_digests: dict[str, str | None] | None = None,
) -> dict[str, Any]:
    value: dict[str, Any] = {
        "schemaVersion": 1,
        "workItem": work_item,
        "state": state if state in STATES else "unknown",
        "phase": phase if phase in PHASES else "unknown",
        "blocking": bool(blocking),
        "blockers": blockers or [],
        "humanDecisionRequired": bool(human_decision),
        "safeActions": SAFE_ACTIONS.get(state, SAFE_ACTIONS["unknown"]),
        "baseCommit": base_commit,
        "branch": branch,
        "lastVerificationAt": last_verification,
        "evidenceFreshness": {
            "state": freshness if freshness in {"fresh", "stale", "unknown"} else "unknown",
            "reason": freshness_reason,
        },
        "diagnostics": sorted(set(diagnostics)),
        "sourceDigests": source_digests or {},
        "updatedAt": now,
    }
    value["statusDigest"] = _digest(value)
    validate_status(value)
    return value


def validate_status(value: dict[str, Any]) -> None:
    """Validate the generated payload's strict interface invariants."""
    required = {
        "schemaVersion",
        "workItem",
        "state",
        "phase",
        "blocking",
        "blockers",
        "humanDecisionRequired",
        "safeActions",
        "baseCommit",
        "branch",
        "lastVerificationAt",
        "evidenceFreshness",
        "diagnostics",
        "sourceDigests",
        "updatedAt",
        "statusDigest",
    }
    missing = sorted(required - set(value))
    if missing:
        raise ValueError(f"status is missing required field(s): {', '.join(missing)}")
    if value.get("schemaVersion") != 1 or value.get("state") not in STATES:
        raise ValueError("status schema version or state is invalid")
    if value.get("phase") not in PHASES or not isinstance(value.get("workItem"), str):
        raise ValueError("status phase or Work Item identity is invalid")
    if not isinstance(value.get("blockers"), list) or not isinstance(
        value.get("safeActions"), list
    ):
        raise TypeError("status blockers and safeActions must be arrays")
    freshness = value.get("evidenceFreshness")
    if not isinstance(freshness, dict) or freshness.get("state") not in {
        "fresh",
        "stale",
        "unknown",
    }:
        raise ValueError("status evidenceFreshness is invalid")


def _phase(contract: dict[str, Any], summary: dict[str, Any]) -> str:
    records = summary.get("verification")
    if isinstance(records, list) and records:
        if any(isinstance(item, dict) and item.get("result") == "failed" for item in records):
            return "verification"
        if any(isinstance(item, dict) and item.get("result") == "passed" for item in records):
            return "review"
        return "verification"
    decision = contract.get("executionDecision")
    if isinstance(decision, dict) and decision.get("status") not in {"continue", None}:
        return "preflight"
    return "implementation"


def _human_decision(contract: dict[str, Any], summary: dict[str, Any]) -> bool:
    capability = contract.get("agentCapability")
    if isinstance(capability, dict) and capability.get("needsHumanDecision") is True:
        return True
    decision = contract.get("executionDecision")
    if isinstance(decision, dict) and decision.get("status") in {
        "needs_human_decision",
        "block",
        "defer",
    }:
        return True
    request = summary.get("humanDecisionRequest")
    return isinstance(request, dict) and request.get("status") in {
        "needs_human_confirmation",
        "human_decision_recorded",
    }


def _blockers(model: dict[str, Any], diagnostics: list[str]) -> list[dict[str, str]]:
    values: list[dict[str, str]] = []
    for item in model.get("decisionDrivers", []):
        if isinstance(item, str) and item.strip():
            values.append({"code": "governance_driver", "detail": item.strip()})
    for code in diagnostics:
        values.append({"code": code, "detail": code.replace("_", " ")})
    return values


def _verification(
    work_item: str,
    contract: dict[str, Any],
    summary: dict[str, Any],
    current_commit: str | None,
) -> tuple[str, str, str | None, list[str]]:
    diagnostics: list[str] = []
    records = summary.get("verification")
    required = {
        str(item.get("check"))
        for item in contract.get("verification", [])
        if isinstance(item, dict) and item.get("required") is True and item.get("check")
    }
    if not isinstance(records, list):
        return "unknown", "verification evidence is missing", None, ["verification_missing"]
    observed: set[str] = set()
    latest: str | None = None
    latest_dt: datetime | None = None
    for record in records:
        if not isinstance(record, dict):
            diagnostics.append("malformed_verification_record")
            continue
        check = record.get("check")
        if isinstance(check, str):
            observed.add(check)
        for key in ("executionContractPath", "executionSummaryPath"):
            bound = record.get(key)
            expected_name = (
                f"{work_item}.contract.json"
                if key == "executionContractPath"
                else f"{work_item}.summary.json"
            )
            if not isinstance(bound, str) or Path(bound).name != expected_name:
                diagnostics.append("cross_work_item_evidence")
        executed = _timestamp(record.get("executedAt"))
        if executed is None and record.get("executedAt") is not None:
            diagnostics.append("malformed_verification_timestamp")
        if executed:
            parsed = datetime.fromisoformat(executed)
            if latest_dt is None or parsed > latest_dt:
                latest_dt, latest = parsed, executed
    if required - observed:
        diagnostics.append("verification_missing")
    if diagnostics:
        if "cross_work_item_evidence" in diagnostics:
            return (
                "stale",
                "verification evidence is bound to another Work Item",
                latest,
                diagnostics,
            )
        return "unknown", "verification evidence is incomplete or malformed", latest, diagnostics
    if not current_commit:
        return "unknown", "current commit is unavailable", latest, ["current_commit_unavailable"]
    commits = {
        record.get("commitSha")
        for record in records
        if isinstance(record, dict) and isinstance(record.get("commitSha"), str)
    }
    if current_commit not in commits:
        return (
            "stale",
            "verification evidence does not match the current commit",
            latest,
            ["stale_verification"],
        )
    return "fresh", "verification evidence matches the current commit", latest, []


def build_status(
    contract: dict[str, Any],
    summary: dict[str, Any],
    *,
    branch: str | None,
    current_commit: str | None,
    now: str | None = None,
    contract_digest: str | None = None,
    summary_digest: str | None = None,
) -> dict[str, Any]:
    now = now or _now()
    work_item = contract.get("workItemId")
    if not isinstance(work_item, str) or not work_item:
        return _base_status(
            "unknown",
            now=now,
            base_commit=None,
            branch=branch,
            diagnostics=["malformed_contract"],
            source_digests={"contract": contract_digest, "summary": summary_digest},
        )
    diagnostics: list[str] = []
    if summary.get("workItemId") != work_item:
        diagnostics.append("cross_work_item_evidence")
    base_commit = (
        contract.get("baseCommit") if isinstance(contract.get("baseCommit"), str) else None
    )
    if base_commit is None:
        diagnostics.append("malformed_base_commit")
    try:
        model = derive_governance_status(contract, summary)
    except (KeyError, TypeError, ValueError):
        model = {"recommendation": "needs_investigation", "decisionDrivers": []}
        diagnostics.append("malformed_governance_evidence")
    freshness, freshness_reason, last_verification, verification_diagnostics = _verification(
        work_item, contract, summary, current_commit
    )
    diagnostics.extend(verification_diagnostics)
    recommendation = model.get("recommendation")
    recommendation_key = recommendation if isinstance(recommendation, str) else ""
    state = STATE_BY_RECOMMENDATION.get(recommendation_key, "unknown")
    if diagnostics or freshness != "fresh":
        state = "unknown"
    human_decision = _human_decision(contract, summary)
    if human_decision and state == "green":
        state = "yellow"
    return _base_status(
        work_item,
        now=now,
        base_commit=base_commit,
        branch=branch,
        diagnostics=diagnostics,
        phase=_phase(contract, summary),
        state=state,
        blocking=state in {"red", "unknown"} or human_decision,
        human_decision=human_decision,
        blockers=_blockers(model, diagnostics),
        last_verification=last_verification,
        freshness=freshness,
        freshness_reason=freshness_reason,
        source_digests={"contract": contract_digest, "summary": summary_digest},
    )


def _load_object(path: Path) -> tuple[dict[str, Any] | None, str | None]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None, "malformed"
    if not isinstance(value, dict):
        return None, "malformed"
    return value, None


def _branch(root: Path, work_item: str) -> str | None:
    receipt = root / ".ai" / "work-items" / "starts" / f"{work_item}.json"
    value, error = _load_object(receipt)
    if error or not value:
        return None
    branch = value.get("baseBranch")
    return branch if isinstance(branch, str) and branch else None


def _head(root: Path) -> str | None:
    try:
        result = subprocess.run(  # nosec B603 B607 - fixed git argv, no shell, read-only query
            ["git", "rev-parse", "HEAD"],
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError:
        return None
    value = result.stdout.strip()
    return value if result.returncode == 0 and len(value) == 40 else None


def project_status(
    contract_path: Path,
    summary_path: Path,
    *,
    root: Path = ROOT,
    current_commit: str | None = None,
    branch: str | None = None,
    now: str | None = None,
) -> dict[str, Any]:
    work_item = _identifier_from_path(contract_path)
    contract, contract_error = _load_object(contract_path)
    summary, summary_error = _load_object(summary_path)
    contract_digest = _file_digest(contract_path)
    summary_digest = _file_digest(summary_path)
    if contract_error or contract is None:
        return _base_status(
            work_item,
            now=now or _now(),
            base_commit=None,
            branch=branch,
            diagnostics=["malformed_contract"],
            source_digests={"contract": contract_digest, "summary": summary_digest},
        )
    if summary_error or summary is None:
        value = build_status(
            contract,
            {},
            branch=branch,
            current_commit=current_commit,
            now=now,
            contract_digest=contract_digest,
            summary_digest=summary_digest,
        )
        value["diagnostics"] = sorted({*value["diagnostics"], "malformed_summary"})
        value["state"] = "unknown"
        value["blocking"] = True
        value["safeActions"] = SAFE_ACTIONS["unknown"]
        value["statusDigest"] = _digest(
            {key: val for key, val in value.items() if key != "statusDigest"}
        )
        validate_status(value)
        return value
    return build_status(
        contract,
        summary,
        branch=branch,
        current_commit=current_commit,
        now=now,
        contract_digest=contract_digest,
        summary_digest=summary_digest,
    )


def _write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", dir=path.parent, delete=False
    ) as handle:
        json.dump(value, handle, ensure_ascii=False, indent=2, sort_keys=True)
        handle.write("\n")
        temp = Path(handle.name)
    temp.replace(path)


def generate(
    *,
    root: Path = ROOT,
    current_commit: str | None = None,
    now: str | None = None,
    output: Path | None = None,
) -> dict[str, Any]:
    active = root / ".ai" / "work-items" / "active"
    output = output or root / ".ai" / "cockpit" / "work-items"
    statuses: list[dict[str, Any]] = []
    for contract_path in sorted(active.glob("*.contract.json")):
        work_item = _identifier_from_path(contract_path)
        summary_path = active / f"{work_item}.summary.json"
        value = project_status(
            contract_path,
            summary_path,
            root=root,
            current_commit=current_commit if current_commit is not None else _head(root),
            branch=_branch(root, work_item),
            now=now,
        )
        statuses.append(value)
        _write_json(output / f"{work_item}.status.json", value)
    generated = now or _now()
    index: dict[str, Any] = {
        "schemaVersion": 1,
        "generatedAt": generated,
        "items": statuses,
        "counts": {
            state: sum(item["state"] == state for item in statuses)
            for state in ("green", "yellow", "red", "unknown")
        },
    }
    index["indexDigest"] = _digest(index)
    _write_json(output / "index.json", index)
    return index


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=str(ROOT))
    parser.add_argument("--contract")
    parser.add_argument("--summary")
    parser.add_argument("--output", default=None)
    parser.add_argument("--current-commit")
    parser.add_argument("--now")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = Path(args.root).resolve()
    if bool(args.contract) != bool(args.summary):
        print("--contract and --summary must be supplied together", file=sys.stderr)
        return 2
    if args.contract and args.summary:
        value = project_status(
            Path(args.contract),
            Path(args.summary),
            root=root,
            current_commit=args.current_commit or _head(root),
            branch=_branch(root, _identifier_from_path(Path(args.contract))),
            now=args.now,
        )
        output = (
            Path(args.output)
            if args.output
            else root / ".ai/cockpit/work-items" / (f"{value['workItem']}.status.json")
        )
        _write_json(output, value)
        print(json.dumps(value, ensure_ascii=False, sort_keys=True))
        return 1 if value["state"] == "unknown" else 0
    index = generate(
        root=root,
        current_commit=args.current_commit,
        now=args.now,
        output=Path(args.output) if args.output else None,
    )
    print(json.dumps(index, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
