"""Build and validate adopter-local Runtime Verification evidence."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from datetime import UTC, datetime
from typing import Any

from ai_start_receipt import receipt_binding

TEMPLATE_OWNED = {
    ".ai/cockpit/sbom.json",
    ".ai/cockpit/provenance.json",
    ".ai/cockpit/release-digests.json",
    ".ai/cockpit/bandit_low_risk_baseline.json",
}


def _digest(value: Mapping[str, Any]) -> str:
    payload = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def build_runtime_verification(
    contract: Mapping[str, Any],
    summary: Mapping[str, Any],
    receipt: Mapping[str, Any],
    *,
    source_release_tag: str,
    source_repository: str,
    checks: Sequence[Mapping[str, Any]],
    verified_at: str | None = None,
) -> dict[str, Any]:
    """Create evidence tied to the adopter Work Item and its Start Receipt."""
    if contract.get("workItemId") != "adopt_ai_cockpit":
        raise ValueError("Runtime Verification requires the adopt_ai_cockpit Work Item")
    if summary.get("workItemId") != contract.get("workItemId"):
        raise ValueError("Runtime Verification Summary binding does not match Contract")
    if contract.get("startReceipt") != receipt_binding(dict(receipt)):
        raise ValueError("Runtime Verification receipt binding does not match Contract")
    normalized: list[dict[str, Any]] = []
    for item in checks:
        check = str(item.get("check", ""))
        result = str(item.get("result", ""))
        if not check or result not in {"passed", "failed", "not_run"}:
            raise ValueError("Runtime Verification checks require a name and valid result")
        evidence = str(item.get("evidence", ""))
        if evidence in TEMPLATE_OWNED:
            raise ValueError(f"template-owned evidence is not adopter evidence: {evidence}")
        normalized.append({key: value for key, value in item.items() if value is not None})
    return {
        "verificationVersion": 1,
        "workItemId": "adopt_ai_cockpit",
        "contractDigest": _digest(dict(contract)),
        "summaryDigest": _digest(dict(summary)),
        "receiptBinding": receipt_binding(dict(receipt)),
        "sourceReleaseTag": source_release_tag or "unknown",
        "sourceRepository": source_repository or "unknown",
        "verifiedAt": verified_at or datetime.now(UTC).isoformat(),
        "checks": normalized,
        "projectQualityState": "not_configured",
        "readiness": "not_ready",
        "enterpriseAssurance": "not_claimed",
    }


def validate_runtime_verification(
    evidence: Mapping[str, Any],
    contract: Mapping[str, Any],
    summary: Mapping[str, Any],
    receipt: Mapping[str, Any],
) -> list[str]:
    """Return fail-closed validation issues for Runtime Verification."""
    issues: list[str] = []
    if evidence.get("workItemId") != "adopt_ai_cockpit":
        issues.append("Runtime Verification workItemId is not adopt_ai_cockpit")
    if evidence.get("contractDigest") != _digest(dict(contract)):
        issues.append("Runtime Verification Contract digest does not match")
    if evidence.get("summaryDigest") != _digest(dict(summary)):
        issues.append("Runtime Verification Summary digest does not match")
    if evidence.get("receiptBinding") != receipt_binding(dict(receipt)):
        issues.append("Runtime Verification receipt binding does not match receipt")
    if evidence.get("projectQualityState") != "not_configured":
        issues.append("Runtime Verification must preserve project quality as not_configured")
    if evidence.get("readiness") != "not_ready":
        issues.append("Runtime Verification must preserve readiness as not_ready")
    for item in evidence.get("checks", []):
        path = item.get("evidence") if isinstance(item, Mapping) else None
        if path in TEMPLATE_OWNED:
            issues.append(f"template-owned evidence is not adopter evidence: {path}")
        if isinstance(item, Mapping) and item.get("result") == "not_run" and not item.get("reason"):
            issues.append("not_run checks require a reason")
    return issues
